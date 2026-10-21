#![no_std]
#![no_main]
// Ring 3 driver for the MacBook Pro's Broadcom BCM4331 Wi-Fi (550-DRV-0006). Stage 1 (550-DRV-0020) reads the chip
// behind its 16 KiB BAR0, the 802.11 core's state from its wrapper, and the SPROM. Stage 1b (550-DRV-0022) stops the
// 802.11 core the firmware left running with a reset pulse, and frees the SPROM's pins while the SPROM is read. Stage 2
// (550-DRV-0023), on request, loads the microcode from the disk, starts it, reads its revision and resets the core.
// Stage 3 sets up the radio; the radio module keeps it running for the station (550-DRV-0027), and stage 4 tests it.
// Register facts come from Broadcom's published headers and the b43 specifications; no driver code is taken.
use core::sync::atomic::{AtomicUsize, Ordering};
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, CAP_KIND_IRQ, SLOT_DEV0, SLOT_IRQ, SLOT_MEM, SLOT_SERVICE};
use mind::dev::{cap_info, device_config, Dma, Mmio};

mod radio;
mod serve;
mod stage3;
mod stage4;
mod tables;
mod txwait;

// PCI configuration: the backplane addresses BAR0's first two 4 KiB windows show.
const CFG_WINDOW: usize = 0x80; const CFG_WINDOW2: usize = 0xAC;
// BAR0 (16 KiB): the core in window 1, that core's wrapper in window 2, the PCIe core, ChipCommon.
const WINDOW: usize = 0x0000; const WRAPPER: usize = 0x1000; const PCIE: usize = 0x2000; const CC: usize = 0x3000;
// The backplane: ChipCommon at its base; core n at BASE + n * 4 KiB, its wrapper at WRAPPERS + n * 4 KiB.
const BASE: u32 = 0x1800_0000; const WRAPPERS: u32 = 0x1810_0000;

// ChipCommon registers.
const CC_CHIP_ID: usize = 0x000; const CC_CAPABILITIES: usize = 0x004; const CC_CHIP_CONTROL: usize = 0x028;
const CC_CHIP_STATUS: usize = 0x02C; const CC_SROM_CONTROL: usize = 0x190; const CC_EROM: usize = 0x0FC;
const CAP_SPROM: u32 = 1 << 30; // a serial SPROM is fitted
const CAP_PMU: u32 = 1 << 28; // a power management unit
// ChipCommon's clock control and the PMU: control, capabilities, status, resources up, pending, and the min and max masks.
const CC_CLOCK: usize = 0x1E0;
const CC_PMU: [usize; 7] = [0x600, 0x604, 0x608, 0x60C, 0x610, 0x618, 0x61C];
// Chip control bits that give the SPROM's pins to the external amplifier lines (BCM4331): cleared while it is read.
const CHIPCTL_EXTPA: u32 = 1 << 4 | 1 << 7 | 1 << 12;
// Where the SPROM's contents appear in ChipCommon; which one depends on revisions, so both are tried.
const SPROM_AT: [usize; 2] = [0x800, 0x830];
const SPROM_WORDS: usize = 220; // revision 8 and later: 440 bytes

// Wrapper (agent) registers of an AI backplane core.
const AI_IOCONTROL: usize = 0x408; const AI_IOSTATUS: usize = 0x500; const AI_RESET_CONTROL: usize = 0x800; const AI_RESET_STATUS: usize = 0x804;
const RESET: u32 = 1;
// The agent's error log: control, done, status, address low and high, id, user, flags.
const AI_ERROR_LOG: [usize; 8] = [0x900, 0x904, 0x908, 0x90C, 0x910, 0x914, 0x918, 0x91C];

