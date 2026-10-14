// The radio kept running for the station (550-DRV-0027): the calls idl/wlanhw.wit 1.0 names, over the receive ring of
// stage 4 and the transmit ring of stage 5. Received frames are filtered as the interface says and queued, each led by
// its 8-byte header; a frame is sent as the station built it (its sequence number and duration kept) at the rate asked,
// and its transmit status read back. Register facts come from the stage-4 and stage-5 fact sheets (b43 and brcmsmac,
// described).
use crate::stage3::Core;
use core::sync::atomic::{fence, Ordering};
use mind::abi::SLOT_MEM;
use mind::dev::Dma;
use mind::mem::Pages;

// The receive engine (engine 0 of the 64-bit layout) and its interrupt reason.
const RX_CONTROL: usize = 0x220; const RX_INDEX: usize = 0x224; const RX_RING_LOW: usize = 0x228; const RX_RING_HIGH: usize = 0x22C;
const RX_STATUS: usize = 0x230; const RX_ERROR: usize = 0x234; const DMA0_REASON: usize = 0x20; const DMA0_MASK: usize = 0x24;
const RX_LAZY: usize = 0x100; const MAC_CONTROL: usize = 0x120; const AGENT_IO_STATUS: usize = 0x1500;
const IO_STATUS_DMA64: u32 = 1 << 12;
// Frame offset 38, enabled, parity check off; the PCIe translation in the high address word.
const RX_ENABLE: u32 = 38 << 1 | 1 | 0x800; const TRANSLATION: u32 = 0x8000_0000;
// MAC control: station, beacons of every network (filtered here), awake, PMQ discarded; AP, the keep-bad bits, power
// save and discarding transmit status off.
const MAC_SET: u32 = 1 << 17 | 1 << 20 | 1 << 26 | 1 << 30; const MAC_KEEP_CONTROL: u32 = 1 << 22; const MAC_KEEP_BAD: u32 = 1 << 21 | 1 << 23; const MAC_CLEAR: u32 = 1 << 18 | 1 << 21 | 1 << 22 | 1 << 23 | 1 << 25 | 1 << 29;
// The receive ring: 8 KiB-aligned (bit 12 of its address must stay clear on the BCM4331); 128 buffers of 2390 bytes.
const RING_BYTES: usize = 8192; const SLOTS: usize = 128; const BUFFER: u32 = 2390; const STRIDE: usize = 2432; const OFFSET: usize = 38;
// The transmit engine (engine 1: the BCM4331 sends everything there), the transmit status FIFO; slots hold a header and a frame.
const TX_CONTROL: usize = 0x240; const TX_INDEX: usize = 0x244; const TX_RING_LOW: usize = 0x248; const TX_RING_HIGH: usize = 0x24C;
const TX_STATUS: usize = 0x250; const TX_STATUS0: usize = 0x170; const TX_STATUS1: usize = 0x174;
const TX_ENABLE: u32 = 1 | 0x800; const TX_SLOTS: usize = 16; const TX_STRIDE: usize = 2560; const HEADER: usize = 118;
const IRQ_REASON: usize = 0x128; const IRQ_TX_STATUS: u32 = 1 << 29; const IFS_CONTROL: usize = 0x688;
// Descriptor flags: start and end of frame, interrupt on completion, end of table.
const SOF: u32 = 1 << 31; const EOF: u32 = 1 << 30; const IOC: u32 = 1 << 29; const EOT: u32 = 1 << 28;
// MAC transmit control: an acknowledgement expected, the first fragment of an MSDU.
const TXC_ACK: u32 = 0x01; const TXC_START: u32 = 0x08;
// The receive-match slots of the station's own address and of the network's BSSID; the template copy of both.
const MATCH_CONTROL: usize = 0x420; const MATCH_DATA: usize = 0x422; const TEMPLATE_CONTROL: usize = 0x130; const TEMPLATE_DATA: usize = 0x134;
// The gain table entry for transmitting, b43's: at entry 80 no access point acknowledged a frame (2026-10-13). A higher
// entry is a lower gain; 550-DRV-0028 replaces the fixed entry.
pub const TX_GAIN: u16 = 40;
// The longest frame without FCS (idl/wlanhw.wit), the header that leads a received one, the frames kept for the station.
pub const FRAME_MAX: usize = 2346; pub const RADIO_HEADER: usize = 8;
const QUEUE: usize = 32; const QUEUE_STRIDE: usize = 2368;
const STATUS_MS: usize = 100;
// The cores a frame leaves through (PHY control bits 6 up, core 0 at bit 6): b43's cores 0 and 1, in SISO mode. Single
// cores and CDD on three did no better on 2026-10-14.
const TX_CORES: u16 = 0x00C0;

