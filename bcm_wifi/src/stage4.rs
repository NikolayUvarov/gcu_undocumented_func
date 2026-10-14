// Stages 4 and 5 (550-DRV-0025, 550-DRV-0026), now a test of the radio's service calls (550-DRV-0027): the station's
// view of a scan of channels 1 to 13 with every beacon let through; a broadcast probe request on channels 1 to 11 at 1
// and at 6 Mbit/s; probe requests to every network heard, which acknowledge them, sent in eight ways (rates, cores,
// cyclic delay: 550-DRV-0028), with the filter following that network.
use crate::radio::{Filter, Radio, FRAME_MAX, RADIO_HEADER, TX_GAIN};
use crate::stage3::Core;

const DWELL_MS: usize = 200; const ANSWER_MS: usize = 150; const TX_CHANNELS: u8 = 11;
// The networks probed, how often, and the ways of sending (550-DRV-0028): the rate, the cores (a bitmap), cyclic delay.
// b43 sends OFDM on cores 0 and 1 without cyclic delay; Broadcom's own driver uses it on several cores.
const PROBED: usize = 6; const REPEATS: u32 = 3; const ATTEMPTS: u8 = 7;
const WAYS: [(&str, u8, u16, bool); 8] = [
    ("1 MBIT/S, CORES 0+1      ", 2, 0b011, false), ("1 MBIT/S, CORE 0         ", 2, 0b001, false),
    ("11 MBIT/S, CORES 0+1     ", 22, 0b011, false), ("6 MBIT/S, CORES 0+1      ", 12, 0b011, false),
    ("6 MBIT/S, CORE 0         ", 12, 0b001, false), ("6 MBIT/S, CORE 1         ", 12, 0b010, false),
    ("6 MBIT/S, CORE 2         ", 12, 0b100, false), ("6 MBIT/S, CORES 0-2, CDD ", 12, 0b111, true),
];

struct Network { bssid: [u8; 6], ssid: [u8; 32], ssid_len: usize, channel: u8, signal: i8, security: u8, frames: u32 }
const NONE: Network = Network { bssid: [0; 6], ssid: [0; 32], ssid_len: 0, channel: 0, signal: -128, security: 0, frames: 0 };
const SEEN: usize = 48;

// What the station would have received: beacons, probe responses, other frames to us, other frames, an unknown rate.
#[derive(Default)]
struct Heard { beacons: u32, responses: u32, to_us: u32, other: u32, unknown_rate: u32 }

fn step(n: u32, what: &str) { mind::println!("[BCM] STAGE 4, STEP {}: {}", n, what); mind::time::sleep(crate::STEP_MS); }

// One received frame as the service hands it (header, then the MPDU without FCS), counted; a network it names noted.
fn note(f: &[u8], mac: &[u8; 6], seen: &mut [Network; SEEN], heard: &mut Heard) {
    let len = u16::from_le_bytes([f[0], f[1]]) as usize;
    let (weakness, rx_channel, rate) = (f[2], f[3], f[4]);
    let Some(frame) = f.get(RADIO_HEADER..RADIO_HEADER + len) else { heard.other += 1; return };
    if rate == 0 { heard.unknown_rate += 1; }
    let fc = frame[0];
    if fc != 0x80 && fc != 0x50 { if frame.len() >= 10 && frame[4..10] == *mac { heard.to_us += 1 } else { heard.other += 1 } return; }
    if fc == 0x80 { heard.beacons += 1 } else { heard.responses += 1 }
    if frame.len() < 36 { return; }
    let signal = if weakness == 0 { -128 } else { -(weakness as i16) as i8 };
    let mut bssid = [0u8; 6]; bssid.copy_from_slice(&frame[16..22]);
    let capability = u16::from_le_bytes([frame[34], frame[35]]);
    let (mut ssid, mut ssid_len, mut channel, mut security) = ([0u8; 32], 0, rx_channel, if capability & 0x10 != 0 { 1 } else { 0 });
    let mut at = 36;
    while at + 2 <= frame.len() {
        let (id, l) = (frame[at], frame[at + 1] as usize);
        let Some(body) = frame.get(at + 2..at + 2 + l) else { break };
        match id {
            0 if l <= 32 => { ssid_len = l; ssid[..l].copy_from_slice(body); }
            3 if l == 1 => channel = body[0],
            48 => security = 3,
            221 if l >= 4 && body[..4] == [0x00, 0x50, 0xF2, 0x01] && security < 2 => security = 2,
            _ => {}
        }
        at += 2 + l;
    }
    let entry = match seen.iter().position(|n| n.frames != 0 && n.bssid == bssid) {
        Some(i) => &mut seen[i],
        None => match seen.iter().position(|n| n.frames == 0) { Some(i) => &mut seen[i], None => return },
    };
    if entry.frames == 0 { *entry = Network { bssid, ssid, ssid_len, channel, signal, security, frames: 0 }; }
    entry.frames += 1;
    if signal > entry.signal { entry.signal = signal; }
}

