//! The A2DP source (700-BLT-0007): the SBC encoder bit-exact against frames checked by an independent decoder, the
//! bit allocation and CRC, the configuration chosen, and AVDTP both as initiator and as acceptor against a simulated
//! sink, down to paced media packets. Evidence for the encoder and these exchanges only: a real sink is the bench's.
mod common;
use bt_host::avdtp::{self, Source};
use bt_host::hci;
use bt_host::l2cap;
use bt_host::sbc::{self, Config, Encoder, Mode, Tone};
use common::*;

/// FNV-1a over a stream, to check a whole second of frames without keeping them.
fn fnv(bytes: &[u8]) -> u64 { bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)) }

/// One second of a tone encoded: the stream's length and hash, and its first 16 frames in hex, a line each.
fn second(cfg: Config, left: u32, right: u32, level: i16) -> (usize, u64, String) {
    let mut enc = Encoder::new(cfg);
    let mut tone = Tone::new(left, right, cfg.hz(), level);
    let (ch, n) = (cfg.channels(), cfg.samples());
    let (mut all, mut hex, mut buf) = (Vec::new(), String::new(), [0u8; 600]);
    for k in 0..cfg.hz() as usize / n {
        let mut pcm = vec![0i16; n * ch];
        tone.fill(&mut pcm, ch);
        let len = enc.frame(&pcm, &mut buf);
        assert_eq!(len, cfg.frame_len());
        all.extend_from_slice(&buf[..len]);
        if k < 16 { for b in &buf[..len] { hex += &format!("{:02x}", b); } hex += "\n"; }
    }
    (all.len(), fnv(&all), hex)
}

/// The vectors in tests/data/sbc_*.hex are this encoder's frames for those tones. Each whole second was decoded with
/// sbcdec (libsbc 2.0, Ubuntu's sbc-tools, used as a tool only) and compared with the input: the decoder's output lags
/// by the filter bank's 73 (8 subbands) or 37 (4 subbands) samples, with an SNR of 52.6, 48.3, 52.5 and 54.1 dB.
#[test]
fn the_encoder_makes_the_frames_an_independent_decoder_took() {
    let sets = [
        ("joint48", Config { rate: 3, blocks: 16, mode: Mode::Joint, snr: false, subbands: 8, bitpool: 53 }, 440, 660, 8192, 44625, 0x2ab7_d849_fdd6_4f72u64),
        ("stereo44", Config { rate: 2, blocks: 8, mode: Mode::Stereo, snr: true, subbands: 4, bitpool: 31 }, 1000, 3000, 20000, 53742, 0xae7a_90ad_0113_9615),
        ("mono48", Config { rate: 3, blocks: 12, mode: Mode::Mono, snr: false, subbands: 8, bitpool: 28 }, 2500, 0, 16000, 25000, 0x0c1f_3e10_b6a6_12a6),
        ("dual32", Config { rate: 1, blocks: 4, mode: Mode::Dual, snr: true, subbands: 8, bitpool: 16 }, 300, 5000, 12000, 28000, 0xa967_dc70_1d1a_a6de),
    ];
    for (name, cfg, l, r, level, len, hash) in sets {
        let (n, h, hex) = second(cfg, l, r, level);
        let want = std::fs::read_to_string(format!("{}/tests/data/sbc_{}.hex", env!("CARGO_MANIFEST_DIR"), name)).unwrap();
        assert_eq!(hex, want, "{}: the first frames", name);
        assert_eq!((n, h), (len, hash), "{}: the whole second", name);
    }
}

#[test]
fn frames_have_their_header_crc_and_length() {
    let cfg = Config { rate: 3, blocks: 16, mode: Mode::Joint, snr: false, subbands: 8, bitpool: 53 };
    // 4 + 8 + (8 + 16 × 53) / 8 rounded up: 119 bytes, 357 kbit/s.
    assert_eq!((cfg.frame_len(), cfg.bitrate()), (119, 357_000));
    assert_eq!(Config { mode: Mode::Mono, bitpool: 31, ..cfg }.frame_len(), 4 + 4 + 62);
    let mut enc = Encoder::new(cfg);
    let mut out = [0u8; 200];
    let n = enc.frame(&[0; 256], &mut out);
    // Sync, 48 kHz, 16 blocks, joint stereo, loudness, 8 subbands; the bitpool.
    assert_eq!(&out[..3], &[0x9C, 0b11_11_11_0_1, 53]);
    // Silence: every scale factor 0, no subband joined.
    assert!(out[4..13].iter().all(|&b| b == 0));
    // The CRC over the header's two bytes, the join bits and the scale factors: 16 + 8 + 64 bits.
    let mut covered = vec![out[1], out[2]];
    covered.extend_from_slice(&out[4..13]);
    assert_eq!(out[3], sbc::crc8(&covered, 16 + 8 + 64));
    assert_eq!(n, 119);
}

