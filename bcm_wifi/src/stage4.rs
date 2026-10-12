// Stage 4 (550-DRV-0025): frames received by the 802.11 core's DMA engine into a ring in the region init grants,
// polled (no interrupt), and the networks around listed from their beacons on channels 1 to 13, receiving only.
// Stage 5 (550-DRV-0026): one broadcast probe request on each of channels 1 to 11 from a transmit ring, at a fixed
// gain well below b43's, and the probe responses that answer it. Register facts come from the stage-4 and stage-5
// fact sheets (Linux's b43 and brcmsmac, described).
use crate::stage3::Core;
use core::sync::atomic::{fence, Ordering};
use mind::abi::SLOT_MEM;
use mind::dev::Dma;

// The receive engine (engine 0 of the 64-bit layout) and its interrupt reason.
const RX_CONTROL: usize = 0x220; const RX_INDEX: usize = 0x224; const RX_RING_LOW: usize = 0x228; const RX_RING_HIGH: usize = 0x22C;
const RX_STATUS: usize = 0x230; const RX_ERROR: usize = 0x234; const DMA0_REASON: usize = 0x20; const DMA0_MASK: usize = 0x24;
const RX_LAZY: usize = 0x100; const MAC_CONTROL: usize = 0x120; const AGENT_IO_STATUS: usize = 0x1500;
const IO_STATUS_DMA64: u32 = 1 << 12;
// Frame offset 38, enabled, parity check off; the PCIe translation in the high address word.
const RX_ENABLE: u32 = 38 << 1 | 1 | 0x800; const TRANSLATION: u32 = 0x8000_0000;
// MAC control: station, beacons of every network, awake, PMQ discarded; AP, the keep-bad bits, power save and
// discarding transmit status off.
const MAC_SET: u32 = 1 << 17 | 1 << 20 | 1 << 26 | 1 << 30; const MAC_CLEAR: u32 = 1 << 18 | 1 << 21 | 1 << 22 | 1 << 23 | 1 << 25 | 1 << 29;
// The ring: 8 KiB-aligned (bit 12 of its address must stay clear on the BCM4331); 128 buffers of 2390 bytes, 64-aligned.
const RING_BYTES: usize = 8192; const SLOTS: usize = 128; const BUFFER: u32 = 2390; const STRIDE: usize = 2432;
const OFFSET: usize = 38; const DWELL_MS: usize = 200;
// The transmit engine (engine 1: the BCM4331 sends everything there) and the transmit status FIFO.
const TX_CONTROL: usize = 0x240; const TX_INDEX: usize = 0x244; const TX_RING_LOW: usize = 0x248; const TX_RING_HIGH: usize = 0x24C;
const TX_STATUS: usize = 0x250; const TX_ERROR: usize = 0x254; const TX_STATUS0: usize = 0x170; const TX_STATUS1: usize = 0x174;
const TX_ENABLE: u32 = 1 | 0x800; const TX_SLOTS: usize = 16; const TX_STRIDE: usize = 512; const HEADER: usize = 118;
const IRQ_REASON: usize = 0x128; const IRQ_TX_STATUS: u32 = 1 << 29; const IFS_CONTROL: usize = 0x688;
// Descriptor flags: start and end of frame, interrupt on completion, end of table.
const SOF: u32 = 1 << 31; const EOF: u32 = 1 << 30; const IOC: u32 = 1 << 29; const EOT: u32 = 1 << 28;
// The gain table entry for transmitting (b43 uses 40; a higher entry is a lower gain); channels sent on.
const TX_GAIN: u16 = 80; const TX_CHANNELS: usize = 11; const ANSWER_MS: usize = 150;

struct Network { bssid: [u8; 6], ssid: [u8; 32], ssid_len: usize, channel: u8, signal: i8, security: u8, frames: u32 }
const NONE: Network = Network { bssid: [0; 6], ssid: [0; 32], ssid_len: 0, channel: 0, signal: -128, security: 0, frames: 0 };
const SEEN: usize = 48;

