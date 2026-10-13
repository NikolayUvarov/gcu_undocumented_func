// Stage 3 (550-DRV-0024): with the microcode running, the MAC's initial values, the 2059 radio and the HT-PHY set up
// for 2.4 GHz at 20 MHz, a channel tuned, and the receiver asked for noise samples. Nothing is transmitted on purpose:
// the transmit power steps are left out. Register facts come from the stage-3 fact sheet (Linux's b43, described).
use crate::tables::*;
use core::cell::Cell;
use mind::dev::Mmio;

// 802.11 core registers (window 1).
const MAC_CONTROL: usize = 0x120; const MAC_COMMAND: usize = 0x124; const IRQ_REASON: usize = 0x128;
const TEMPLATE_CONTROL: usize = 0x130; const TEMPLATE_DATA: usize = 0x134; const HW_CAPABILITIES: usize = 0x15C;
const SHM_CONTROL: usize = 0x160; const SHM_DATA: usize = 0x164; const SHM_DATA_HIGH: usize = 0x166; const CLOCK: usize = 0x1E0;
const RADIO_CONTROL: usize = 0x3D8; const RADIO_DATA: usize = 0x3DA; const PHY_VERSION: usize = 0x3E0;
const PHY_CONTROL: usize = 0x3FC; const PHY_DATA: usize = 0x3FE; const ADDRESS_MATCH_CONTROL: usize = 0x420;
const ADDRESS_MATCH_DATA: usize = 0x422; const PSM_PHY_HEADER: usize = 0x492; const GPIO_MASK: usize = 0x49E;
// The agent's I/O control (window 2) and ChipCommon (BAR0+0x3000).
const IO_CONTROL: usize = 0x1408; const CC: usize = 0x3000;
const CC_GPIO_CONTROL: usize = 0x06C; const CC_PMU_CONTROL: usize = 0x600; const CC_CHIP_CONTROL_ADDRESS: usize = 0x650;
const CC_CHIP_CONTROL_DATA: usize = 0x654; const CC_PLL_CONTROL_ADDRESS: usize = 0x660; const CC_PLL_CONTROL_DATA: usize = 0x664;
// MAC control bits; interrupt reasons; the MAC command asking the microcode for a noise sample.
const MAC_ENABLED: u32 = 1; const MAC_INFRA: u32 = 1 << 17; const MAC_AWAKE: u32 = 1 << 26; const MAC_HW_POWER_SAVE: u32 = 1 << 25;
const MAC_DISCARD_PMQ: u32 = 1 << 30; const MAC_GPIO_SELECT: u32 = 0xC000;
const MAC_FILTERS: u32 = 1 << 18 | 1 << 22 | 1 << 21 | 1 << 23 | 1 << 24 | 1 << 20; // AP, keep control/PLCP/FCS, promiscuous
const IRQ_SUSPENDED: u32 = 1; const IRQ_NOISE: u32 = 1 << 18; const COMMAND_NOISE: u32 = 1 << 4;
// Shared memory routings.
const SHARED: u32 = 1; const SCRATCH: u32 = 2;
// Initial values files: an 8-byte header (type 'i', version 1, big-endian count), then entries.
const INITVALS: [&str; 2] = ["data/firmware/b43/ht0initvals29.fw", "data/firmware/b43/ht0bsinitvals29.fw"];
// PHY writes in a row before one is read back to flush them.
const WRITES_IN_ROW: u32 = 24;

// `gain`: the entry of the transmit gain table (0x1A from 0xC0) the fixed transmit gain is taken from; b43 uses 40.
pub struct Core<'a> { bar: &'a Mmio, phy_writes: Cell<u32>, radio_writes: Cell<u32>, pub(crate) gain: Cell<u16> }