#[test]
fn the_bit_allocation_fills_the_bitpool() {
    // Loudness at 48 kHz, 8 subbands, stereo: every bit of the bitpool given, none above 16.
    for pool in [2u8, 19, 35, 53, 250] {
        let cfg = Config { rate: 3, blocks: 16, mode: Mode::Stereo, snr: false, subbands: 8, bitpool: pool };
        let scale = [[9, 8, 7, 6, 5, 4, 3, 2], [9, 9, 8, 6, 4, 2, 1, 0]];
        let mut bits = [[0u8; 8]; 2];
        sbc::allocate(&cfg, &scale, &mut bits);
        let total: u32 = bits.iter().flatten().map(|&b| b as u32).sum();
        assert!(bits.iter().flatten().all(|&b| b <= 16));
        assert!(total <= pool as u32 && (total == pool as u32 || bits.iter().flatten().all(|&b| b == 16) || pool < 4), "bitpool {}: {} bits", pool, total);
    }
    // The low subband gets more under loudness (its offset is -4).
    let cfg = Config { rate: 3, blocks: 16, mode: Mode::Mono, snr: false, subbands: 8, bitpool: 20 };
    let mut bits = [[0u8; 8]; 2];
    sbc::allocate(&cfg, &[[6; 8], [0; 8]], &mut bits);
    assert!(bits[0][0] > bits[0][1]);
}

#[test]
fn the_configuration_is_chosen_within_the_sinks() {
    // Everything offered: 48 kHz, joint stereo, 16 blocks, 8 subbands, loudness, bitpool 53.
    let cfg = avdtp::choose([0xFF, 0xFF, 2, 53]).unwrap();
    assert_eq!(cfg, Config { rate: 3, blocks: 16, mode: Mode::Joint, snr: false, subbands: 8, bitpool: 53 });
    assert_eq!(avdtp::codec_info(&cfg, 2), [0x11, 0x15, 2, 53]);
    // 44.1 kHz stereo only, bitpool up to 35, SNR only.
    let cfg = avdtp::choose([0x22, 0x16, 2, 35]).unwrap();
    assert_eq!((cfg.rate, cfg.mode, cfg.blocks, cfg.snr, cfg.bitpool), (2, Mode::Stereo, 16, true, 35));
    // No common frequency (16 and 32 kHz only), a bitpool range we cannot meet.
    assert!(avdtp::choose([0xCF, 0xFF, 2, 53]).is_none());
    assert!(avdtp::choose([0x11, 0xFF, 70, 60]).is_none());
    // What a sink sets: one bit each, or refused.
    assert_eq!(avdtp::configured([0x21, 0x15, 2, 51]).map(|c| (c.rate, c.mode, c.bitpool)), Some((2, Mode::Joint, 51)));
    assert!(avdtp::configured([0x31, 0x15, 2, 51]).is_none());
    assert!(avdtp::configured([0x81, 0x15, 2, 51]).is_none());
    // The capability inside a Get Capabilities answer.
    assert_eq!(avdtp::sbc_capability(&[1, 0, 7, 6, 0, 0, 0x3F, 0xFF, 2, 53, 8, 0]), Some([0x3F, 0xFF, 2, 53]));
    assert_eq!(avdtp::sbc_capability(&[1, 0, 7, 6, 0, 2, 0x3F, 0xFF, 2, 53]), None);
    // The media packet: RTP version 2, payload type 96.
    assert_eq!(avdtp::rtp_header(0x0102, 0x0A0B0C0D), [0x80, 0x60, 1, 2, 0x0A, 0x0B, 0x0C, 0x0D, 0, 0, 0, 1]);
    assert_eq!(Source::frames_per_packet(&Config { rate: 3, blocks: 16, mode: Mode::Joint, snr: false, subbands: 8, bitpool: 53 }, 895), 7);
}