struct Ring { dma: Dma, ring: usize, buffers: usize, next: usize, tx_ring: usize, tx_buffers: usize, tx_next: usize }

impl Ring {
    fn buffer(&self, slot: usize) -> usize { self.buffers + slot * STRIDE }
    fn byte(&self, at: usize) -> u8 { (self.dma.read32(at & !3) >> (8 * (at & 3))) as u8 }
    fn u16(&self, at: usize) -> u16 { self.byte(at) as u16 | (self.byte(at + 1) as u16) << 8 }
    fn u32(&self, at: usize) -> u32 { self.u16(at) as u32 | (self.u16(at + 2) as u32) << 16 }
    // Length zeroed and the 8 bytes at the frame offset set to 0xFF, so a buffer the engine did not write is known.
    fn poison(&mut self, slot: usize) {
        let b = self.buffer(slot);
        let first = self.dma.read32(b) & 0xFFFF_0000; self.dma.write32(b, first);
        self.dma.write32(b + 36, (self.dma.read32(b + 36) & 0x0000_FFFF) | 0xFFFF_0000);
        self.dma.write32(b + 40, 0xFFFF_FFFF);
        self.dma.write32(b + 44, (self.dma.read32(b + 44) & 0xFFFF_0000) | 0x0000_FFFF);
    }
    fn poisoned(&self, slot: usize) -> bool { let b = self.buffer(slot) + OFFSET; (0..8).all(|i| self.byte(b + i) == 0xFF) }
}

// The ring and its descriptors in the region; None (logged) when the region is missing or too small.
fn ring() -> Option<Ring> {
    let Ok(mut dma) = Dma::map(SLOT_MEM) else { mind::println!("[BCM] NO DMA REGION: STAGE 4 NOT ATTEMPTED"); return None };
    let base = dma.physical(0) as usize;
    let ring = (RING_BYTES - base % RING_BYTES) % RING_BYTES;
    let buffers = ring + RING_BYTES;
    let tx_ring = ring + (RING_BYTES + SLOTS * STRIDE).div_ceil(RING_BYTES) * RING_BYTES;
    let tx_buffers = tx_ring + RING_BYTES;
    if tx_buffers + TX_SLOTS * TX_STRIDE > dma.len() || dma.physical(dma.len() - 1) >> 32 != 0 {
        mind::println!("[BCM] THE DMA REGION ({} KiB AT {:#X}) CANNOT HOLD THE RING BELOW 4 GiB: STAGE 4 NOT ATTEMPTED", dma.len() / 1024, base);
        return None;
    }
    dma.zero(ring, RING_BYTES); dma.zero(tx_ring, RING_BYTES);
    let mut r = Ring { dma, ring, buffers, next: 0, tx_ring, tx_buffers, tx_next: 0 };
    for slot in 0..SLOTS {
        r.poison(slot);
        let (d, address) = (ring + 16 * slot, r.dma.physical(r.buffer(slot)) as u32);
        r.dma.write32(d, if slot == SLOTS - 1 { 0x1000_0000 } else { 0 });
        r.dma.write32(d + 4, BUFFER);
        r.dma.write32(d + 8, address);
        r.dma.write32(d + 12, TRANSLATION);
    }
    fence(Ordering::SeqCst);
    Some(r)
}

