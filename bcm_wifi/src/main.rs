#![no_std]
#![no_main]
// Ring 3 driver for the MacBook Pro's Broadcom BCM4331 Wi-Fi (550-DRV-0006). Stage 1 (550-DRV-0020) only reads: the
// chip behind its 16 KiB BAR0, the 802.11 core's state from its wrapper, and the SPROM; it writes nothing to the chip.
// Register facts come from Broadcom's published headers and the b43 specifications; no driver code is taken.
use mind::abi::{BootInfo, SLOT_DEV0};
use mind::dev::{device_config, Mmio};

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
// Where the SPROM's contents appear in ChipCommon; which one depends on revisions, so both are tried.
const SPROM_AT: [usize; 2] = [0x800, 0x830];
const SPROM_WORDS: usize = 220; // revision 8 and later: 440 bytes

// Wrapper (agent) registers of an AI backplane core.
const AI_IOCONTROL: usize = 0x408; const AI_IOSTATUS: usize = 0x500; const AI_RESET_CONTROL: usize = 0x800; const AI_RESET_STATUS: usize = 0x804;

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

    let chip = bar.read32(CC + CC_CHIP_ID);
    let (number, revision, package, cores, bus) = (chip & 0xFFFF, chip >> 16 & 0xF, chip >> 20 & 0xF, chip >> 24 & 0xF, chip >> 28);
    mind::println!("[BCM] CHIP {:04X} REVISION {} PACKAGE {} CORES {} BUS {}", number, revision, package, cores, if bus == 1 { "AI" } else { "OTHER" });
    let capabilities = bar.read32(CC + CC_CAPABILITIES);
    mind::println!("[BCM] CHIPCOMMON CAPABILITIES {:08X}, CHIP CONTROL {:08X}, STATUS {:08X}, SROM CONTROL {:08X}, EROM AT {:08X}",
                   capabilities, bar.read32(CC + CC_CHIP_CONTROL), bar.read32(CC + CC_CHIP_STATUS), bar.read32(CC + CC_SROM_CONTROL), bar.read32(CC + CC_EROM));
    if number != 0x4331 || bus != 1 { mind::println!("[BCM] NOT A BCM4331 ON AN AI BACKPLANE: STOPPED"); return idle(); }

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
    mind::println!("[BCM] PCIE CORE: {:08X} {:08X} {:08X} {:08X}", bar.read32(PCIE), bar.read32(PCIE + 4), bar.read32(PCIE + 8), bar.read32(PCIE + 12));
    let _ = WINDOW;

    if capabilities & CAP_SPROM == 0 { mind::println!("[BCM] NO SPROM FITTED (OTP SIZE CODE {})", capabilities >> 19 & 7); return idle(); }
    let mut found = false;
    for at in SPROM_AT {
        let mut words = [0u16; SPROM_WORDS];
        for (i, w) in words.iter_mut().enumerate() { *w = bar.read16(CC + at + i * 2); }
        match sprom_valid(&words) {
            Some(revision) => {
                // Revision 8 and later: the MAC address in words 0x46..0x48, big-endian per word.
                let mac = [words[0x46] >> 8, words[0x46] & 0xFF, words[0x47] >> 8, words[0x47] & 0xFF, words[0x48] >> 8, words[0x48] & 0xFF];
                mind::println!("[BCM] SPROM AT CHIPCOMMON+{:03X}: REVISION {}, CRC GOOD, MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}, WORDS 0-3 {:04X} {:04X} {:04X} {:04X}",
                               at, revision, mac[0], mac[1], mac[2], mac[3], mac[4], mac[5], words[0], words[1], words[2], words[3]);
                found = true;
            }
            None => mind::println!("[BCM] SPROM AT CHIPCOMMON+{:03X}: NOT VALID (FIRST WORDS {:04X} {:04X} {:04X} {:04X}, LAST {:04X}, CRC OF THE REST {:02X})",
                                   at, words[0], words[1], words[2], words[3], words[SPROM_WORDS - 1], sprom_crc(&words)),
        }
    }
    if !found { mind::println!("[BCM] NO VALID SPROM: ITS PINS MAY BE SHARED WITH THE EXTERNAL AMPLIFIER LINES"); }
    idle()
}

// Stage 1 serves nothing yet: it stays, so init does not restart it.
fn idle() { loop { mind::time::sleep(60_000); } }