impl<'a> Core<'a> {
    pub fn new(bar: &'a Mmio) -> Self { Self { bar, phy_writes: Cell::new(0), radio_writes: Cell::new(0), gain: Cell::new(40) } }
    pub(crate) fn r16(&self, r: usize) -> u16 { self.bar.read16(r) }
    pub(crate) fn w16(&self, r: usize, v: u16) { self.bar.write16(r, v) }
    pub(crate) fn r32(&self, r: usize) -> u32 { self.bar.read32(r) }
    pub(crate) fn w32(&self, r: usize, v: u32) { self.bar.write32(r, v) }
    pub(crate) fn set32(&self, r: usize, s: u32) { self.w32(r, self.r32(r) | s) }
    pub(crate) fn clear32(&self, r: usize, c: u32) { self.w32(r, self.r32(r) & !c) }
    fn cc(&self, r: usize) -> u32 { self.r32(CC + r) }
    fn cc_w(&self, r: usize, v: u32) { self.w32(CC + r, v) }
    // About `n` microseconds: reads of the agent take about one each.
    fn us(&self, n: usize) { for _ in 0..n { let _ = self.r32(0x1500); } }

    pub(crate) fn phy(&self, reg: u16) -> u16 { self.phy_writes.set(0); self.w16(PHY_CONTROL, reg); self.r16(PHY_DATA) }
    fn phy_w(&self, reg: u16, v: u16) {
        if self.phy_writes.get() >= WRITES_IN_ROW { let _ = self.r16(PHY_VERSION); self.phy_writes.set(0); }
        self.phy_writes.set(self.phy_writes.get() + 1);
        self.w16(PHY_CONTROL, reg); self.w16(PHY_DATA, v);
    }
    // Keep `keep`, OR `or`, with the address written once.
    fn phy_ms(&self, reg: u16, keep: u16, or: u16) { self.w16(PHY_CONTROL, reg); let v = self.r16(PHY_DATA); self.w16(PHY_DATA, (v & keep) | or); }
    fn phy_set(&self, reg: u16, s: u16) { self.phy_ms(reg, 0xFFFF, s) }
    pub(crate) fn phy_clear(&self, reg: u16, c: u16) { self.phy_ms(reg, !c, 0) }

    fn radio(&self, reg: u16) -> u16 { self.radio_writes.set(0); self.w16(RADIO_CONTROL, reg | 0x200); self.r16(RADIO_DATA) }
    fn radio_w(&self, reg: u16, v: u16) {
        if self.radio_writes.get() >= WRITES_IN_ROW { let _ = self.r32(MAC_CONTROL); self.radio_writes.set(0); }
        self.radio_writes.set(self.radio_writes.get() + 1);
        self.w16(RADIO_CONTROL, reg); self.w16(RADIO_DATA, v);
    }
    fn radio_ms(&self, reg: u16, keep: u16, or: u16) { let v = self.radio(reg); self.radio_w(reg, (v & keep) | or) }
    fn radio_set(&self, reg: u16, s: u16) { self.radio_ms(reg, 0xFFFF, s) }
    fn radio_clear(&self, reg: u16, c: u16) { self.radio_ms(reg, !c, 0) }
    // Polls a radio register until (value & mask) == want, every `step_us`, for up to `ms`.
    fn radio_wait(&self, reg: u16, mask: u16, want: u16, step_us: usize, ms: usize) -> Result<u16, u16> {
        let end = mind::time::uptime_ms() + ms;
        loop {
            let v = self.radio(reg);
            if v & mask == want { return Ok(v); }
            if mind::time::uptime_ms() >= end { return Err(v); }
            self.us(step_us);
        }
    }

    // PHY tables: the address word (table << 10 | offset) once, then entries; a 32-bit entry high half first.
    fn table16(&self, table: u16, offset: u16, values: &[u16]) { self.phy_w(0x072, table << 10 | offset); for &v in values { self.phy_w(0x073, v); } }
    fn table32(&self, table: u16, offset: u16, values: &[u32]) {
        self.phy_w(0x072, table << 10 | offset);
        for &v in values { self.phy_w(0x074, (v >> 16) as u16); self.phy_w(0x073, v as u16); }
    }
    fn table16_fill(&self, table: u16, offset: u16, n: usize) { self.phy_w(0x072, table << 10 | offset); for _ in 0..n { self.phy_w(0x073, 0); } }
    fn table32_fill(&self, table: u16, offset: u16, n: usize) { self.phy_w(0x072, table << 10 | offset); for _ in 0..n { self.phy_w(0x074, 0); self.phy_w(0x073, 0); } }
    fn table16_read(&self, table: u16, offset: u16) -> u16 { self.phy_w(0x072, table << 10 | offset); self.phy(0x073) }
    fn table32_read(&self, table: u16, offset: u16) -> u32 { self.phy_w(0x072, table << 10 | offset); let high = self.phy(0x074) as u32; high << 16 | self.phy(0x073) as u32 }

    // Shared memory takes byte offsets; scratch takes word indexes.
    pub(crate) fn shm16(&self, offset: u32) -> u16 { self.w32(SHM_CONTROL, SHARED << 16 | offset / 4); self.r16(if offset % 4 == 2 { SHM_DATA_HIGH } else { SHM_DATA }) }
    pub(crate) fn shm16_w(&self, offset: u32, v: u16) { self.w32(SHM_CONTROL, SHARED << 16 | offset / 4); self.w16(if offset % 4 == 2 { SHM_DATA_HIGH } else { SHM_DATA }, v) }
    fn shm32_w(&self, offset: u32, v: u32) { self.w32(SHM_CONTROL, SHARED << 16 | offset / 4); self.w32(SHM_DATA, v) }
    pub(crate) fn scratch_w(&self, index: u32, v: u16) { self.w32(SHM_CONTROL, SCRATCH << 16 | index); self.w16(SHM_DATA, v) }

    fn force_clock(&self, on: bool) { if on { self.set32(IO_CONTROL, 0x2) } else { self.clear32(IO_CONTROL, 0x2) } }
    // The RF sequencer driven through `bit`; true when its status bit cleared within 200 ms.
    fn rf_sequence(&self, bit: u16) -> bool {
        let saved = self.phy(0x800);
        self.phy_set(0x800, 0x0003); self.phy_set(0x803, bit);
        let end = mind::time::uptime_ms() + 200;
        let mut done = false;
        while mind::time::uptime_ms() < end { if self.phy(0x804) & bit == 0 { done = true; break; } self.us(50); }
        self.phy_w(0x800, saved);
        done
    }
    fn cca_reset(&self, full: bool) {
        self.force_clock(true);
        let v = self.phy(0x001);
        self.phy_w(0x001, v | 0x4000); if full { self.us(1); }
        self.phy_w(0x001, v & !0x4000);
        self.force_clock(false);
        if full { self.rf_sequence(0x0020); }
    }
    fn classifier(&self, mask: u16, value: u16) {
        let bits = (self.phy(0x0B0) & 0x7 & !mask) | (value & mask);
        let fresh = self.phy(0x0B0);
        self.phy_w(0x0B0, (fresh & 0xFFF8) | bits);
    }
    fn bphy_release(&self) {
        let header = self.r16(PSM_PHY_HEADER);
        self.w16(PSM_PHY_HEADER, header | 0x0004);
        self.phy_clear(0xC01, 0xC000);
        self.w16(PSM_PHY_HEADER, header);
    }
    fn analog_on(&self) { for (control, over) in [(0x911, 0x910), (0x915, 0x914), (0x919, 0x918)] { self.phy_w(control, 0x00CD); self.phy_w(over, 0x0000); } }

    // The fixed transmit gain the channel switch sets from entry `gain` of the gain table; the entry is returned.
    pub(crate) fn tx_power_fix(&self) -> u32 {
        let mut t = 0;
        for i in 0..3u16 {
            t = self.table32_read(0x1A, 0x0C0 + self.gain.get());
            self.phy_w(0x908, 0);
            self.table16(7, 0x110 + i, &[(t >> 16) as u16]);
            self.phy_w(0x072, 0x0D << 10 | (0x63 + 4 * i)); self.phy_w(0x073, (t & 0xFF) as u16);
            self.phy_w(0x072, 0x0D << 10 | (0x73 + 4 * i)); self.phy_w(0x073, (t & 0xFF) as u16);
        }
        t
    }

    // A 2.4 GHz channel at 20 MHz (1 to 13): the radio's synthesizer and per-core values, its calibration pulse, the
    // PHY's channel values, spur avoidance mode 0, the PHY PLL reset; the clock control before and after is returned.
    pub fn channel(&self, ch: usize) -> (u32, u32) {
        let i = ch - 1;
        self.shm16_w(0x00A0, ch as u16);
        for (reg, values) in SYNTH { self.radio_w(reg, values[i] as u16); }
        for core in [0x000, 0x400, 0x800] { for (reg, values) in PER_CORE { self.radio_w(reg | core, values[i] as u16); } }
        self.us(50);
        self.radio_clear(0x02B, 0x0001); self.radio_clear(0x02E, 0x0004); self.radio_set(0x02E, 0x0004); self.radio_set(0x02B, 0x0001);
        self.us(300);
        self.phy_clear(0x009, 0x0001);
        self.bphy_release();
        for (reg, values) in PHY_CHANNEL { self.phy_w(reg, values[i]); }
        self.classifier(0x0002, 0x0002);
        self.phy_clear(0xC0A, 0x0840);
        self.tx_power_fix();
        let before = self.r32(CLOCK);
        self.clear32(CLOCK, 0x200); let _ = self.r32(CLOCK);
        self.cc_w(CC_PLL_CONTROL_ADDRESS, 0); self.cc_w(CC_PLL_CONTROL_DATA, 0x1110_0014);
        self.cc_w(CC_PLL_CONTROL_ADDRESS, 2); self.cc_w(CC_PLL_CONTROL_DATA, 0x0300_0A08);
        self.cc_w(CC_PMU_CONTROL, self.cc(CC_PMU_CONTROL) | 0x400);
        self.clear32(CLOCK, 0x300); let after = self.r32(CLOCK);
        self.w16(0x62E, 0x6666); self.w16(0x630, 0x0006);
        self.cc_w(CC_CHIP_CONTROL_ADDRESS, 0);
        let c = self.cc(CC_CHIP_CONTROL_DATA);
        self.cc_w(CC_CHIP_CONTROL_DATA, c & !0x4); self.cc_w(CC_CHIP_CONTROL_DATA, c | 0x4); self.cc_w(CC_CHIP_CONTROL_DATA, c & !0x4);
        self.phy_clear(0x001, 0x8000);
        self.cca_reset(true);
        self.phy_w(0x17E, 0x3830);
        mind::time::sleep(8);
        (before, after)
    }

    // MAC suspended and enabled as the microcode expects; true when it answered.
    pub(crate) fn suspend(&self) -> bool {
        self.set32(MAC_CONTROL, MAC_AWAKE); self.clear32(MAC_CONTROL, MAC_ENABLED); let _ = self.r32(MAC_CONTROL);
        let end = mind::time::uptime_ms() + 50;
        while mind::time::uptime_ms() < end { if self.r32(IRQ_REASON) & IRQ_SUSPENDED != 0 { return true; } self.us(10); }
        false
    }
    pub(crate) fn enable(&self) -> u16 {
        self.set32(MAC_CONTROL, MAC_ENABLED); self.w32(IRQ_REASON, IRQ_SUSPENDED);
        let _ = (self.r32(MAC_CONTROL), self.r32(IRQ_REASON));
        self.w32(MAC_CONTROL, (self.r32(MAC_CONTROL) | MAC_AWAKE) & !MAC_HW_POWER_SAVE);
        let mut status = 0;
        for _ in 0..100 { status = self.shm16(0x0040); if status != 4 { break; } self.us(10); }
        status
    }
    // One noise sample from the microcode: four bytes, 0x7F where none came, or the interrupt reason when it never answered.
    fn noise(&self) -> Result<[u8; 4], u32> {
        self.shm32_w(0x0088, 0x7F7F_7F7F);
        self.set32(MAC_COMMAND, COMMAND_NOISE);
        let end = mind::time::uptime_ms() + 1000;
        loop {
            let reason = self.r32(IRQ_REASON);
            if reason & IRQ_NOISE != 0 { self.w32(IRQ_REASON, IRQ_NOISE); break; }
            if mind::time::uptime_ms() >= end { return Err(reason); }
            mind::time::sleep(1);
        }
        let (low, high) = (self.shm16(0x0088), self.shm16(0x008A));
        Ok([low as u8, (low >> 8) as u8, high as u8, (high >> 8) as u8])
    }
}

// The entries of an initial values file applied to the core; the count written, or why not.
fn initvals(core: &Core, data: &[u8]) -> Result<usize, &'static str> {
    if data.len() < 8 || data[0] != b'i' || data[1] != 1 { return Err("NOT AN INITIAL VALUES FILE OF VERSION 1"); }
    let count = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
    // Checked whole before anything is written.
    let mut at = 8;
    for _ in 0..count {
        let word = u16::from_be_bytes(*data.get(at..at + 2).and_then(|b| b.first_chunk()).ok_or("AN ENTRY RUNS PAST THE END")?);
        let size = if word & 0x8000 != 0 { 4 } else { 2 };
        if word & 0x7FFF >= 0x1000 { return Err("AN OFFSET PAST THE CORE'S REGISTERS"); }
        if at + 2 + size > data.len() { return Err("AN ENTRY RUNS PAST THE END"); }
        at += 2 + size;
    }
    if at != data.len() { return Err("BYTES LEFT AFTER THE ENTRIES"); }
    let mut at = 8;
    for _ in 0..count {
        let word = u16::from_be_bytes([data[at], data[at + 1]]);
        let offset = (word & 0x7FFF) as usize;
        if word & 0x8000 != 0 { core.w32(offset, u32::from_be_bytes([data[at + 2], data[at + 3], data[at + 4], data[at + 5]])); at += 6; }
        else { core.w16(offset, u16::from_be_bytes([data[at + 2], data[at + 3]])); at += 4; }
    }
    Ok(count)
}