// One frame's 802.11 part, if it is a beacon or a probe response: the network it names, updated in `seen`.
fn frame(r: &Ring, slot: usize, seen: &mut [Network; SEEN], counts: &mut [u32; 5]) {
    let b = r.buffer(slot);
    let length = r.u16(b) as usize;
    if length == 0 || r.poisoned(slot) || length + OFFSET > BUFFER as usize { counts[3] += 1; return; }
    let status = r.u32(b + 16);
    if status & 1 != 0 { counts[2] += 1; return; } // FCS error
    let pad = if status & 4 != 0 { 2 } else { 0 };
    let (start, end) = (b + OFFSET + pad + 6, b + OFFSET + length - 4); // after the PLCP header, before the FCS
    if end < start + 24 + 12 { counts[1] += 1; return; }
    let fc = r.byte(start);
    if fc != 0x80 && fc != 0x50 { counts[1] += 1; return; } // beacon, probe response
    if fc == 0x80 { counts[0] += 1 } else { counts[4] += 1 }
    let signal = [9, 10, 11].map(|o| r.byte(b + o) as i8).into_iter().max().unwrap_or(-128);
    let rx_channel = ((r.u16(b + 22) >> 3) & 0xFF) as u8;
    let mut bssid = [0u8; 6]; for (i, x) in bssid.iter_mut().enumerate() { *x = r.byte(start + 16 + i); }
    let capability = r.u16(start + 24 + 10);
    let (mut ssid, mut ssid_len, mut channel, mut security) = ([0u8; 32], 0, rx_channel, if capability & 0x10 != 0 { 1 } else { 0 });
    let mut at = start + 24 + 12;
    while at + 2 <= end {
        let (id, len) = (r.byte(at), r.byte(at + 1) as usize);
        if at + 2 + len > end { break; }
        match id {
            0 if len <= 32 => { ssid_len = len; for i in 0..len { ssid[i] = r.byte(at + 2 + i); } }
            3 if len == 1 => channel = r.byte(at + 2),
            48 => security = 3,
            221 if len >= 4 && [r.byte(at + 2), r.byte(at + 3), r.byte(at + 4), r.byte(at + 5)] == [0x00, 0x50, 0xF2, 0x01] && security < 2 => security = 2,
            _ => {}
        }
        at += 2 + len;
    }
    let entry = match seen.iter().position(|n| n.frames != 0 && n.bssid == bssid) {
        Some(i) => &mut seen[i],
        None => match seen.iter().position(|n| n.frames == 0) { Some(i) => &mut seen[i], None => return },
    };
    if entry.frames == 0 { *entry = Network { bssid, ssid, ssid_len, channel, signal, security, frames: 0 }; }
    entry.frames += 1;
    if signal > entry.signal { entry.signal = signal; }
}

// Completed slots from `next` up to the engine's current one, then the index set one short of it.
fn drain(c: &Core, r: &mut Ring, seen: &mut [Network; SEEN], counts: &mut [u32; 5]) {
    fence(Ordering::SeqCst);
    let current = ((c.r32(RX_STATUS) & 0x1FFF) as usize / 16) % SLOTS;
    while r.next != current {
        let slot = r.next;
        frame(r, slot, seen, counts);
        r.poison(slot);
        r.next = (slot + 1) % SLOTS;
    }
    fence(Ordering::SeqCst);
    c.w32(RX_INDEX, (((current + SLOTS - 1) % SLOTS) * 16) as u32);
    let reason = c.r32(DMA0_REASON);
    if reason != 0 { c.w32(DMA0_REASON, reason); if reason & 0xDC00 != 0 { counts[3] += 1 << 16; } }
}

fn step(n: u32, what: &str) { mind::println!("[BCM] STAGE 4, STEP {}: {}", n, what); mind::time::sleep(crate::STEP_MS); }