/// A sink after pairing: the link up and secured, the AVDTP channel we open configured; its local CID.
fn sink() -> (Sim, u16) {
    let mut s = Sim::new("11:22:33:44:55:66 audio\n");
    s.acl_len = 1021;
    s.stack.ctl.bredr.len = 1021;
    s.tick(10);
    s.connected(0x0B);
    s.pair(0x0B);
    s.encrypted(0x0B);
    let f = s.frames();
    let local = le16(&f[0].2, 6);
    open(&mut s, f[0].2[1], local, 0x0060, 895);
    (s, local)
}

/// The peer's side of opening our channel `local` to its CID `remote` with an MTU.
fn open(s: &mut Sim, ident: u8, local: u16, remote: u16, mtu: u16) {
    let (l, r) = (local.to_le_bytes(), remote.to_le_bytes());
    s.send(0x0B, l2cap::SIGNALLING, &sig(l2cap::CONNECTION_RESPONSE, ident, &[r[0], r[1], l[0], l[1], 0, 0, 0, 0]), 1021);
    let f = s.frames();
    assert_eq!(f[0].2[0], l2cap::CONFIGURATION_REQUEST);
    let m = mtu.to_le_bytes();
    s.send(0x0B, l2cap::SIGNALLING, &[sig(l2cap::CONFIGURATION_RESPONSE, f[0].2[1], &[l[0], l[1], 0, 0, 0, 0]), sig(l2cap::CONFIGURATION_REQUEST, 0x40, &[l[0], l[1], 0, 0, 1, 2, m[0], m[1]])].concat(), 1021);
}

/// The AVDTP signals the stack sent on the sink's CID `cid`: label, message type, signal, parameters.
fn signals(s: &mut Sim, cid: u16) -> Vec<(u8, u8, u8, Vec<u8>)> {
    s.frames().into_iter().filter(|f| f.1 == cid).map(|f| (f.2[0] >> 4, f.2[0] & 3, f.2[1] & 0x3F, f.2[2..].to_vec())).collect()
}

#[test]
fn as_initiator_the_source_configures_opens_and_streams() {
    let (mut s, local) = sink();
    // Our channel open: Discover.
    let d = signals(&mut s, 0x60);
    assert_eq!((d[0].1, d[0].2), (avdtp::COMMAND, avdtp::DISCOVER));
    let label = d[0].0;
    // Two endpoints: a source (left alone) and a free audio sink, SEID 3.
    s.send(0x0B, local, &[label << 4 | 2, avdtp::DISCOVER, 1 << 2, 0x00, 3 << 2, 0x08], 1021);
    let c = signals(&mut s, 0x60);
    assert_eq!((c[0].2, c[0].3.clone()), (avdtp::GET_CAPABILITIES, vec![3 << 2]));
    s.send(0x0B, local, &[c[0].0 << 4 | 2, avdtp::GET_CAPABILITIES, 1, 0, 7, 6, 0, 0, 0x3F, 0xFF, 2, 53], 1021);
    let c = signals(&mut s, 0x60);
    assert_eq!((c[0].2, c[0].3.clone()), (avdtp::SET_CONFIGURATION, vec![3 << 2, 1 << 2, 1, 0, 7, 6, 0, 0, 0x11, 0x15, 2, 53]));
    s.send(0x0B, local, &[c[0].0 << 4 | 2, avdtp::SET_CONFIGURATION], 1021);
    let c = signals(&mut s, 0x60);
    assert_eq!((c[0].2, c[0].3.clone()), (avdtp::OPEN, vec![3 << 2]));
    s.send(0x0B, local, &[c[0].0 << 4 | 2, avdtp::OPEN], 1021);
    // Opened: we open the media channel, a second one to PSM 0x0019.
    let f = s.frames();
    assert_eq!((f[0].2[0], le16(&f[0].2, 4)), (l2cap::CONNECTION_REQUEST, l2cap::PSM_AVDTP));
    let media = le16(&f[0].2, 6);
    open(&mut s, f[0].2[1], media, 0x0061, 895);
    let c = signals(&mut s, 0x60);
    assert_eq!((c[0].2, c[0].3.clone()), (avdtp::START, vec![3 << 2]));
    s.send(0x0B, local, &[c[0].0 << 4 | 2, avdtp::START], 1021);
    assert!(s.logged("A2DP: STREAMING"));
    // One second of media: 7 frames of 128 samples a packet (895-byte MTU), paced by the clock.
    let mut packets = Vec::new();
    for _ in 0..100 {
        s.tick(10);
        packets.extend(s.frames().into_iter().filter(|f| f.1 == 0x61));
    }
    let samples: u32 = packets.len() as u32 * 7 * 128;
    // 1000 ms and the 40 ms lead at 48 kHz: 49 920 samples due, in whole packets.
    assert_eq!(samples, 49_920 / 896 * 896);
    for (i, p) in packets.iter().enumerate() {
        let b = &p.2;
        assert_eq!(&b[..2], &[0x80, 0x60]);
        assert_eq!(u16::from_be_bytes([b[2], b[3]]), i as u16);
        assert_eq!(u32::from_be_bytes([b[4], b[5], b[6], b[7]]), i as u32 * 896);
        assert_eq!(b[12], 7);
        assert_eq!(b.len(), 13 + 7 * 119);
        for f in 0..7 { assert_eq!(b[13 + f * 119], 0x9C); }
    }
    // The sink suspends: no more media.
    s.send(0x0B, local, &[0x50, avdtp::SUSPEND, 1 << 2], 1021);
    let a = signals(&mut s, 0x60);
    assert_eq!((a[0].0, a[0].1, a[0].2), (5, avdtp::ACCEPT, avdtp::SUSPEND));
    s.tick(500);
    assert!(s.frames().iter().all(|f| f.1 != 0x61));
}