// Why a call failed, as idl/wlanhw.wit names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { NoDevice, Empty, Invalid, Failed }

// What passes: every beacon and probe response (a scan); frames to `own` (zero: the radio's address); group frames of
// the network `bssid` (zero: none).
#[derive(Clone, Copy, Default)]
pub struct Filter { pub all_beacons: bool, pub bssid: [u8; 6], pub own: [u8; 6] }

// A frame's transmit status: acknowledged, and how many times it went out.
#[derive(Clone, Copy)]
pub struct Sent { pub acked: bool, pub attempts: u8 }

// What the receive side did since the radio came up. `frames`: by modulation (CCK, OFDM, HT) and outcome (good, bad
// FCS, bad PLCP; bad ones reach the host only when kept, `keep`). The acknowledgements to us among the good frames
// (control frames kept), and the per-core powers summed and the clipped count of good OFDM frames.
#[derive(Clone, Copy, Default)]
pub struct Counts {
    pub queued: u32, pub filtered: u32, pub bad: u32, pub dma_errors: u32, pub frames: [[u32; 3]; 3], pub acks_cck: u32, pub acks_ofdm: u32,
    pub ofdm_power: [i32; 3], pub ofdm_clipped: u32,
}

pub struct Radio<'a> {
    c: &'a Core<'a>,
    dma: Dma, buffers: usize, next: usize, tx_ring: usize, tx_buffers: usize, tx_next: usize,
    pub mac: [u8; 6],
    channel: u8,
    filter: Filter,
    queue: Pages, head: usize, count: usize,
    retry: u16,
    // MAC control bits a test adds: control frames, bad frames kept for the host.
    mac_extra: u32,
    // The cores' front-end words before a test switched one off.
    afe_saved: Option<[(u16, u16); 3]>,
    // Table 2's words at offsets 8 to 11 before a test changed them.
    table2_saved: Option<[u16; 4]>,
    staging: [u8; STRIDE],
    pub counts: Counts,
}

// The rate in 500 kb/s units as the hardware takes it: whether OFDM, the PLCP rate code, PHY control word 1.
fn rate_code(rate: u8) -> Option<(bool, u8, u16)> {
    Some(match rate {
        2 => (false, 0x0A, 0x0002), 4 => (false, 0x14, 0x0002), 11 => (false, 0x37, 0x0002), 22 => (false, 0x6E, 0x0002),
        12 => (true, 0xB, 0x0002), 18 => (true, 0xF, 0x0202), 24 => (true, 0xA, 0x0802), 36 => (true, 0xE, 0x0A02),
        48 => (true, 0x9, 0x1002), 72 => (true, 0xD, 0x1202), 96 => (true, 0x8, 0x1902), 108 => (true, 0xC, 0x1A02),
        _ => return None,
    })
}

// The received rate in 500 kb/s units from the frame type and the PLCP's first byte; 0 for HT or an unknown code.
fn rx_rate(ofdm: bool, plcp0: u8) -> u8 {
    if ofdm {
        match plcp0 & 0xF { 0xB => 12, 0xF => 18, 0xA => 24, 0xE => 36, 0x9 => 48, 0xD => 72, 0x8 => 96, 0xC => 108, _ => 0 }
    } else {
        match plcp0 { 0x0A => 2, 0x14 => 4, 0x37 => 11, 0x6E => 22, _ => 0 }
    }
}

// The PLCP header for `len` frame bytes (the FCS added) at `rate`.
fn plcp(ofdm: bool, code: u8, rate: u8, len: usize) -> [u8; 6] {
    let psdu = len + 4;
    if ofdm {
        let signal = code as u32 | (psdu as u32) << 5;
        return [signal as u8, (signal >> 8) as u8, (signal >> 16) as u8, 0, 0, 0];
    }
    let micros = (psdu * 16).div_ceil(rate as usize) as u16;
    // At 11 Mbit/s the length extension bit marks a duration rounded up by more than one byte's worth.
    let extension = rate == 22 && (1..=3).contains(&(psdu * 8 % 11));
    [code, 0x04 | if extension { 0x80 } else { 0 }, micros as u8, (micros >> 8) as u8, 0, 0]
}