// The stages, after stage 3 left the MAC suspended with the microcode running; `mac` is the station's address. Both
// engines are stopped before return.
pub fn run(c: &Core, mac: [u8; 6]) {
    step(1, "THE RECEIVE RING SET UP AND THE ENGINE STARTED");
    let iost = c.r32(AGENT_IO_STATUS);
    if iost & IO_STATUS_DMA64 == 0 { mind::println!("[BCM] IO STATUS {:08X}: NO 64-BIT DMA ENGINE, STAGE 4 NOT ATTEMPTED", iost); return; }
    let Some(mut r) = ring() else { return };
    let ring_address = r.dma.physical(r.ring) as u32;
    c.w32(RX_CONTROL, RX_ENABLE); c.w32(RX_RING_LOW, ring_address); c.w32(RX_RING_HIGH, TRANSLATION);
    c.w32(RX_INDEX, ((SLOTS - 1) * 16) as u32);
    c.w32(RX_LAZY, 0x0100_0000); c.w32(DMA0_MASK, 0x0001_FC00);
    // Receive only: no address of ours to answer to, no probe responses, beacons of every network.
    c.w16(0x420, 0x0020); for _ in 0..3 { c.w16(0x422, 0); }
    c.shm16_w(0x0074, 1);
    c.w32(MAC_CONTROL, (c.r32(MAC_CONTROL) & !MAC_CLEAR) | MAC_SET);
    mind::println!("[BCM] RING AT {:#X} ({} SLOTS OF {} BYTES); RX CONTROL {:08X}, STATUS {:08X}, ERROR {:08X}",
                   ring_address, SLOTS, BUFFER, c.r32(RX_CONTROL), c.r32(RX_STATUS), c.r32(RX_ERROR));

    step(2, "A SCAN OF CHANNELS 1 TO 13, 200 MS EACH, RECEIVING ONLY");
    let mut seen = [NONE; SEEN];
    let mut total = [0u32; 5];
    for ch in 1..=13 {
        if !c.suspend() { mind::println!("[BCM] THE MAC DOES NOT SUSPEND FOR CHANNEL {}", ch); break; }
        c.channel(ch);
        c.w32(MAC_CONTROL, (c.r32(MAC_CONTROL) & !MAC_CLEAR) | MAC_SET);
        c.enable();
        let mut counts = [0u32; 5];
        let end = mind::time::uptime_ms() + DWELL_MS;
        while mind::time::uptime_ms() < end { drain(c, &mut r, &mut seen, &mut counts); mind::time::sleep(2); }
        drain(c, &mut r, &mut seen, &mut counts);
        mind::println!("[BCM] CHANNEL {}: {} BEACONS, {} OTHER FRAMES, {} WITH A BAD FCS, {} DROPPED, {} DMA ERRORS; RX STATUS {:08X}, ERROR {:08X}",
                       ch, counts[0], counts[1], counts[2], counts[3] & 0xFFFF, counts[3] >> 16, c.r32(RX_STATUS), c.r32(RX_ERROR));
        for i in 0..5 { total[i] += counts[i]; }
    }
    let suspended = c.suspend();

    step(3, "THE NETWORKS HEARD");
    let heard = list(&seen);
    mind::println!("[BCM] SCAN DONE: {} NETWORKS FROM {} BEACONS; {} OTHER FRAMES, {} BAD FCS; MAC SUSPENDED: {}",
                   heard, total[0], total[1], total[2], if suspended { "YES" } else { "NO" });
    if suspended { transmit(c, &mut r, &mut seen, mac); }

    step(7, "THE ENGINES STOPPED");
    c.suspend();
    c.w32(TX_CONTROL, 0);
    let end = mind::time::uptime_ms() + 10;
    let mut tx_state = c.r32(TX_STATUS) >> 28;
    while tx_state != 0 && mind::time::uptime_ms() < end { mind::time::sleep(1); tx_state = c.r32(TX_STATUS) >> 28; }
    c.w32(TX_RING_LOW, 0); c.w32(TX_RING_HIGH, 0);
    mind::println!("[BCM] TRANSMIT ENGINE {} (STATE {})", if tx_state == 0 { "DISABLED" } else { "NOT DISABLED IN 10 MS" }, tx_state);
    c.w32(RX_CONTROL, 0);
    let end = mind::time::uptime_ms() + 10;
    let mut state = c.r32(RX_STATUS) >> 28;
    while state != 0 && mind::time::uptime_ms() < end { mind::time::sleep(1); state = c.r32(RX_STATUS) >> 28; }
    c.w32(RX_RING_LOW, 0); c.w32(RX_RING_HIGH, 0);
    mind::println!("[BCM] RECEIVE ENGINE {} (STATE {})", if state == 0 { "DISABLED" } else { "NOT DISABLED IN 10 MS" }, state);
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

// The transmit status FIFO's next final entry (0x170 then 0x174), waited for up to `ms`.
fn tx_status(c: &Core, ms: usize) -> Option<(u32, u32)> {
    let end = mind::time::uptime_ms() + ms;
    loop {
        let first = c.r32(TX_STATUS0);
        if first != u32::MAX && first & 1 != 0 {
            let second = c.r32(TX_STATUS1);
            if first & 0x40 == 0 { return Some((first, second)); } // intermediate entries are skipped
            continue;
        }
        if mind::time::uptime_ms() >= end { return None; }
        mind::time::sleep(1);
    }
}

// A broadcast probe request (45 bytes) from `mac` on `ch`, with its 118-byte transmit header, in transmit slot `slot`:
// 1 Mbit/s CCK, the microcode filling the sequence number and the FCS.
fn probe(r: &mut Ring, slot: usize, mac: [u8; 6], ch: u8) -> usize {
    let mut frame = [0u8; 45];
    frame[0] = 0x40;
    frame[4..10].copy_from_slice(&[0xFF; 6]); frame[10..16].copy_from_slice(&mac); frame[16..22].copy_from_slice(&[0xFF; 6]);
    frame[26..36].copy_from_slice(&[0x01, 0x08, 0x02, 0x04, 0x0B, 0x16, 0x0C, 0x12, 0x18, 0x24]);
    frame[36..42].copy_from_slice(&[0x32, 0x04, 0x30, 0x48, 0x60, 0x6C]);
    frame[42..45].copy_from_slice(&[0x03, 0x01, ch]);
    // The PLCP for 1 Mbit/s: SIGNAL 0x0A, SERVICE 0x04, LENGTH in microseconds of the frame and its FCS.
    let micros = ((frame.len() + 4) * 8) as u16;
    let plcp = [0x0A, 0x04, micros as u8, (micros >> 8) as u8, 0, 0];
    let mut header = [0u8; HEADER];
    header[0x00..0x04].copy_from_slice(&(0x02u32 | 0x08 | 0x10).to_le_bytes()); // long frame, start of MSDU, hardware sequence
    header[0x04..0x06].copy_from_slice(&0x0040u16.to_le_bytes());
    header[0x08..0x0A].copy_from_slice(&0x00C0u16.to_le_bytes()); // CCK, antennas 0/1 automatic, no per-frame power
    header[0x0A..0x0C].copy_from_slice(&0x0002u16.to_le_bytes()); header[0x0C..0x0E].copy_from_slice(&0x0002u16.to_le_bytes());
    header[0x12] = 0x0A; header[0x14] = 0; header[0x15] = ch;
    header[0x26..0x2C].copy_from_slice(&[0xFF; 6]);
    header[0x36..0x3C].copy_from_slice(&plcp);
    header[0x4C..0x4E].copy_from_slice(&(0x2000u16 | slot as u16).to_le_bytes());
    header[0x70..0x76].copy_from_slice(&plcp);
    let at = r.tx_buffers + slot * TX_STRIDE;
    r.dma.bytes(at, HEADER).copy_from_slice(&header);
    r.dma.bytes(at + HEADER, frame.len()).copy_from_slice(&frame);
    let d = r.tx_ring + 16 * slot;
    let address = r.dma.physical(at) as u32;
    r.dma.write32(d + 4, (HEADER + frame.len()) as u32);
    r.dma.write32(d + 8, address);
    r.dma.write32(d + 12, TRANSLATION);
    r.dma.write32(d, SOF | EOF | IOC | if slot == TX_SLOTS - 1 { EOT } else { 0 });
    HEADER + frame.len()
}

// Stage 5 (550-DRV-0026): a low fixed gain, the transmit ring on engine 1, one probe request on each of channels 1 to
// 11, the answers received. The receive ring keeps running throughout.
fn transmit(c: &Core, r: &mut Ring, seen: &mut [Network; SEEN], mac: [u8; 6]) {
    step(4, "A LOW FIXED TRANSMIT GAIN, THE TRANSMIT RING ON ENGINE 1");
    c.gain.set(TX_GAIN);
    let entry = c.tx_power_fix();
    c.phy_clear(0x1E7, 0xE000); // power control off: the fixed gain stands
    // QoS off (the BCM4331 sends on engine 1 only); the microcode's own frames at CCK, antennas automatic.
    c.shm16_w(0x005E, c.shm16(0x005E) & !0x0100);
    c.w16(IFS_CONTROL, c.r16(IFS_CONTROL) & !0x0004);
    for offset in [0x0054, 0x0022, 0x0188] { c.shm16_w(offset, 0xFCC0); }
    // Our address in the address match now, so the probe responses addressed to it are kept.
    c.w16(0x420, 0x0020); for k in 0..3 { c.w16(0x422, (mac[2 * k + 1] as u16) << 8 | mac[2 * k] as u16); }
    let tx_address = r.dma.physical(r.tx_ring) as u32;
    c.w32(TX_CONTROL, TX_ENABLE); c.w32(TX_RING_LOW, tx_address); c.w32(TX_RING_HIGH, TRANSLATION);
    let mut stale = 0;
    while c.r32(TX_STATUS0) & 1 != 0 && stale < 64 { let _ = c.r32(TX_STATUS1); stale += 1; }
    mind::println!("[BCM] TRANSMIT GAIN FROM ENTRY {} ({:08X}); PHY 1E7 {:04X}; TX RING AT {:#X}, CONTROL {:08X}, STATUS {:08X}; {} STALE STATUSES DRAINED",
                   TX_GAIN, entry, c.phy(0x1E7), tx_address, c.r32(TX_CONTROL), c.r32(TX_STATUS), stale);

    step(5, "AN ACTIVE SCAN: ONE BROADCAST PROBE REQUEST ON EACH OF CHANNELS 1 TO 11");
    let mut answers = 0;
    for ch in 1..=TX_CHANNELS {
        if !c.suspend() { mind::println!("[BCM] THE MAC DOES NOT SUSPEND FOR CHANNEL {}", ch); break; }
        c.channel(ch);
        c.w32(MAC_CONTROL, (c.r32(MAC_CONTROL) & !MAC_CLEAR) | MAC_SET);
        c.enable();
        let slot = r.tx_next;
        let bytes = probe(r, slot, mac, ch as u8);
        r.tx_next = (slot + 1) % TX_SLOTS;
        fence(Ordering::SeqCst);
        c.w32(TX_INDEX, (r.tx_next * 16) as u32);
        let status = tx_status(c, 100);
        c.w32(IRQ_REASON, IRQ_TX_STATUS);
        let mut counts = [0u32; 5];
        let end = mind::time::uptime_ms() + ANSWER_MS;
        while mind::time::uptime_ms() < end { drain(c, r, seen, &mut counts); mind::time::sleep(2); }
        drain(c, r, seen, &mut counts);
        answers += counts[4];
        match status {
            Some((first, second)) => {
                let word = first & 0xFFFF;
                mind::println!("[BCM] CHANNEL {}: PROBE REQUEST ({} BYTES) COOKIE {:04X}: SENT {} TIMES, SUPPRESSED FOR REASON {}, SEQUENCE {}, PHY STATUS {:02X}; {} PROBE RESPONSES, {} BEACONS",
                               ch, bytes, first >> 16, word >> 12 & 0xF, word >> 2 & 7, second & 0xFFFF, second >> 16 & 0xFF, counts[4], counts[0]);
            }
            None => mind::println!("[BCM] CHANNEL {}: NO TRANSMIT STATUS IN 100 MS; TX STATUS {:08X}, ERROR {:08X}; {} PROBE RESPONSES",
                                   ch, c.r32(TX_STATUS), c.r32(TX_ERROR), counts[4]),
        }
    }
    c.suspend();

    step(6, "THE NETWORKS HEARD, WITH THE ANSWERS");
    let heard = list(seen);
    mind::println!("[BCM] ACTIVE SCAN DONE: {} PROBE RESPONSES; {} NETWORKS IN ALL", answers, heard);
}