fn read_file(path: &str, buffer: &mut [u8]) -> Option<usize> {
    let file = mind::fs::File::open(path).ok()?;
    let n = file.read_at(0, buffer).ok()?;
    (n == file.size() && n < buffer.len()).then_some(n)
}

fn step(n: u32, what: &str) { mind::println!("[BCM] STAGE 3, STEP {}: {}", n, what); mind::time::sleep(crate::STEP_MS); }

// The stage, after the microcode answered; `mac` from the SPROM. True when it ran through with the MAC suspended at
// the end; the core is left to the caller to reset.
pub fn run(core: &Core, mac: [u8; 6]) -> bool {
    let Some(mut pages) = mind::mem::Pages::new(16 * 1024) else { mind::println!("[BCM] NO MEMORY FOR THE INITIAL VALUES"); return false };
    let buffer = pages.as_mut_slice();

    step(1, "GPIO AND THE INITIAL VALUES");
    let _ = core.r32(IRQ_REASON);
    core.clear32(MAC_CONTROL, MAC_GPIO_SELECT);
    core.w16(GPIO_MASK, core.r16(GPIO_MASK) | 0x000F);
    core.cc_w(CC_GPIO_CONTROL, (core.cc(CC_GPIO_CONTROL) & !0x1F) | 0x0F);
    for path in INITVALS {
        let Some(n) = read_file(path, buffer) else { mind::println!("[BCM] NO INITIAL VALUES AT {}: STAGE 3 STOPPED", path); return false };
        match initvals(core, &buffer[..n]) {
            Ok(count) => mind::println!("[BCM] {}: {} BYTES, {} VALUES WRITTEN", path, n, count),
            Err(why) => { mind::println!("[BCM] {}: {}: STAGE 3 STOPPED", path, why); return false; }
        }
    }

    step(2, "ANALOG ON, THE RADIO RESET AND ITS INITIAL TABLE");
    core.analog_on();
    if core.r32(MAC_CONTROL) & MAC_ENABLED != 0 { mind::println!("[BCM] THE MAC IS ENABLED BEFORE THE RADIO IS SET UP"); }
    core.phy_clear(0x810, 0x0002); core.phy_set(0x810, 0x0001); core.phy_clear(0x810, 0x0001); core.phy_set(0x810, 0x0002);
    for (reg, v) in [(0xC51, 0x70), (0xC5A, 0x03), (0xC79, 0x01), (0xC82, 0x70), (0xC83, 0x00), (0xC84, 0x70), (0xC9A, 0x7F), (0xCB6, 0x10), (0xD88, 0x05)] { core.radio_w(reg, v); }
    for reg in [0x146, 0x546, 0x946] { core.radio_set(reg, 0x0003); }
    core.radio_set(0x02E, 0x0078); core.radio_set(0x0C0, 0x0080); mind::time::sleep(2); core.radio_clear(0x02E, 0x0078); core.radio_clear(0x0C0, 0x0080);

    step(3, "THE RADIO'S RESISTOR CALIBRATION");
    core.radio_set(0x804, 0x0001); core.us(20);
    core.radio_set(0x8BF, 0x0001);
    core.radio_ms(0x99B, 0x0003, 0x0002);
    core.radio_set(0x804, 0x0002); core.us(200);
    core.radio_clear(0x804, 0x0002);
    let rcal = core.radio_wait(0x945, 0x0001, 0x0001, 100, 1000);
    core.radio_clear(0x804, 0x0001);
    core.radio_set(0x00A, 0x0060);
    match rcal { Ok(v) => mind::println!("[BCM] RESISTOR CALIBRATION DONE (RADIO 945 = {:04X})", v), Err(v) => mind::println!("[BCM] RESISTOR CALIBRATION NOT DONE IN 1 S (RADIO 945 = {:04X})", v) }

    step(4, "THE RADIO'S RC CALIBRATION, THREE PASSES");
    for (pass, (master, trc)) in [(0x61, 0xE9), (0x69, 0xD5), (0x73, 0x99)].into_iter().enumerate() {
        core.radio_w(0x17F, master); core.radio_w(0x13D, 0x6E); core.radio_w(0x13E, trc); core.radio_w(0x13C, 0x55);
        let done = core.radio_wait(0x140, 0x0002, 0x0002, 500, 5000);
        core.radio_w(0x13C, 0x15);
        match done { Ok(v) => mind::println!("[BCM] RC CALIBRATION PASS {} DONE (RADIO 140 = {:04X})", pass + 1, v), Err(v) => mind::println!("[BCM] RC CALIBRATION PASS {} NOT DONE IN 5 S (RADIO 140 = {:04X})", pass + 1, v) }
    }
    core.radio_clear(0x17F, 0x0001);
    core.radio_clear(0x011, 0x0008);

    step(5, "THE FIRST CHANNEL SWITCH, TO CHANNEL 6");
    let (before, after) = core.channel(6);
    mind::println!("[BCM] CHANNEL 6: CLOCK CONTROL {:08X} BEFORE THE PLL UPDATE, {:08X} AFTER", before, after);

    step(6, "THE PHY TABLES, 24 UPLOADS");
    core.table16(0x12, 0x000, &SET_A); core.table16(0x27, 0x000, &SET_B);
    core.table16_fill(0x26, 0x000, 128); core.table32_fill(0x25, 0x000, 128);
    core.table32(0x2F, 0x000, &SET_C);
    for t in [0x1A, 0x1B, 0x1C] { core.table16(t, 0x000, &SET_D); }
    core.table32(0x1A, 0x0C0, &SET_E);
    for t in [0x1A, 0x1B, 0x1C] { core.table32_fill(t, 0x140, 128); }
    for t in [0x1A, 0x1B, 0x1C] { core.table16_fill(t, 0x1C0, 128); }
    for t in [0x1A, 0x1B, 0x1C] { core.table16(t, 0x240, &SET_F); }
    for t in [0x1F, 0x21, 0x23] { core.table32(t, 0x000, &SET_G); }
    for t in [0x20, 0x22, 0x24] { core.table32(t, 0x000, &SET_H); }

    step(7, "THE HT-PHY'S SET-UP SEQUENCE");
    phy_init(core);

    step(8, "THE SECOND CHANNEL SWITCH, TO CHANNEL 6");
    let (before, after) = core.channel(6);
    mind::println!("[BCM] CHANNEL 6 AGAIN: CLOCK CONTROL {:08X} BEFORE THE PLL UPDATE, {:08X} AFTER; RADIO 945 {:04X}, 140 {:04X}", before, after, core.radio(0x945), core.radio(0x140));

    step(9, "THE REST OF THE CHIP'S INIT AND THE MAC ADDRESS");
    chip_init(core, mac);
    mind::println!("[BCM] MAC CONTROL {:08X}, IRQ REASON {:08X}, CLOCK CONTROL {:08X}, MICROCODE STATUS {}", core.r32(MAC_CONTROL), core.r32(IRQ_REASON), core.r32(CLOCK), core.shm16(0x0040));

    step(10, "THE RECEIVER ON: NOISE SAMPLES ON CHANNELS 1, 6 AND 11");
    let status = core.enable();
    mind::println!("[BCM] MAC ENABLED: MICROCODE STATUS {} (2 ACTIVE), MAC CONTROL {:08X}", status, core.r32(MAC_CONTROL));
    for ch in [1, 6, 11] {
        if !core.suspend() { mind::println!("[BCM] THE MAC DOES NOT SUSPEND FOR CHANNEL {}", ch); break; }
        core.channel(ch);
        core.enable();
        mind::time::sleep(100);
        for _ in 0..3 {
            match core.noise() {
                Ok(s) => mind::println!("[BCM] CHANNEL {}: NOISE SAMPLE {:02X} {:02X} {:02X} {:02X} (SIGNED: {} {} {} {}; 7F: NONE)", ch, s[0], s[1], s[2], s[3], s[0] as i8, s[1] as i8, s[2] as i8, s[3] as i8),
                Err(reason) => { mind::println!("[BCM] CHANNEL {}: NO NOISE SAMPLE IN 1 S (IRQ REASON {:08X})", ch, reason); break; }
            }
        }
    }
    let suspended = core.suspend();
    mind::println!("[BCM] MAC SUSPENDED: {}; IRQ REASON {:08X}", if suspended { "YES" } else { "NO" }, core.r32(IRQ_REASON));
    suspended
}