impl<'a> Radio<'a> {
    // The rings in the granted region, both engines started, the station's address matched, a fixed gain, channel 1;
    // the MAC is left enabled. None (logged) when the region or the engine does not fit.
    pub fn up(c: &'a Core<'a>, mac: [u8; 6]) -> Option<Self> {
        let iost = c.r32(AGENT_IO_STATUS);
        if iost & IO_STATUS_DMA64 == 0 { mind::println!("[BCM] IO STATUS {:08X}: NO 64-BIT DMA ENGINE", iost); return None; }
        let Ok(mut dma) = Dma::map(SLOT_MEM) else { mind::println!("[BCM] NO DMA REGION GRANTED"); return None };
        let base = dma.physical(0) as usize;
        let ring = (RING_BYTES - base % RING_BYTES) % RING_BYTES;
        let buffers = ring + RING_BYTES;
        let tx_ring = ring + (RING_BYTES + SLOTS * STRIDE).div_ceil(RING_BYTES) * RING_BYTES;
        let tx_buffers = tx_ring + RING_BYTES;
        if tx_buffers + TX_SLOTS * TX_STRIDE > dma.len() || dma.physical(dma.len() - 1) >> 32 != 0 {
            mind::println!("[BCM] THE DMA REGION ({} KiB AT {:#X}) CANNOT HOLD THE RINGS BELOW 4 GiB", dma.len() / 1024, base);
            return None;
        }
        let Some(queue) = Pages::new(QUEUE * QUEUE_STRIDE) else { mind::println!("[BCM] NO MEMORY FOR THE RECEIVE QUEUE"); return None };
        dma.zero(ring, RING_BYTES); dma.zero(tx_ring, RING_BYTES);
        let mut r = Radio { c, dma, buffers, next: 0, tx_ring, tx_buffers, tx_next: 0, mac, channel: 0, filter: Filter::default(),
                            queue, head: 0, count: 0, retry: 0, mac_extra: 0, afe_saved: None, table2_saved: None, staging: [0; STRIDE], counts: Counts::default() };
        for slot in 0..SLOTS {
            r.poison(slot);
            let (d, address) = (ring + 16 * slot, r.dma.physical(r.buffer(slot)) as u32);
            r.dma.write32(d, if slot == SLOTS - 1 { EOT } else { 0 });
            r.dma.write32(d + 4, BUFFER);
            r.dma.write32(d + 8, address);
            r.dma.write32(d + 12, TRANSLATION);
        }
        fence(Ordering::SeqCst);

        if !c.suspend() { mind::println!("[BCM] THE MAC DOES NOT SUSPEND FOR THE RINGS"); return None; }
        let (rx_address, tx_address) = (r.dma.physical(ring) as u32, r.dma.physical(tx_ring) as u32);
        c.w32(RX_CONTROL, RX_ENABLE); c.w32(RX_RING_LOW, rx_address); c.w32(RX_RING_HIGH, TRANSLATION);
        c.w32(RX_INDEX, ((SLOTS - 1) * 16) as u32);
        c.w32(RX_LAZY, 0x0100_0000); c.w32(DMA0_MASK, 0x0001_FC00);
        // A low fixed gain with power control off; QoS off (engine 1 only); the microcode's own frames at CCK.
        c.gain.set(TX_GAIN);
        let entry = c.tx_power_fix();
        c.phy_clear(0x1E7, 0xE000);
        c.shm16_w(0x005E, c.shm16(0x005E) & !0x0100);
        c.w16(IFS_CONTROL, c.r16(IFS_CONTROL) & !0x0004);
        for offset in [0x0054, 0x0022, 0x0188] { c.shm16_w(offset, 0xFCC0); }
        c.w32(TX_CONTROL, TX_ENABLE); c.w32(TX_RING_LOW, tx_address); c.w32(TX_RING_HIGH, TRANSLATION);
        let mut stale = 0;
        while c.r32(TX_STATUS0) & 1 != 0 && stale < 64 { let _ = c.r32(TX_STATUS1); stale += 1; }
        r.addresses(&mac, &[0; 6]);
        c.w32(MAC_CONTROL, (c.r32(MAC_CONTROL) & !MAC_CLEAR) | MAC_SET | r.mac_extra);
        c.enable();
        mind::println!("[BCM] RADIO UP: RX RING AT {:#X} ({} SLOTS), TX RING AT {:#X} ({} SLOTS); GAIN ENTRY {} ({:08X}); {} STALE TRANSMIT STATUSES; RX STATUS {:08X}, ERROR {:08X}",
                       rx_address, SLOTS, tx_address, TX_SLOTS, TX_GAIN, entry, stale, c.r32(RX_STATUS), c.r32(RX_ERROR));
        if r.set_channel(1).is_err() { mind::println!("[BCM] CHANNEL 1 NOT SET"); }
        Some(r)
    }