// Wrapper I/O control bits of the 802.11 core.
const IO_CLOCK: u32 = 1 << 0; const IO_FORCE_GATED: u32 = 1 << 1; const IO_PHY_CLOCK: u32 = 1 << 2; const IO_PHY_RESET: u32 = 1 << 3;
const IO_MAC_PHY_CLOCK: u32 = 1 << 4; const IO_PHY_20MHZ: u32 = 1 << 6; const IO_GMODE: u32 = 1 << 13;
// 802.11 core registers (window 1).
const D11_MAC_CONTROL: usize = 0x120; const D11_IRQ_REASON: usize = 0x128; const D11_SHM_CONTROL: usize = 0x160; const D11_SHM_DATA: usize = 0x164;
const D11_CLOCK: usize = 0x1E0; const D11_RADIO_CONTROL: usize = 0x3D8; const D11_RADIO_DATA: usize = 0x3DA; const D11_PHY_VERSION: usize = 0x3E0;
// Clock control: high throughput forced and present; the 802.11 and PHY PLLs requested and running.
const CLOCK_FORCE_HT: u32 = 1 << 1; const CLOCK_HAVE_HT: u32 = 1 << 17; const CLOCK_PLL_REQUEST: u32 = 3 << 8; const CLOCK_PLL_RUNNING: u32 = 3 << 24;
// MAC control: the microcode processor running or held at 0, shared memory and internal registers on, G-mode, a station.
const MAC_PSM_RUN: u32 = 1 << 1; const MAC_PSM_JUMP0: u32 = 1 << 2; const MAC_SHM: u32 = 1 << 8; const MAC_IHR: u32 = 1 << 10;
const MAC_INFRA: u32 = 1 << 17; const MAC_GMODE: u32 = 1 << 31;
const IRQ_MAC_SUSPENDED: u32 = 1;
// Shared memory routing (high half of the control word): microcode memory, shared words, scratch registers; auto-increment.
const SHM_UCODE: u32 = 0; const SHM_SHARED: u32 = 1; const SHM_SCRATCH: u32 = 2; const SHM_AUTOINC_WRITE: u32 = 0x100;
// The microcode for core revision 29 that scripts/proprietary.sh puts on the disk; a b43 firmware file of type 'u'.
const MICROCODE: &str = "data/firmware/b43/ucode29_mimo.fw";
const MICROCODE_MAX: usize = 64 * 1024;
// The stages run as a test when this file exists (the maintainer's request), and the radio comes up for the station when
// init gives bcm_wifi its service endpoint (550-DRV-0027); without either the core stays in reset.
const STAGE2_FLAG: &str = "data/wifitest.txt";
/// With this file, 802.11g is offered to the station (550-DRV-0037).
const OFDM_FLAG: &str = "data/wifi-ofdm.txt";
// Each step is logged, then waited for longer than the journal's save period, so a hang leaves its step as the last line.
const STEP_MS: usize = 3000;
// The wait after each step: STEP_MS in a test, none when the radio only comes up for the station.
static STEP_WAIT: AtomicUsize = AtomicUsize::new(0);
pub fn step_wait() -> usize { STEP_WAIT.load(Ordering::Relaxed) }
// The BCM4331's 802.11 core is core 1 (ChipCommon 0, 802.11 1, PCIe 2).
const CORE_80211: u32 = 1;

// The CRC-8 a Broadcom SPROM ends with (polynomial x^8 + x^7 + x^6 + x^4 + x^2 + 1, reflected 0xAB, initial 0xFF).
fn crc8(bytes: &[u8]) -> u8 {
    let mut crc = 0xFFu8;
    for &byte in bytes {
        crc ^= byte;
        for _ in 0..8 { crc = if crc & 1 != 0 { (crc >> 1) ^ 0xAB } else { crc >> 1 }; }
    }
    crc
}

// The complemented CRC of every byte but the last word's high byte, which should equal it.
fn sprom_crc(words: &[u16]) -> u8 {
    let mut bytes = [0u8; SPROM_WORDS * 2];
    for (i, w) in words.iter().enumerate() { bytes[i * 2..i * 2 + 2].copy_from_slice(&w.to_le_bytes()); }
    crc8(&bytes[..bytes.len() - 1]) ^ 0xFF
}

