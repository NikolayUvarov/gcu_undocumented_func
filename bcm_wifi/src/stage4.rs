// Stage 4 (550-DRV-0025): frames received by the 802.11 core's DMA engine into a ring in the region init grants,
// polled (no interrupt), and the networks around listed from their beacons and probe responses on channels 1 to 13.
// Nothing is sent: no probe request, and the station's own address is cleared from the address match, so no frame is
// addressed to the card. Register facts come from the stage-4 fact sheet (Linux's b43 and brcmsmac, described).
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
// MAC control: station, beacons of every network, awake, PMQ discarded; AP, the keep-bad bits and power save off.
const MAC_SET: u32 = 1 << 17 | 1 << 20 | 1 << 26 | 1 << 30; const MAC_CLEAR: u32 = 1 << 18 | 1 << 21 | 1 << 22 | 1 << 23 | 1 << 25;
// The ring: 8 KiB-aligned (bit 12 of its address must stay clear on the BCM4331); 128 buffers of 2390 bytes, 64-aligned.
const RING_BYTES: usize = 8192; const SLOTS: usize = 128; const BUFFER: u32 = 2390; const STRIDE: usize = 2432;
const OFFSET: usize = 38; const DWELL_MS: usize = 200;

struct Network { bssid: [u8; 6], ssid: [u8; 32], ssid_len: usize, channel: u8, signal: i8, security: u8, frames: u32 }
const NONE: Network = Network { bssid: [0; 6], ssid: [0; 32], ssid_len: 0, channel: 0, signal: -128, security: 0, frames: 0 };
const SEEN: usize = 48;

struct Ring { dma: Dma, ring: usize, buffers: usize, next: usize }

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
    if buffers + SLOTS * STRIDE > dma.len() || dma.physical(dma.len() - 1) >> 32 != 0 {
        mind::println!("[BCM] THE DMA REGION ({} KiB AT {:#X}) CANNOT HOLD THE RING BELOW 4 GiB: STAGE 4 NOT ATTEMPTED", dma.len() / 1024, base);
        return None;
    }
    dma.zero(ring, RING_BYTES);
    let mut r = Ring { dma, ring, buffers, next: 0 };
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
fn frame(r: &Ring, slot: usize, seen: &mut [Network; SEEN], counts: &mut [u32; 4]) {
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
    counts[0] += 1;
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
fn drain(c: &Core, r: &mut Ring, seen: &mut [Network; SEEN], counts: &mut [u32; 4]) {
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

// The stage, after stage 3 left the MAC suspended with the microcode running. The engine is stopped before return.
pub fn run(c: &Core) {
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
    let mut total = [0u32; 4];
    for ch in 1..=13 {
        if !c.suspend() { mind::println!("[BCM] THE MAC DOES NOT SUSPEND FOR CHANNEL {}", ch); break; }
        c.channel(ch);
        c.w32(MAC_CONTROL, (c.r32(MAC_CONTROL) & !MAC_CLEAR) | MAC_SET);
        c.enable();
        let mut counts = [0u32; 4];
        let end = mind::time::uptime_ms() + DWELL_MS;
        while mind::time::uptime_ms() < end { drain(c, &mut r, &mut seen, &mut counts); mind::time::sleep(2); }
        drain(c, &mut r, &mut seen, &mut counts);
        mind::println!("[BCM] CHANNEL {}: {} BEACONS, {} OTHER FRAMES, {} WITH A BAD FCS, {} DROPPED, {} DMA ERRORS; RX STATUS {:08X}, ERROR {:08X}",
                       ch, counts[0], counts[1], counts[2], counts[3] & 0xFFFF, counts[3] >> 16, c.r32(RX_STATUS), c.r32(RX_ERROR));
        for i in 0..4 { total[i] += counts[i]; }
    }
    let suspended = c.suspend();

    step(3, "THE NETWORKS HEARD");
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
    mind::println!("[BCM] SCAN DONE: {} NETWORKS FROM {} BEACONS AND PROBE RESPONSES; {} OTHER FRAMES, {} BAD FCS; MAC SUSPENDED: {}",
                   heard, total[0], total[1], total[2], if suspended { "YES" } else { "NO" });

    step(4, "THE RECEIVE ENGINE STOPPED");
    c.w32(RX_CONTROL, 0);
    let end = mind::time::uptime_ms() + 10;
    let mut state = c.r32(RX_STATUS) >> 28;
    while state != 0 && mind::time::uptime_ms() < end { mind::time::sleep(1); state = c.r32(RX_STATUS) >> 28; }
    c.w32(RX_RING_LOW, 0); c.w32(RX_RING_HIGH, 0);
    mind::println!("[BCM] RECEIVE ENGINE {} (STATE {})", if state == 0 { "DISABLED" } else { "NOT DISABLED IN 10 MS" }, state);
}