    fn buffer(&self, slot: usize) -> usize { self.buffers + slot * STRIDE }
    // Length zeroed and the 8 bytes at the frame offset set to 0xFF, so a buffer the engine did not write is known.
    fn poison(&mut self, slot: usize) {
        let b = self.buffer(slot);
        let first = self.dma.read32(b) & 0xFFFF_0000; self.dma.write32(b, first);
        self.dma.write32(b + 36, (self.dma.read32(b + 36) & 0x0000_FFFF) | 0xFFFF_0000);
        self.dma.write32(b + 40, 0xFFFF_FFFF);
        self.dma.write32(b + 44, (self.dma.read32(b + 44) & 0xFFFF_0000) | 0x0000_FFFF);
    }

    // The own address in match slot 0 and the BSSID in slot 3, both in the template; with the MAC suspended.
    fn addresses(&self, own: &[u8; 6], bssid: &[u8; 6]) {
        let c = self.c;
        for (slot, a) in [(0u16, own), (3, bssid)] {
            c.w16(MATCH_CONTROL, slot | 0x0020);
            for k in 0..3 { c.w16(MATCH_DATA, (a[2 * k + 1] as u16) << 8 | a[2 * k] as u16); }
        }
        let mut bytes = [0u8; 12]; bytes[..6].copy_from_slice(own); bytes[6..].copy_from_slice(bssid);
        let swap = c.r32(MAC_CONTROL) & (1 << 16) != 0;
        for k in 0..3 {
            let word = u32::from_le_bytes([bytes[4 * k], bytes[4 * k + 1], bytes[4 * k + 2], bytes[4 * k + 3]]);
            c.w32(TEMPLATE_CONTROL, 0x20 + 4 * k as u32); c.w32(TEMPLATE_DATA, if swap { word.swap_bytes() } else { word });
        }
    }

    // The channel set (0: none yet) and how many received frames wait.
    pub fn channel(&self) -> u8 { self.channel }
    pub fn waiting(&self) -> usize { self.count }

    // Tunes to a 2.4 GHz channel at 20 MHz with the MAC suspended meanwhile.
    pub fn set_channel(&mut self, ch: u8) -> Result<(), Error> {
        if !(1..=13).contains(&ch) { return Err(Error::Invalid); }
        if !self.c.suspend() { return Err(Error::Failed); }
        self.c.channel(ch as usize);
        self.c.w32(MAC_CONTROL, (self.c.r32(MAC_CONTROL) & !MAC_CLEAR) | MAC_SET | self.mac_extra);
        self.c.enable();
        self.channel = ch;
        Ok(())
    }

    // The filter, and the addresses the hardware acknowledges and follows.
    pub fn set_filter(&mut self, filter: Filter) -> Result<(), Error> {
        let own = if filter.own == [0; 6] { self.mac } else { filter.own };
        if !self.c.suspend() { return Err(Error::Failed); }
        self.addresses(&own, &filter.bssid);
        self.c.enable();
        self.filter = filter;
        Ok(())
    }

    // A transmit gain entry for a test (550-DRV-0028 replaces it); the entry read back.
    pub fn set_gain(&mut self, entry: u16) -> Result<u32, Error> {
        if !self.c.suspend() { return Err(Error::Failed); }
        self.c.gain.set(entry);
        let t = self.c.tx_power_fix();
        self.c.enable();
        Ok(t)
    }