// A SPROM image is valid when its last word holds its revision (low byte) and the CRC of all bytes before it matches
// the complement in the high byte.
fn sprom_valid(words: &[u16]) -> Option<u8> {
    let last = words[words.len() - 1];
    let revision = (last & 0xFF) as u8;
    (revision != 0 && revision != 0xFF && sprom_crc(words) == (last >> 8) as u8).then_some(revision)
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let Ok(bar) = Mmio::map(SLOT_DEV0) else { mind::println!("[BCM] NO BAR0 GRANTED"); return idle() };
    let id = device_config(SLOT_DEV0, 0x00).unwrap_or(0);
    let (window, window2) = (device_config(SLOT_DEV0, CFG_WINDOW).unwrap_or(0), device_config(SLOT_DEV0, CFG_WINDOW2).unwrap_or(0));
    mind::println!("[BCM] PCI {:04X}:{:04X}, BAR0 WINDOWS {:08X} AND {:08X}", id & 0xFFFF, id >> 16, window, window2);
    // What stage 4 receives with (550-KRN-0091): the DMA region and the interrupt, only noted; neither starts the core.
    match Dma::map(SLOT_MEM) { Ok(dma) => mind::println!("[BCM] DMA REGION AT {:#X}, {} KiB", dma.physical(0), dma.len() / 1024), Err(_) => mind::println!("[BCM] NO DMA REGION GRANTED") }
    let (kind, line, _) = cap_info(SLOT_IRQ);
    if kind == CAP_KIND_IRQ { mind::println!("[BCM] INTERRUPT GRANTED: LINE {}", line); } else { mind::println!("[BCM] NO INTERRUPT GRANTED (KIND {})", kind); }

    let chip = bar.read32(CC + CC_CHIP_ID);
    let (number, revision, package, cores, bus) = (chip & 0xFFFF, chip >> 16 & 0xF, chip >> 20 & 0xF, chip >> 24 & 0xF, chip >> 28);
    mind::println!("[BCM] CHIP {:04X} REVISION {} PACKAGE {} CORES {} BUS {}", number, revision, package, cores, if bus == 1 { "AI" } else { "OTHER" });
    let capabilities = bar.read32(CC + CC_CAPABILITIES);
    mind::println!("[BCM] CHIPCOMMON CAPABILITIES {:08X}, CHIP CONTROL {:08X}, STATUS {:08X}, SROM CONTROL {:08X}, EROM AT {:08X}",
                   capabilities, bar.read32(CC + CC_CHIP_CONTROL), bar.read32(CC + CC_CHIP_STATUS), bar.read32(CC + CC_SROM_CONTROL), bar.read32(CC + CC_EROM));
    if number != 0x4331 || bus != 1 { mind::println!("[BCM] NOT A BCM4331 ON AN AI BACKPLANE: STOPPED"); return idle(); }
    if capabilities & CAP_PMU != 0 {
        let pmu = CC_PMU.map(|r| bar.read32(CC + r));
        mind::println!("[BCM] CLOCK CONTROL {:08X}; PMU CONTROL {:08X}, CAPABILITIES {:08X}, STATUS {:08X}, RESOURCES UP {:08X}, PENDING {:08X}, MIN {:08X}, MAX {:08X}",
                       bar.read32(CC + CC_CLOCK), pmu[0], pmu[1], pmu[2], pmu[3], pmu[4], pmu[5], pmu[6]);
    }
    link_power();

    // The windows are used as the firmware left them; a core index and its wrapper must match.
    let index = window.wrapping_sub(BASE) / 0x1000;
    if window & 0xFFF != 0 || index > 15 || window2 != WRAPPERS + index * 0x1000 {
        mind::println!("[BCM] WINDOW 1 IS NOT A CORE WITH ITS WRAPPER IN WINDOW 2: STOPPED");
        return idle();
    }
    // The wrapper is readable whatever the core's state; the core itself is not touched while it may be in reset.
    let (control, status, reset, reset_status) = (bar.read32(WRAPPER + AI_IOCONTROL), bar.read32(WRAPPER + AI_IOSTATUS),
                                                 bar.read32(WRAPPER + AI_RESET_CONTROL), bar.read32(WRAPPER + AI_RESET_STATUS));
    mind::println!("[BCM] CORE {} (WINDOW 1): IO CONTROL {:08X} (CLOCK {}), IO STATUS {:08X}, RESET {:08X} ({}), RESET STATUS {:08X}",
                   index, control, if control & 1 != 0 { "ON" } else { "OFF" }, status, reset, if reset & 1 != 0 { "IN RESET" } else { "RUNNING" }, reset_status);
    // The firmware may leave the 802.11 core running, receiving into memory it no longer owns: a reset pulse leaves it idle.
    if index == CORE_80211 && reset & RESET == 0 { pulse(&bar, "THE 802.11 CORE WAS LEFT RUNNING BY THE FIRMWARE"); }
    mind::println!("[BCM] PCIE CORE: {:08X} {:08X} {:08X} {:08X}", bar.read32(PCIE), bar.read32(PCIE + 4), bar.read32(PCIE + 8), bar.read32(PCIE + 12));
    let _ = WINDOW;

    let mac = sprom(&bar, capabilities);
    if index == CORE_80211 {
        let test = mind::fs::File::open(STAGE2_FLAG).is_ok();
        let ofdm = mind::fs::File::open(OFDM_FLAG).is_ok();
        serve::offer_ofdm(ofdm);
        mind::println!("[BCM] 2.4 GHZ OFFERED TO THE STATION: {}", if ofdm { "802.11G (data/wifi-ofdm.txt)" } else { "802.11B ONLY, CCK (data/wifi-ofdm.txt OFFERS 802.11G)" });
        let serving = cap_info(SLOT_SERVICE).0 == CAP_KIND_ENDPOINT;
        if test { STEP_WAIT.store(STEP_MS, Ordering::Relaxed); }
        mind::println!("[BCM] TEST {} ({}), A STATION TO SERVE: {}", if test { "ASKED FOR" } else { "NOT ASKED FOR" }, STAGE2_FLAG, if serving { "YES" } else { "NO" });
        // The station is answered during the test, so that it and the stack do not wait for the test's end.
        if let (true, true, Some(own)) = (test, serving, mac) { serve::hold(own); mind::println!("[BCM] THE STATION IS ANSWERED BUSY UNTIL THE TEST ENDS"); }
        microcode(&bar, mac, test, serving);
        if test { test_done(); }
        if serving { serve::absent(); }
    }
    idle()
}