// The HT-PHY's set-up after its tables (fact sheet 3.3, steps 1 to 34); the transmit power steps are left out.
fn phy_init(c: &Core) {
    c.phy_clear(0x0BE, 0x0002);
    for r in [0x23F, 0x240, 0x241] { c.phy_set(r, 0x07FF); }
    for r in [0x840, 0x841, 0x842, 0x843, 0x860, 0x861, 0x862, 0x863, 0x880, 0x881, 0x882, 0x883] { c.phy_w(r, 0); }
    for r in [0x84C, 0x86C, 0x88C] { c.phy_w(r, 0); }
    c.phy_clear(0x800, 0x0003);
    for r in [0x910, 0x914, 0x918] { c.phy_w(r, 0); }
    for (r, v) in [(0x903, 0x0020), (0x901, 0x0020), (0x20D, 0x00B8), (0x94F, 0x00C8), (0x070, 0x0050), (0x1FF, 0x0030)] { c.phy_w(r, v); }
    c.classifier(0x0001, 0x0001);
    c.phy_set(0x0B1, 0x0091);
    for (r, v) in [(0x32F, 0x0003), (0x077, 0x0010), (0x0B4, 0x0258)] { c.phy_w(r, v); }
    c.phy_clear(0x17E, 0x4000);
    c.phy_w(0x0B9, 0x0072);
    for o in [0x14E, 0x15E, 0x16E] { c.table16(7, o, &[0x010F, 0x010F]); }
    for (i, (over, control)) in [(0x910, 0x911), (0x914, 0x915), (0x918, 0x919)].into_iter().enumerate() {
        c.phy_set(control, 0x0004); c.phy_set(over, 0x0004); c.phy_clear(control, 0x0001); c.phy_set(over, 0x0001);
        c.table16(8, 0x05 + 0x10 * i as u16, &[0]);
        c.phy_clear(over, 0x0004);
    }
    c.table16(7, 0x130, &[0x0777, 0x0111, 0x0111, 0x0777, 0x0111, 0x0111, 0x0777, 0x0111, 0x0111]);
    c.table16(7, 0x120, &[0x0777]); c.table16(7, 0x124, &[0x0777]);
    for o in [0x00, 0x10, 0x20] { c.table16(8, o, &[0x0002]); }
    c.table16(8, 0x08, &[0x008E, 0x0096, 0x0096, 0x0096]);
    c.table16(8, 0x18, &[0x008F, 0x009F, 0x009F, 0x009F]);
    c.table16(8, 0x28, &[0x008F, 0x009F, 0x009F, 0x009F]);
    for o in [0x0C, 0x1C, 0x2C] { c.table16(8, o, &[0x0002; 4]); }
    c.phy_ms(0x280, 0xFF00, 0x003E); c.phy_ms(0x283, 0xFF00, 0x003E); c.phy_ms(0x541, 0xFF00, 0x0046); c.phy_ms(0x283, 0xFF00, 0x0040);
    for t in [0x00, 0x01, 0x28] { c.table16(t, 0x08, &[0x0009, 0x000E, 0x0013, 0x0018]); }
    for r in [0x424, 0x464, 0x4A4] { c.phy_ms(r, 0x003F, 0x000D); }
    for r in [0x860, 0x864, 0x880, 0x884] { c.phy_set(r, 0x0001); }
    for (from, to) in [(0x144, 0x14A), (0x154, 0x15A), (0x164, 0x16A)] { let v = c.table16_read(7, from); c.table16(7, to, &[v]); }
    c.cca_reset(false);
    c.set32(IO_CONTROL, 0x0010);
    let saved = [c.phy(0x84C), c.phy(0x86C), c.phy(0x88C)];
    for r in [0x84C, 0x86C, 0x88C] { c.phy_w(r, 0x0400); }
    let (to_tx, to_rx) = (c.rf_sequence(0x0001), c.rf_sequence(0x0020));
    for (r, v) in [0x84C, 0x86C, 0x88C].into_iter().zip(saved) { c.phy_w(r, v); }
    c.classifier(0, 0);
    let _ = (c.phy(0x40E), c.phy(0x44E), c.phy(0x48E));
    // The B-PHY: 0xC88..0xC97 get 0x1E1F down to 0x0001, 0xC98..0xCA7 get 0x3E3F down to 0x2021.
    for (i, r) in (0xC88..=0xC97).enumerate() { let k = 2 * (15 - i as u16); c.phy_w(r, k << 8 | (k + 1)); }
    for (i, r) in (0xC98..=0xCA7).enumerate() { let k = 0x20 + 2 * (15 - i as u16); c.phy_w(r, k << 8 | (k + 1)); }
    c.phy_w(0xC38, 0x0668);
    c.table32(0x1A, 0x0C0, &SET_L);
    mind::println!("[BCM] HT-PHY SET UP: RF SEQUENCE TO TX {}, BACK TO RX {}; TRANSMIT POWER STEPS LEFT OUT",
                   if to_tx { "DONE" } else { "TIMED OUT" }, if to_rx { "DONE" } else { "TIMED OUT" });
}