// Receiving for `ms`, every frame the radio queues taken as the station takes it.
fn listen(radio: &mut Radio, ms: usize, seen: &mut [Network; SEEN], heard: &mut Heard) {
    let mut buffer = [0u8; RADIO_HEADER + FRAME_MAX];
    let end = mind::time::uptime_ms() + ms;
    loop {
        radio.poll();
        while let Ok(n) = radio.take(&mut buffer) { note(&buffer[..n], &radio.mac, seen, heard); }
        if mind::time::uptime_ms() >= end { return; }
        mind::time::sleep(2);
    }
}

// The networks in `seen`, one line each; how many.
fn list(seen: &[Network; SEEN]) -> usize {
    let mut heard = 0;
    for n in seen.iter().filter(|n| n.frames != 0) {
        heard += 1;
        let b = n.bssid;
        let name = core::str::from_utf8(&n.ssid[..n.ssid_len]).ok().filter(|s| !s.is_empty() && !s.bytes().any(|x| x < 0x20));
        let security = ["OPEN", "WEP", "WPA", "WPA2/RSN"][n.security as usize];
        match name {
            Some(s) => mind::println!("[BCM] NETWORK \"{}\" {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} CHANNEL {} SIGNAL {} DBM {} ({} FRAMES)", s, b[0], b[1], b[2], b[3], b[4], b[5], n.channel, n.signal, security, n.frames),
            None => mind::println!("[BCM] NETWORK (HIDDEN OR NOT TEXT, {} BYTES) {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} CHANNEL {} SIGNAL {} DBM {} ({} FRAMES)", n.ssid_len, b[0], b[1], b[2], b[3], b[4], b[5], n.channel, n.signal, security, n.frames),
        }
    }
    heard
}

// A probe request from `mac` to `to` (broadcast, or a network's BSSID) on `ch`, any SSID; its length. The sequence
// number is ours, as the station's would be.
fn probe(out: &mut [u8; 45], mac: &[u8; 6], to: &[u8; 6], ch: u8, sequence: u16) -> usize {
    *out = [0; 45];
    out[0] = 0x40;
    // A directed request expects an acknowledgement: SIFS and an ACK at 1 Mbit/s.
    let duration: u16 = if to[0] & 1 == 0 { 314 } else { 0 };
    out[2..4].copy_from_slice(&duration.to_le_bytes());
    out[4..10].copy_from_slice(to); out[10..16].copy_from_slice(mac); out[16..22].copy_from_slice(to);
    out[22..24].copy_from_slice(&((sequence & 0x0FFF) << 4).to_le_bytes());
    out[26..36].copy_from_slice(&[0x01, 0x08, 0x02, 0x04, 0x0B, 0x16, 0x0C, 0x12, 0x18, 0x24]);
    out[36..42].copy_from_slice(&[0x32, 0x04, 0x30, 0x48, 0x60, 0x6C]);
    out[42..45].copy_from_slice(&[0x03, 0x01, ch]);
    45
}