#[test]
fn as_acceptor_the_source_answers_a_sink_that_configures_it() {
    let mut s = Sim::new("11:22:33:44:55:66 audio\n");
    s.acl_len = 1021;
    s.stack.ctl.bredr.len = 1021;
    // The sink connects to us, secures the link and opens AVDTP signalling itself.
    let mut req = PEER.0.to_vec();
    req.extend_from_slice(&[0x04, 0x04, 0x24, 1]);
    s.event(hci::EV_CONNECTION_REQUEST, &req);
    s.connected(0x0B);
    s.send(0x0B, l2cap::SIGNALLING, &sig(l2cap::CONNECTION_REQUEST, 1, &[0x19, 0, 0x70, 0]), 1021);
    assert_eq!(ops(&s.run()), vec![hci::AUTHENTICATION_REQUESTED]);
    s.frames();
    s.pair(0x0B);
    s.encrypted(0x0B);
    let f = s.frames();
    let sig_local = le16(&f[0].2, 4);
    let ident = f[1].2[1];
    let l = sig_local.to_le_bytes();
    s.send(0x0B, l2cap::SIGNALLING, &[sig(l2cap::CONFIGURATION_RESPONSE, ident, &[l[0], l[1], 0, 0, 0, 0]), sig(l2cap::CONFIGURATION_REQUEST, 2, &[l[0], l[1], 0, 0])].concat(), 1021);
    s.frames();
    // It discovers us: one audio source.
    s.send(0x0B, sig_local, &[0x10, avdtp::DISCOVER], 1021);
    assert_eq!(signals(&mut s, 0x70), vec![(1, avdtp::ACCEPT, avdtp::DISCOVER, vec![1 << 2, 0x00])]);
    s.send(0x0B, sig_local, &[0x20, avdtp::GET_ALL_CAPABILITIES, 1 << 2], 1021);
    assert_eq!(signals(&mut s, 0x70)[0].3, vec![1, 0, 7, 6, 0, 0, 0x3F, 0xFF, 2, 53]);
    // A configuration we cannot make (16 kHz) refused; 44.1 kHz stereo taken.
    s.send(0x0B, sig_local, &[0x30, avdtp::SET_CONFIGURATION, 1 << 2, 2 << 2, 7, 6, 0, 0, 0x82, 0x15, 2, 35], 1021);
    assert_eq!(signals(&mut s, 0x70)[0], (3, avdtp::REJECT, avdtp::SET_CONFIGURATION, vec![7, avdtp::UNSUPPORTED_CONFIGURATION]));
    s.send(0x0B, sig_local, &[0x40, avdtp::SET_CONFIGURATION, 1 << 2, 2 << 2, 1, 0, 7, 6, 0, 0, 0x22, 0x15, 2, 35], 1021);
    assert_eq!(signals(&mut s, 0x70)[0].1, avdtp::ACCEPT);
    assert!(s.logged("THE SINK CONFIGURED SBC: 44100 HZ, Stereo, 16 BLOCKS, 8 SUBBANDS, LOUDNESS, BITPOOL 35"));
    s.send(0x0B, sig_local, &[0x50, avdtp::OPEN, 1 << 2], 1021);
    assert_eq!(signals(&mut s, 0x70)[0].1, avdtp::ACCEPT);
    // Its media channel to us: accepted; then we start.
    s.send(0x0B, l2cap::SIGNALLING, &sig(l2cap::CONNECTION_REQUEST, 3, &[0x19, 0, 0x71, 0]), 1021);
    let f = s.frames();
    let media = le16(&f[0].2, 4);
    let m = media.to_le_bytes();
    s.send(0x0B, l2cap::SIGNALLING, &[sig(l2cap::CONFIGURATION_RESPONSE, f[1].2[1], &[m[0], m[1], 0, 0, 0, 0]), sig(l2cap::CONFIGURATION_REQUEST, 4, &[m[0], m[1], 0, 0, 1, 2, 0x7F, 0x03])].concat(), 1021);
    let st = signals(&mut s, 0x70);
    assert_eq!((st[0].1, st[0].2, st[0].3.clone()), (avdtp::COMMAND, avdtp::START, vec![2 << 2]));
    s.send(0x0B, sig_local, &[st[0].0 << 4 | 2, avdtp::START], 1021);
    s.tick(100);
    let media_frames: Vec<_> = s.frames().into_iter().filter(|f| f.1 == 0x71).collect();
    // 44.1 kHz, stereo, bitpool 35: 4 + 8 + 70 = 82-byte frames, 10 a packet.
    assert!(!media_frames.is_empty());
    assert_eq!(media_frames[0].2[12], 10);
    assert_eq!(&media_frames[0].2[13..16], &[0x9C, 0b10_11_10_0_1, 35]);
    // Commands for an endpoint we do not have, and unknown signals.
    s.send(0x0B, sig_local, &[0x60, avdtp::GET_CAPABILITIES, 5 << 2], 1021);
    s.send(0x0B, sig_local, &[0x70, 0x3E], 1021);
    let r = signals(&mut s, 0x70);
    assert!(r.iter().any(|x| x == &(6, avdtp::REJECT, avdtp::GET_CAPABILITIES, vec![avdtp::BAD_ACP_SEID])));
    assert!(r.iter().any(|x| x == &(7, avdtp::GENERAL_REJECT, 0x3E, vec![])));
}

