//! audio_gw's Bluetooth sink (700-DRV-0110): the ring bt_hci reads the gateway's output from while its A2DP stream
//! runs. Whole frames taken while there is room, scaled by the level; a tone; read back in order; the free buffers
//! the gateway's waiters count; a sink not read for 2 s taken as gone. Evidence for the ring's logic only (MC-12.1).
#[path = "../audio_gw/src/btsink.rs"] #[allow(dead_code)] mod btsink;
use btsink::{Sink, FRAME, RING, SILENT_MS};

fn frames(pairs: &[(i16, i16)]) -> Vec<u8> { pairs.iter().flat_map(|&(l, r)| [l.to_le_bytes(), r.to_le_bytes()].concat()).collect() }

#[test]
fn what_is_played_is_read_back_in_order() {
    let mut s = Sink::new(7, 1_000, 4096);
    assert_eq!(s.free(32), 31);
    // Whole frames only: the last 3 bytes are left.
    let pcm = frames(&[(1, -1), (2, -2), (3, -3)]);
    assert_eq!(s.play(&[&pcm[..], &[9, 9, 9]].concat()), 12);
    let mut out = [0u8; 64];
    assert_eq!(s.read(&mut out, 1_010), 12);
    assert_eq!(&out[..12], &pcm[..]);
    assert_eq!(s.read(&mut out, 1_020), 0);
    // A full ring takes no more; reading frees it, in buffers of 4096 bytes.
    let big = vec![0x11u8; RING + 400];
    assert_eq!(s.play(&big), RING);
    assert_eq!((s.free(32), s.play(&big)), (0, 0));
    let mut out = vec![0u8; 4096 * 3 + 2];
    assert_eq!(s.read(&mut out, 1_030), 4096 * 3);
    assert_eq!(s.free(32), 3);
    // Wrapping around the ring keeps the order.
    s.reset();
    let a = vec![0x22u8; RING - 8];
    s.play(&a);
    s.read(&mut vec![0u8; RING - 8], 1_040);
    let pcm = frames(&[(100, 200), (300, 400), (500, 600)]);
    s.play(&pcm);
    let mut out = [0u8; 12];
    assert_eq!(s.read(&mut out, 1_050), 12);
    assert_eq!(&out[..], &pcm[..]);
}

#[test]
fn the_level_scales_and_a_tone_fills() {
    let mut s = Sink::new(7, 0, 4096);
    s.out_factor = 1 << 14;
    s.play(&frames(&[(1000, -1000), (i16::MAX, i16::MIN)]));
    let mut out = [0u8; 8];
    s.read(&mut out, 1);
    assert_eq!(&out[..], &frames(&[(500, -500), (16383, -16384)])[..]);
    // A 1 kHz tone of 10 ms at 48 kHz: 480 frames, both channels alike.
    s.out_factor = 1 << 15;
    let sine = |phase: u32| ((phase % 64) as i16 - 32) * 100;
    assert_eq!(s.tone(1000, 10, 48_000, sine), 480);
    let mut out = vec![0u8; 480 * FRAME];
    assert_eq!(s.read(&mut out, 2), 480 * FRAME);
    assert!(out.chunks_exact(4).all(|f| f[..2] == f[2..]));
    // At most the room there is.
    assert_eq!(s.tone(1000, 5000, 48_000, sine), RING / FRAME);
}

#[test]
fn a_sink_not_read_is_taken_as_gone() {
    let mut s = Sink::new(7, 5_000, 4096);
    assert!(!s.silent(5_000 + SILENT_MS));
    assert!(s.silent(5_001 + SILENT_MS));
    s.read(&mut [0u8; 4], 6_000);
    assert!(!s.silent(6_000 + SILENT_MS));
}