// The SPROM, read where it may appear, its revision, CRC and MAC address logged.
fn sprom(bar: &Mmio, capabilities: u32) -> Option<[u8; 6]> {
    if capabilities & CAP_SPROM == 0 { mind::println!("[BCM] NO SPROM FITTED (OTP SIZE CODE {})", capabilities >> 19 & 7); return None; }
    // The SPROM's pins are shared with the external amplifier lines; they are given back as they were after the read.
    let chip_control = bar.read32(CC + CC_CHIP_CONTROL);
    if chip_control & CHIPCTL_EXTPA != 0 {
        bar.write32(CC + CC_CHIP_CONTROL, chip_control & !CHIPCTL_EXTPA);
        mind::time::sleep(1);
        mind::println!("[BCM] SPROM PINS TAKEN FROM THE AMPLIFIER LINES: CHIP CONTROL {:08X} -> {:08X}", chip_control, bar.read32(CC + CC_CHIP_CONTROL));
    }
    let mut found = None;
    for at in SPROM_AT {
        let mut words = [0u16; SPROM_WORDS];
        for (i, w) in words.iter_mut().enumerate() { *w = bar.read16(CC + at + i * 2); }
        match sprom_valid(&words) {
            Some(revision) => {
                // Revision 8 and later: the MAC address in words 0x46..0x48, big-endian per word.
                let mac = [words[0x46] >> 8, words[0x46] & 0xFF, words[0x47] >> 8, words[0x47] & 0xFF, words[0x48] >> 8, words[0x48] & 0xFF];
                mind::println!("[BCM] SPROM AT CHIPCOMMON+{:03X}: REVISION {}, CRC GOOD, MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}, WORDS 0-3 {:04X} {:04X} {:04X} {:04X}",
                               at, revision, mac[0], mac[1], mac[2], mac[3], mac[4], mac[5], words[0], words[1], words[2], words[3]);
                // Revision 9's board flags (bytes 0x84, 0x86), chains (0xA2) and front end (0xAE), as read; b43's HT path uses none.
                mind::println!("[BCM] SPROM BYTES 84 {:04X}, 86 {:04X} (BOARD FLAGS), A2 {:04X} (CHAINS), AE {:04X} (FRONT END)", words[0x42], words[0x43], words[0x51], words[0x57]);
                found = Some(mac.map(|b| b as u8));
            }
            None => mind::println!("[BCM] SPROM AT CHIPCOMMON+{:03X}: NOT VALID (FIRST WORDS {:04X} {:04X} {:04X} {:04X}, LAST {:04X}, CRC OF THE REST {:02X})",
                                   at, words[0], words[1], words[2], words[3], words[SPROM_WORDS - 1], sprom_crc(&words)),
        }
    }
    if chip_control & CHIPCTL_EXTPA != 0 {
        bar.write32(CC + CC_CHIP_CONTROL, chip_control);
        mind::println!("[BCM] CHIP CONTROL GIVEN BACK: {:08X}", bar.read32(CC + CC_CHIP_CONTROL));
    }
    if found.is_none() { mind::println!("[BCM] NO VALID SPROM"); }
    found
}