#[test]
fn pcm_is_read_at_the_streams_rate() {
    let mut p = avdtp::Pcm::new();
    // A ramp, left rising, right falling.
    let input: Vec<i16> = (0..200).flat_map(|i| [i * 100, -i * 100]).collect();
    p.push(&input, 0);
    assert_eq!(p.frames(), 200);
    // At 48 kHz: the samples as they are.
    let mut out = vec![0i16; 20];
    p.take(&mut out, 2, 48_000);
    assert_eq!(&out[..6], &[0, 0, 100, -100, 200, -200]);
    assert_eq!(p.frames(), 190);
    // At 44.1 kHz: 48/44.1 input frames an output frame, interpolated; mono is the two channels' mean (0 here).
    let mut out = vec![0i16; 10];
    p.take(&mut out, 2, 44_100);
    let left: Vec<i16> = out.iter().step_by(2).copied().collect();
    assert_eq!(left, vec![1000, 1108, 1217, 1326, 1435]);
    let mut mono = vec![1i16; 4];
    p.take(&mut mono, 1, 48_000);
    assert!(mono.iter().all(|&v| v.abs() <= 1));
    // Run short: silence, counted; overfull: the oldest dropped.
    let mut out = vec![7i16; 2 * 400];
    p.take(&mut out, 2, 48_000);
    assert!(out[out.len() - 2..].iter().all(|&v| v == 0) && p.short > 0);
    // One frame stays back: interpolation needs two.
    let left_over = p.frames() as u64;
    p.push(&vec![1i16; 2 * (avdtp::PCM / 2 + 10)], 5);
    assert_eq!((p.frames(), p.dropped), (avdtp::PCM / 2, 10 + left_over));
}