// The rest of the chip's init and the station's address (fact sheet 1.6 steps 4 to 13, 1.7, 7.1 to 7.3).
fn chip_init(c: &Core, mac: [u8; 6]) {
    for offset in [0x0022, 0x0188] { let v = c.shm16(offset); c.shm16_w(offset, (v & !0x03C0) | 0x00C0); }
    c.w32(0x100, 0x0100_0000);
    c.clear32(MAC_CONTROL, MAC_INFRA); c.set32(MAC_CONTROL, MAC_INFRA);
    c.shm16_w(0x0074, 0);
    c.w32(MAC_CONTROL, (c.r32(MAC_CONTROL) & !MAC_FILTERS) | MAC_INFRA);
    c.w16(0x612, 0x0032);
    c.set32(MAC_CONTROL, MAC_DISCARD_PMQ);
    c.w32(0x188, 0x8000_0000); c.w32(0x18C, 0x0200_0000);
    c.w32(IRQ_REASON, 0x0000_4000);
    for (r, v) in [(0x024, 0x0001_FC00), (0x02C, 0x0000_DC00), (0x034, 0x0000_DC00), (0x03C, 0x0001_DC00), (0x044, 0x0000_DC00), (0x04C, 0x0000_DC00)] { c.w32(r, v); }
    c.set32(IO_CONTROL, 0x0010);
    c.w16(0x6A8, 0x0E74);
    // Core init: the core revision, the hardware's capabilities, retry limits, probe responses off, the PHY's type.
    c.shm16_w(0x0016, 29);
    let caps = c.r32(HW_CAPABILITIES);
    c.shm16_w(0x00C0, caps as u16); c.shm16_w(0x00C2, (caps >> 16) as u16);
    c.scratch_w(6, 7); c.scratch_w(7, 4);
    c.shm16_w(0x0044, 3); c.shm16_w(0x0046, 2); c.shm16_w(0x0074, 1);
    c.scratch_w(3, 0x000F); c.scratch_w(4, 0x03FF);
    c.shm16_w(0x0052, 7); c.shm16_w(0x0050, 1);
    c.shm16_w(0x0094, 500);
    // The address: no BSSID yet (slot 3 zero), the template copy, the station's own in slot 0.
    let slot = |n: u16, a: [u8; 6]| { c.w16(ADDRESS_MATCH_CONTROL, n | 0x0020); for k in 0..3 { c.w16(ADDRESS_MATCH_DATA, (a[2 * k + 1] as u16) << 8 | a[2 * k] as u16); } };
    slot(3, [0; 6]);
    let mut bytes = [0u8; 12]; bytes[..6].copy_from_slice(&mac);
    let swap = c.r32(MAC_CONTROL) & (1 << 16) != 0;
    for k in 0..3 {
        let word = u32::from_le_bytes([bytes[4 * k], bytes[4 * k + 1], bytes[4 * k + 2], bytes[4 * k + 3]]);
        c.w32(TEMPLATE_CONTROL, 0x20 + 4 * k as u32); c.w32(TEMPLATE_DATA, if swap { word.swap_bytes() } else { word });
    }
    // While only receiving, the address match holds no address of ours, so no frame is acknowledged.
    slot(0, [0; 6]);
    mind::println!("[BCM] HARDWARE CAPABILITIES {:08X}; THE STATION ADDRESS IN THE TEMPLATE, NOT IN THE ADDRESS MATCH", caps);
}