// About `n` microseconds: one read of a wrapper register over PCIe takes about one.
fn pause(bar: &Mmio, n: usize) { for _ in 0..n { let _ = bar.read32(WRAPPER + AI_IOSTATUS); } }

// Polls `register` until `done` holds, for up to `ms` milliseconds; the last value read.
fn wait(bar: &Mmio, register: usize, ms: usize, done: impl Fn(u32) -> bool) -> Result<u32, u32> {
    let end = mind::time::uptime_ms() + ms;
    loop {
        let v = bar.read32(register);
        if done(v) { return Ok(v); }
        if mind::time::uptime_ms() >= end { return Err(v); }
        pause(bar, 10);
    }
}

// The 802.11 core's agent: readable whatever the core's state.
fn wrapper(bar: &Mmio, when: &str) {
    mind::println!("[BCM] AGENT {}: IO CONTROL {:08X}, IO STATUS {:08X}, RESET {:08X}, RESET STATUS {:08X}", when,
                   bar.read32(WRAPPER + AI_IOCONTROL), bar.read32(WRAPPER + AI_IOSTATUS), bar.read32(WRAPPER + AI_RESET_CONTROL), bar.read32(WRAPPER + AI_RESET_STATUS));
}

// The PCI Express link's control and status from configuration space: ASPM in control bits 0-1.
fn link_power() {
    let status = device_config(SLOT_DEV0, 0x04).unwrap_or(0) >> 16;
    let mut at = if status & 0x10 != 0 { device_config(SLOT_DEV0, 0x34).unwrap_or(0) as usize & 0xFC } else { 0 };
    for _ in 0..48 {
        if at < 0x40 { break; }
        let header = device_config(SLOT_DEV0, at).unwrap_or(0);
        if header & 0xFF == 0x10 {
            let link = device_config(SLOT_DEV0, at + 0x10).unwrap_or(0);
            let aspm = ["OFF", "L0S", "L1", "L0S AND L1"][(link & 3) as usize];
            mind::println!("[BCM] PCIE LINK CONTROL {:04X} (ASPM {}), LINK STATUS {:04X}", link & 0xFFFF, aspm, link >> 16);
            return;
        }
        at = (header >> 8) as usize & 0xFC;
    }
    mind::println!("[BCM] NO PCI EXPRESS CAPABILITY FOUND");
}

fn shm_select(bar: &Mmio, routing: u32, offset: u32) { bar.write32(D11_SHM_CONTROL, routing << 16 | offset); }

// Stage 2 (550-DRV-0023): the microcode read from the disk, the core enabled, the microcode loaded and started; its
// revision read back; then the processor stopped and the core put back into reset. No DMA is set up.
fn step(n: u32, what: &str) { mind::println!("[BCM] STAGE 2, STEP {}: {}", n, what); serve::pause(step_wait()); }

// The maintainer's sign that the disk may go back: logged after a wait longer than the journal's save period.
fn test_done() { serve::pause(STEP_MS); mind::println!("*** WIFI TEST Complete. Data collected ***"); }