    // Control frames, and frames with a bad FCS or PLCP, passed to the host as well, for a test.
    pub fn keep(&mut self, control: bool, bad: bool) {
        self.mac_extra = if control { MAC_KEEP_CONTROL } else { 0 } | if bad { MAC_KEEP_BAD } else { 0 };
        self.c.w32(MAC_CONTROL, (self.c.r32(MAC_CONTROL) & !MAC_CLEAR) | MAC_SET | self.mac_extra);
    }

    // For a test: every core's front end as it was, then `core`'s switched off; None leaves all three on.
    pub fn core_off(&mut self, core: Option<u16>) -> Result<(), Error> {
        if core.is_none() && self.afe_saved.is_none() { return Ok(()); }
        if !self.c.suspend() { return Err(Error::Failed); }
        let saved = *self.afe_saved.get_or_insert_with(|| [0, 1, 2].map(|i| self.c.afe(i)));
        for (i, words) in saved.into_iter().enumerate() { self.c.afe_restore(i as u16, words); }
        if let Some(i) = core.filter(|&i| i < 3) { self.c.afe_off(i); }
        self.c.enable();
        Ok(())
    }

    // For a test: PHY table 2 at offsets 8 to 11 given b43's gain values of tables 0, 1 and 0x28, or its own back.
    pub fn table2_gains(&mut self, fixed: bool) -> Result<(), Error> {
        if !fixed && self.table2_saved.is_none() { return Ok(()); }
        if !self.c.suspend() { return Err(Error::Failed); }
        let saved = *self.table2_saved.get_or_insert_with(|| [8, 9, 10, 11].map(|o| self.c.table16_read(2, o)));
        self.c.table16(2, 8, if fixed { &[9, 14, 19, 24] } else { &saved });
        self.c.enable();
        Ok(())
    }

    // What the MAC shows when it does not suspend: its control word, the interrupt reason, the microcode's status.
    pub fn state(&self) -> (u32, u32, u16) { (self.c.r32(MAC_CONTROL), self.c.r32(IRQ_REASON), self.c.shm16(0x0040)) }

    // The per-core gain values b43 writes at offsets 8 to 11 of PHY tables 0, 1 and 0x28, with table 2's beside them.
    // Read with the MAC suspended: the microcode uses the same table address registers.
    pub fn rx_gain_tables(&self) -> Option<[[u16; 4]; 4]> {
        if !self.c.suspend() { return None; }
        let t = [0, 1, 2, 0x28].map(|t| [8, 9, 10, 11].map(|o| self.c.table16_read(t, o)));
        self.c.enable();
        Some(t)
    }

    // Whether the filter lets `f` (an MPDU without FCS) through: control frames never do.
    fn wanted(&self, f: &[u8]) -> bool {
        if f.len() < 10 { return false; }
        let (kind, subtype, ds) = (f[0] >> 2 & 3, f[0] >> 4, f[1] & 3);
        if kind == 1 || kind == 3 { return false; }
        if kind == 0 && (subtype == 8 || subtype == 5) && self.filter.all_beacons { return true; }
        let own = if self.filter.own == [0; 6] { self.mac } else { self.filter.own };
        if f[4..10] == own { return true; }
        if f.len() < 24 || self.filter.bssid == [0; 6] || f[4] & 1 == 0 { return false; }
        let bss = match (kind, ds) { (0, _) | (2, 0) => &f[16..22], (2, 2) => &f[10..16], _ => return false };
        bss == self.filter.bssid
    }

    // Received frames moved from the ring into the queue, filtered, while the queue has room: the rest wait in the ring.
    // The engine's index is set one short of the first slot not yet taken.
    pub fn poll(&mut self) {
        fence(Ordering::SeqCst);
        let current = ((self.c.r32(RX_STATUS) & 0x1FFF) as usize / 16) % SLOTS;
        while self.next != current && self.count < QUEUE {
            let slot = self.next;
            self.take_slot(slot);
            self.poison(slot);
            self.next = (slot + 1) % SLOTS;
        }
        fence(Ordering::SeqCst);
        self.c.w32(RX_INDEX, (((self.next + SLOTS - 1) % SLOTS) * 16) as u32);
        let reason = self.c.r32(DMA0_REASON);
        if reason != 0 { self.c.w32(DMA0_REASON, reason); if reason & 0xDC00 != 0 { self.counts.dma_errors += 1; } }
    }