// The test, after stage 3 left the MAC suspended with the microcode running; `mac` is the station's address. The
// radio is handed back running, or None when it did not come up.
pub fn run<'a>(c: &'a Core<'a>, mac: [u8; 6]) -> Option<Radio<'a>> {
    step(1, "THE RADIO UP: BOTH RINGS, THE STATION'S ADDRESS, A LOW FIXED GAIN, CHANNEL 1");
    let mut radio = Radio::up(c, mac)?;

    step(2, "A SCAN AS THE STATION SEES IT: CHANNELS 1 TO 13, 200 MS EACH, EVERY BEACON LET THROUGH");
    let mut seen = [NONE; SEEN];
    let _ = radio.set_filter(Filter { all_beacons: true, ..Filter::default() });
    for ch in 1..=13 {
        if let Err(e) = radio.set_channel(ch) { mind::println!("[BCM] CHANNEL {} NOT SET: {:?}", ch, e); break; }
        let before = radio.counts;
        let mut heard = Heard::default();
        listen(&mut radio, DWELL_MS, &mut seen, &mut heard);
        let after = radio.counts;
        mind::println!("[BCM] CHANNEL {}: {} BEACONS, {} PROBE RESPONSES, {} OTHER TO US, {} OTHER; {} OF UNKNOWN RATE; {} FILTERED, {} BAD, {} DMA ERRORS",
                       ch, heard.beacons, heard.responses, heard.to_us, heard.other, heard.unknown_rate, after.filtered - before.filtered,
                       after.bad - before.bad, after.dma_errors - before.dma_errors);
    }

    step(3, "THE NETWORKS HEARD");
    mind::println!("[BCM] SCAN DONE: {} NETWORKS", list(&seen));

    step(4, "A BROADCAST PROBE REQUEST ON EACH OF CHANNELS 1 TO 11, AT 1 AND AT 6 MBIT/S");
    let mut frame = [0u8; 45];
    let mut sequence = 0u16;
    let mut answers = 0;
    for ch in 1..=TX_CHANNELS {
        if let Err(e) = radio.set_channel(ch) { mind::println!("[BCM] CHANNEL {} NOT SET: {:?}", ch, e); break; }
        for rate in [2u8, 12] {
            sequence += 1;
            let len = probe(&mut frame, &mac, &[0xFF; 6], ch, sequence);
            let sent = radio.send(&frame[..len], rate, 1);
            let mut heard = Heard::default();
            listen(&mut radio, ANSWER_MS, &mut seen, &mut heard);
            answers += heard.responses;
            match sent {
                Ok(s) => mind::println!("[BCM] CHANNEL {}, {} MBIT/S: SENT {} TIMES; {} PROBE RESPONSES, {} BEACONS", ch, rate / 2, s.attempts, heard.responses, heard.beacons),
                Err(e) => mind::println!("[BCM] CHANNEL {}, {} MBIT/S: NOT SENT ({:?}); {} PROBE RESPONSES", ch, rate / 2, e, heard.responses),
            }
        }
    }

    step(5, "THE NETWORKS HEARD, WITH THE ANSWERS");
    mind::println!("[BCM] ACTIVE SCAN DONE: {} PROBE RESPONSES; {} NETWORKS IN ALL", answers, list(&seen));

    step(6, "PROBE REQUESTS TO EVERY NETWORK HEARD, WHICH ACKNOWLEDGE THEM: 3 EACH IN 8 WAYS OF SENDING");
    let p = radio.phy_state();
    mind::println!("[BCM] PHY 0B0 {:04X} (CLASSIFIER), C01 {:04X}, C0A {:04X}, 17E {:04X}", p[0], p[1], p[2], p[3]);
    let mut order: [usize; SEEN] = core::array::from_fn(|i| i);
    order.sort_unstable_by_key(|&i| (seen[i].frames == 0, -(seen[i].signal as i16)));
    let targets = order.iter().copied().filter(|&i| seen[i].frames != 0 && (1..=13).contains(&seen[i].channel)).take(PROBED);
    for i in targets {
        let (bssid, ch, signal) = (seen[i].bssid, seen[i].channel, seen[i].signal);
        if radio.set_channel(ch).is_err() || radio.set_filter(Filter { all_beacons: false, bssid, own: [0; 6] }).is_err() { mind::println!("[BCM] CHANNEL {} OR FILTER NOT SET", ch); continue; }
        let b = bssid;
        mind::println!("[BCM] {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} CHANNEL {} ({} DBM), ACKNOWLEDGED OF {} (TRIES, NOT SENT, PROBE RESPONSES):", b[0], b[1], b[2], b[3], b[4], b[5], ch, signal, REPEATS);
        for (name, rate, cores, cdd) in WAYS {
            radio.set_tx_cores(cores, cdd);
            let (mut acked, mut tries, mut failed) = (0, 0u32, 0);
            let mut heard = Heard::default();
            let mut ignore = [NONE; SEEN];
            for _ in 0..REPEATS {
                sequence += 1;
                let len = probe(&mut frame, &mac, &bssid, ch, sequence);
                match radio.send(&frame[..len], rate, ATTEMPTS) {
                    Ok(s) => { acked += s.acked as u32; tries += s.attempts as u32; }
                    Err(_) => failed += 1,
                }
                listen(&mut radio, ANSWER_MS, &mut ignore, &mut heard);
            }
            mind::println!("[BCM]   {}: {} ({}, {}, {})", name, acked, tries, failed, heard.responses);
        }
    }
    radio.set_tx_cores(0b011, false);
    let _ = radio.set_gain(TX_GAIN);
    let _ = radio.set_filter(Filter::default());
    let k = radio.counts;
    mind::println!("[BCM] TEST DONE: {} FRAMES QUEUED, {} FILTERED, {} BAD, {} DMA ERRORS", k.queued, k.filtered, k.bad, k.dma_errors);
    Some(radio)
}