fn microcode(bar: &Mmio, mac: Option<[u8; 6]>, test: bool, serving: bool) {
    if !test && !serving { mind::println!("[BCM] NO TEST AND NO STATION: THE CORE STAYS IN RESET"); return; }
    let Ok(file) = mind::fs::File::open(MICROCODE) else { mind::println!("[BCM] NO MICROCODE AT {} (FILE ACCESS OR scripts/proprietary.sh copy): STAGE 2 SKIPPED", MICROCODE); return };
    let Some(mut pages) = mind::mem::Pages::new(MICROCODE_MAX) else { mind::println!("[BCM] NO MEMORY FOR THE MICROCODE"); return };
    let buffer = pages.as_mut_slice();
    let size = match file.read_at(0, &mut buffer[..MICROCODE_MAX]) { Ok(n) if n == file.size() && n < MICROCODE_MAX => n, _ => { mind::println!("[BCM] MICROCODE UNREADABLE OR OVER {} BYTES", MICROCODE_MAX); return } };
    let image = &buffer[..size];
    let digest = mind::sha256::digest(image);
    mind::println!("[BCM] MICROCODE {}: {} BYTES, SHA-256 {:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}...", MICROCODE, size,
                   digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7]);
    // The header: type, version, two bytes of padding, a big-endian size; then big-endian 32-bit words.
    if size < 12 || image[0] != b'u' || image[1] != 1 || (size - 8) % 4 != 0 { mind::println!("[BCM] NOT A MICROCODE FILE OF VERSION 1 (TYPE {:02X}, VERSION {})", image.first().copied().unwrap_or(0), image.get(1).copied().unwrap_or(0)); return; }
    let words = image[8..].chunks_exact(4).map(|w| u32::from_be_bytes([w[0], w[1], w[2], w[3]]));

    step(1, "THE AGENT'S REGISTERS AND ERROR LOG READ");
    wrapper(bar, "BEFORE");
    let errors = AI_ERROR_LOG.map(|r| bar.read32(WRAPPER + r));
    mind::println!("[BCM] AGENT READ AND WRITE IDS {:08X} {:08X}; ERROR LOG CONTROL {:08X}, DONE {:08X}, STATUS {:08X}, ADDRESS {:08X}{:08X}, ID {:08X}, USER {:08X}, FLAGS {:08X}; INTERRUPT STATUS {:08X}",
                   bar.read32(WRAPPER + 0x808), bar.read32(WRAPPER + 0x80C), errors[0], errors[1], errors[2], errors[4], errors[3], errors[5], errors[6], errors[7], bar.read32(WRAPPER + 0xA00));
    // A pending operation means the kernel's stop before bus mastering went off did not take (550-KRN-0082): an access
    // to the core would stop the machine, so nothing further is tried.
    if let Err(v) = wait(bar, WRAPPER + AI_RESET_STATUS, 300, |v| v == 0) { mind::println!("[BCM] RESET STATUS {:08X}: A BACKPLANE OPERATION PENDING (550-KRN-0082 DID NOT TAKE), STAGE 2 NOT ATTEMPTED", v); return; }
    // The simplest access first: two registers of the core as the reset pulse left it, nothing else changed.
    step(2, "TWO REGISTERS OF THE IDLE CORE READ (MAC CONTROL, CLOCK CONTROL)");
    if bar.read32(WRAPPER + AI_RESET_CONTROL) & RESET == 0 {
        let mac = bar.read32(D11_MAC_CONTROL);
        mind::println!("[BCM] IDLE CORE: MAC CONTROL {:08X}, CLOCK CONTROL {:08X}", mac, bar.read32(D11_CLOCK));
    } else { mind::println!("[BCM] THE CORE IS HELD IN RESET: NOT READ"); }
    step(3, "THE CORE DISABLED AND ENABLED");
    if let Err(v) = wait(bar, WRAPPER + AI_RESET_STATUS, 300, |v| v == 0) { mind::println!("[BCM] RESET STATUS {:08X} AFTER THE IDLE READ: STAGE 2 STOPPED", v); return; }
    // Disabled first if it runs: held in reset with its clock off.
    if bar.read32(WRAPPER + AI_RESET_CONTROL) & RESET == 0 {
        bar.write32(WRAPPER + AI_RESET_CONTROL, RESET); let _ = bar.read32(WRAPPER + AI_RESET_CONTROL);
        pause(bar, 2);
        bar.write32(WRAPPER + AI_IOCONTROL, IO_GMODE); let _ = bar.read32(WRAPPER + AI_IOCONTROL);
        pause(bar, 10);
    }
    // The core out of reset with its clock forced, then running on its own clock, in G-mode.
    let flags = IO_GMODE;
    bar.write32(WRAPPER + AI_IOCONTROL, IO_CLOCK | IO_FORCE_GATED | flags); let _ = bar.read32(WRAPPER + AI_IOCONTROL);
    bar.write32(WRAPPER + AI_RESET_CONTROL, 0); let _ = bar.read32(WRAPPER + AI_RESET_CONTROL);
    pause(bar, 2);
    bar.write32(WRAPPER + AI_IOCONTROL, IO_CLOCK | flags); let _ = bar.read32(WRAPPER + AI_IOCONTROL);
    pause(bar, 2);
    wrapper(bar, "AFTER");
    // From here the core's registers answer.
    step(4, "THE CORE'S CLOCK CONTROL READ, HIGH-THROUGHPUT CLOCK FORCED");
    let clock = bar.read32(D11_CLOCK);
    bar.write32(D11_CLOCK, clock | CLOCK_FORCE_HT);
    if let Err(v) = wait(bar, D11_CLOCK, 10, |v| v & CLOCK_HAVE_HT != 0) { mind::println!("[BCM] NO HIGH-THROUGHPUT CLOCK (CLOCK CONTROL {:08X})", v); return stop(bar); }
    step(5, "THE PHY RESET");
    // The PHY reset at 20 MHz, then out of reset with its clock forced for a moment.
    let io = bar.read32(WRAPPER + AI_IOCONTROL);
    bar.write32(WRAPPER + AI_IOCONTROL, io | IO_PHY_RESET | IO_PHY_20MHZ); pause(bar, 3);
    let io = bar.read32(WRAPPER + AI_IOCONTROL);
    bar.write32(WRAPPER + AI_IOCONTROL, (io & !(IO_PHY_RESET | IO_PHY_CLOCK)) | IO_FORCE_GATED); pause(bar, 2);
    let io = bar.read32(WRAPPER + AI_IOCONTROL);
    bar.write32(WRAPPER + AI_IOCONTROL, (io & !IO_FORCE_GATED) | IO_PHY_CLOCK); pause(bar, 2);
    step(6, "THE 802.11 AND PHY PLLS REQUESTED");
    let clock = bar.read32(D11_CLOCK);
    bar.write32(D11_CLOCK, clock | CLOCK_PLL_REQUEST);
    if let Err(v) = wait(bar, D11_CLOCK, 100, |v| v & CLOCK_PLL_RUNNING == CLOCK_PLL_RUNNING) { mind::println!("[BCM] THE 802.11 AND PHY PLLS DO NOT RUN (CLOCK CONTROL {:08X})", v); return stop(bar); }
    let io = bar.read32(WRAPPER + AI_IOCONTROL);
    bar.write32(WRAPPER + AI_IOCONTROL, io | IO_MAC_PHY_CLOCK);
    step(7, "THE PHY AND RADIO VERSIONS READ");
    let phy = bar.read16(D11_PHY_VERSION);
    let mut radio = [0u16; 3];
    for (i, r) in radio.iter_mut().enumerate() { bar.write16(D11_RADIO_CONTROL, i as u16); *r = bar.read16(D11_RADIO_DATA); }
    // The radio: its ID from words 2 and 1, its revision in word 0's low nibble.
    mind::println!("[BCM] CORE ENABLED: IO CONTROL {:08X}, CLOCK CONTROL {:08X}; PHY {:04X} (TYPE {}, REVISION {}, ANALOG {}); RADIO {:04X} REVISION {} ({:04X} {:04X} {:04X})",
                   bar.read32(WRAPPER + AI_IOCONTROL), bar.read32(D11_CLOCK), phy, phy >> 8 & 0xF, phy & 0xFF, phy >> 12,
                   radio[2] << 8 | radio[1] & 0xFF, radio[0] & 0xF, radio[0], radio[1], radio[2]);

    step(8, "SHARED MEMORY CLEARED");
    // The processor held at address 0; scratch registers and shared memory cleared; the words written with auto-increment.
    bar.write32(D11_MAC_CONTROL, MAC_IHR | MAC_SHM | MAC_GMODE | MAC_INFRA);
    bar.write32(D11_MAC_CONTROL, bar.read32(D11_MAC_CONTROL) | MAC_PSM_JUMP0);
    for i in 0..64 { shm_select(bar, SHM_SCRATCH, i); bar.write16(D11_SHM_DATA, 0); }
    for i in 0..1024 { shm_select(bar, SHM_SHARED, i); bar.write32(D11_SHM_DATA, 0); }
    step(9, "THE MICROCODE WRITTEN");
    shm_select(bar, SHM_UCODE | SHM_AUTOINC_WRITE, 0);
    let mut count = 0;
    for word in words { bar.write32(D11_SHM_DATA, word); pause(bar, 10); count += 1; }
    step(10, "THE MICROCODE STARTED");
    bar.write32(D11_IRQ_REASON, u32::MAX);
    bar.write32(D11_MAC_CONTROL, (bar.read32(D11_MAC_CONTROL) & !MAC_PSM_JUMP0) | MAC_PSM_RUN);
    // The running microcode answers by suspending the MAC, which is not enabled.
    let answered = wait(bar, D11_IRQ_REASON, 1000, |v| v == IRQ_MAC_SUSPENDED);
    let shared = |offset: u32| { shm_select(bar, SHM_SHARED, offset / 4); bar.read16(D11_SHM_DATA + (offset % 4) as usize) };
    match answered {
        Ok(_) => {
            let (revision, patch, date, time) = (shared(0), shared(2), shared(4), shared(6));
            mind::println!("[BCM] MICROCODE RUNS: {} WORDS LOADED; REVISION {}.{}, DATE {:04X}, TIME {:04X}", count, revision, patch, date, time);
            let core = stage3::Core::new(bar);
            match mac {
                Some(mac) => if stage3::run(&core, mac) {
                    // The test's stages leave the radio running; for the station alone it comes straight up.
                    let radio = if test { stage4::run(&core, mac) } else { radio::Radio::up(&core, mac) };
                    if let Some(mut radio) = radio {
                        if serving {
                            if test { test_done(); }
                            serve::run(&mut radio);
                        }
                        mind::println!("[BCM] NO STATION TO SERVE: THE RADIO STOPS");
                        radio.stop();
                    }
                },
                None => mind::println!("[BCM] NO MAC ADDRESS FROM THE SPROM: STAGES 3 AND 4 SKIPPED"),
            }
        }
        Err(v) => mind::println!("[BCM] MICROCODE DOES NOT ANSWER: {} WORDS LOADED; IRQ REASON {:08X}, MAC CONTROL {:08X}", count, v, bar.read32(D11_MAC_CONTROL)),
    }
    bar.write32(D11_MAC_CONTROL, bar.read32(D11_MAC_CONTROL) & !MAC_PSM_RUN);
    stop(bar)
}