    // One completed buffer: its 24-byte receive header checked, the frame queued with the interface's header.
    fn take_slot(&mut self, slot: usize) {
        let b = self.buffer(slot);
        let length = (self.dma.read32(b) & 0xFFFF) as usize;
        if length == 0 || length + OFFSET > BUFFER as usize { self.counts.bad += 1; return; }
        for w in 0..(OFFSET + length).div_ceil(4) { self.staging[4 * w..4 * w + 4].copy_from_slice(&self.dma.read32(b + 4 * w).to_le_bytes()); }
        let s = &self.staging;
        let (phy0, status) = (u16::from_le_bytes([s[4], s[5]]), u32::from_le_bytes([s[16], s[17], s[18], s[19]]));
        if s[OFFSET..OFFSET + 8].iter().all(|&b| b == 0xFF) { self.counts.bad += 1; return; }
        // Counted by modulation and outcome first; FCS error, decryption error, PLCP violation or header check failure: dropped.
        let kind = match phy0 & 3 { 0 => 0, 1 => 1, _ => 2 };
        let outcome = if status & 1 != 0 { 1 } else if phy0 & 0x0300 != 0 { 2 } else { 0 };
        self.counts.frames[kind][outcome] += 1;
        if outcome != 0 || status & 1 << 4 != 0 { self.counts.bad += 1; return; }
        let pad = if status & 4 != 0 { 2 } else { 0 };
        if length < pad + 6 + 4 + 10 { self.counts.bad += 1; return; }
        let (plcp0, start, end) = (s[OFFSET + pad], OFFSET + pad + 6, OFFSET + length - 4);
        let ofdm = kind == 1;
        if ofdm {
            for i in 0..3 { self.counts.ofdm_power[i] += s[9 + i] as i8 as i32; }
            if phy0 & 0x000C != 0 { self.counts.ofdm_clipped += 1; }
        }
        let own = if self.filter.own == [0; 6] { self.mac } else { self.filter.own };
        if s[start] == 0xD4 && s[start + 4..start + 10] == own { if ofdm { self.counts.acks_ofdm += 1 } else { self.counts.acks_cck += 1 } }
        if !self.wanted(&s[start..end]) { self.counts.filtered += 1; return; }
        let signal = [s[9], s[10], s[11]].map(|v| v as i8).into_iter().max().unwrap_or(0);
        let weakness = if signal < 0 && signal > -128 { signal.unsigned_abs() } else { 0 };
        let channel = match (u16::from_le_bytes([s[22], s[23]]) >> 3) as u8 { 0 => self.channel, ch => ch };
        let len = end - start;
        let header = [len as u8, (len >> 8) as u8, weakness, channel, rx_rate(phy0 & 3 == 1, plcp0), self.filter.all_beacons as u8, 0, 0];
        let at = (self.head + self.count) % QUEUE * QUEUE_STRIDE;
        let q = self.queue.as_mut_slice();
        q[at..at + RADIO_HEADER].copy_from_slice(&header);
        q[at + RADIO_HEADER..at + RADIO_HEADER + len].copy_from_slice(&self.staging[start..end]);
        self.count += 1;
        self.counts.queued += 1;
    }

    // The oldest received frame, led by its header, copied into `out`; its length.
    pub fn take(&mut self, out: &mut [u8]) -> Result<usize, Error> {
        if self.count == 0 { return Err(Error::Empty); }
        let at = self.head * QUEUE_STRIDE;
        let q = self.queue.as_slice();
        let len = RADIO_HEADER + u16::from_le_bytes([q[at], q[at + 1]]) as usize;
        let out = out.get_mut(..len).ok_or(Error::Invalid)?;
        out.copy_from_slice(&q[at..at + len]);
        self.head = (self.head + 1) % QUEUE;
        self.count -= 1;
        Ok(len)
    }