// The core reset again: its microcode and state gone, idle until a later stage sets it up.
fn stop(bar: &Mmio) { pulse(bar, "STAGE 2 DONE"); }

// A reset pulse: pending backplane operations end first (up to 1 ms), then reset asserted, released and read back.
fn pulse(bar: &Mmio, why: &str) {
    let before = bar.read32(WRAPPER + AI_RESET_STATUS);
    let settled = wait(bar, WRAPPER + AI_RESET_STATUS, 1, |v| v == 0).is_ok();
    bar.write32(WRAPPER + AI_RESET_CONTROL, RESET); let _ = bar.read32(WRAPPER + AI_RESET_CONTROL);
    pause(bar, 2);
    bar.write32(WRAPPER + AI_RESET_CONTROL, 0); let _ = bar.read32(WRAPPER + AI_RESET_CONTROL);
    pause(bar, 20);
    mind::println!("[BCM] {}: RESET PULSED (RESET STATUS {:08X} BEFORE{}, {:08X} AFTER; RESET {:08X}, IO CONTROL {:08X})", why, before,
                   if settled { "" } else { ", NOT CLEARED IN 1 MS" }, bar.read32(WRAPPER + AI_RESET_STATUS), bar.read32(WRAPPER + AI_RESET_CONTROL), bar.read32(WRAPPER + AI_IOCONTROL));
}

// Nothing is served yet: it stays, so init does not restart it.
fn idle() { loop { mind::time::sleep(60_000); } }