    // Sends `frame` (an MPDU without FCS) on the current channel at `rate`, tried at most `attempts` times, and waits
    // for its transmit status. Receiving goes on meanwhile.
    pub fn send(&mut self, frame: &[u8], rate: u8, attempts: u8) -> Result<Sent, Error> {
        let Some((ofdm, code, control1)) = rate_code(rate) else { return Err(Error::Invalid) };
        if frame.len() < 10 || frame.len() > FRAME_MAX || !(1..=15).contains(&attempts) { return Err(Error::Invalid); }
        if self.channel == 0 { return Err(Error::NoDevice); }
        // The short retry limit, in scratch word 6, bounds the tries of a frame without the long-frame bit.
        if self.retry != attempts as u16 {
            if !self.c.suspend() { return Err(Error::Failed); }
            self.c.scratch_w(6, attempts as u16);
            self.c.enable();
            self.retry = attempts as u16;
        }
        let unicast = frame[4] & 1 == 0;
        let p = plcp(ofdm, code, rate, frame.len());
        let slot = self.tx_next;
        let mut header = [0u8; HEADER];
        header[0x00..0x04].copy_from_slice(&(TXC_START | if unicast { TXC_ACK } else { 0 }).to_le_bytes());
        header[0x04..0x06].copy_from_slice(&frame[0..2]);
        header[0x08..0x0A].copy_from_slice(&(TX_CORES | ofdm as u16).to_le_bytes()); // no per-frame power
        header[0x0A..0x0C].copy_from_slice(&control1.to_le_bytes()); header[0x0C..0x0E].copy_from_slice(&control1.to_le_bytes());
        header[0x12] = code; header[0x14] = ofdm as u8; header[0x15] = self.channel;
        header[0x26..0x2C].copy_from_slice(&frame[4..10]);
        header[0x36..0x3C].copy_from_slice(&p);
        header[0x3C..0x3E].copy_from_slice(&frame[2..4]);
        let cookie = 0x2000u16 | slot as u16;
        header[0x4C..0x4E].copy_from_slice(&cookie.to_le_bytes());
        header[0x70..0x76].copy_from_slice(&p);
        let at = self.tx_buffers + slot * TX_STRIDE;
        self.dma.bytes(at, HEADER).copy_from_slice(&header);
        self.dma.bytes(at + HEADER, frame.len()).copy_from_slice(frame);
        let d = self.tx_ring + 16 * slot;
        let address = self.dma.physical(at) as u32;
        self.dma.write32(d + 4, (HEADER + frame.len()) as u32);
        self.dma.write32(d + 8, address);
        self.dma.write32(d + 12, TRANSLATION);
        self.dma.write32(d, SOF | EOF | IOC | if slot == TX_SLOTS - 1 { EOT } else { 0 });
        self.tx_next = (slot + 1) % TX_SLOTS;
        fence(Ordering::SeqCst);
        self.c.w32(TX_INDEX, (self.tx_next * 16) as u32);
        let status = self.status(cookie);
        self.c.w32(IRQ_REASON, IRQ_TX_STATUS);
        let word = status.ok_or(Error::Failed)?;
        let (count, suppressed) = ((word >> 12 & 0xF) as u8, word >> 2 & 7);
        if count == 0 || suppressed != 0 { return Err(Error::Failed); }
        Ok(Sent { acked: word & 2 != 0, attempts: count })
    }

    // The final status word for `cookie`, others skipped, polling the receive ring while waiting.
    fn status(&mut self, cookie: u16) -> Option<u32> {
        let end = mind::time::uptime_ms() + STATUS_MS;
        loop {
            let first = self.c.r32(TX_STATUS0);
            if first != u32::MAX && first & 1 != 0 {
                let _ = self.c.r32(TX_STATUS1);
                if first & 0x40 == 0 && (first >> 16) as u16 == cookie { return Some(first & 0xFFFF); }
                continue;
            }
            if mind::time::uptime_ms() >= end { return None; }
            self.poll();
            mind::time::sleep(1);
        }
    }

    // Both engines stopped, the MAC suspended; the core is the caller's to reset.
    pub fn stop(self) {
        let c = self.c;
        c.suspend();
        for (control, low, high, status, name) in [(TX_CONTROL, TX_RING_LOW, TX_RING_HIGH, TX_STATUS, "TRANSMIT"), (RX_CONTROL, RX_RING_LOW, RX_RING_HIGH, RX_STATUS, "RECEIVE")] {
            c.w32(control, 0);
            let end = mind::time::uptime_ms() + 10;
            let mut state = c.r32(status) >> 28;
            while state != 0 && mind::time::uptime_ms() < end { mind::time::sleep(1); state = c.r32(status) >> 28; }
            c.w32(low, 0); c.w32(high, 0);
            mind::println!("[BCM] {} ENGINE {} (STATE {})", name, if state == 0 { "DISABLED" } else { "NOT DISABLED IN 10 MS" }, state);
        }
    }
}
