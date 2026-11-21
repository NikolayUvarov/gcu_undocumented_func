#![no_std]
#![no_main]
// display (213-DRV-0086, DRV's): the Ryzen's Radeon display engine. Step 1: the grant logged as KRN's stub did
// (213-KRN-0270), then the firmware's display state read in batches, as many as BATCHES_ALLOWED says; nothing is written.
#[allow(dead_code)] mod batches;
#[allow(dead_code)] mod dcn;
#[allow(dead_code)] mod edid;
#[allow(dead_code)] mod timing;
use mind::abi::{BootInfo, CAP_KIND_MMIO, SLOT_DEV0};
use mind::dev::{cap_info, device_config, Mmio};

/// How many of step 1's seven batches this build may read. 0 maps nothing and reads nothing: it logs each batch and the
/// registers it would read. Raised one run at a time, on the DRV owner session's word (213-DRV-0086).
const BATCHES_ALLOWED: u8 = 0;
/// Between two log lines, so that a burst is not dropped.
const LINE_PACE_MS: usize = 20;
/// Before step 1, so that the boot's own lines are kept first.
const START_PAUSE_MS: usize = 5000;
const RADEON: u32 = 0x1638_1002;

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (kind, _, len) = cap_info(SLOT_DEV0);
    let device = mind::process::args_str().trim();
    if kind != CAP_KIND_MMIO { mind::println!("[DISPLAY] NO REGISTERS GRANTED"); idle(); }
    let (Ok(id), Ok(command), Ok(bar)) = (device_config(SLOT_DEV0, 0), device_config(SLOT_DEV0, 4), device_config(SLOT_DEV0, 0x24)) else {
        mind::println!("[DISPLAY] PCI CONFIG UNAVAILABLE");
        idle();
    };
    mind::println!("[DISPLAY] PCI {:04X}:{:04X} (DEVICE {}), COMMAND {:04X}, BAR 5 AT {:08X}, {} KIB GRANTED; STEP 1 MAY READ {} OF {} BATCHES, WRITES NOTHING",
                   id & 0xFFFF, id >> 16, device, command & 0xFFFF, bar & !0xF, len >> 10, BATCHES_ALLOWED, batches::BATCHES);
    if id != RADEON { mind::println!("[DISPLAY] NOT THE RADEON 1002:1638: STEP 1 NOT RUN"); idle(); }
    if len < dcn::BAR5_BYTES as usize { mind::println!("[DISPLAY] BAR 5 GRANT BELOW 512 KIB: STEP 1 NOT RUN"); idle(); }
    mind::time::sleep(START_PAUSE_MS);
    if BATCHES_ALLOWED == 0 {
        batches::run(&mut Unmapped, 0, &mut Log);
    } else {
        let Ok(mmio) = Mmio::map(SLOT_DEV0) else { mind::println!("[DISPLAY] BAR 5 NOT MAPPED: STEP 1 NOT RUN"); idle() };
        batches::run(&mut Registers(mmio), BATCHES_ALLOWED, &mut Log);
    }
    if let Ok(after) = device_config(SLOT_DEV0, 4) { mind::println!("[DISPLAY] COMMAND {:04X} AFTER STEP 1", after & 0xFFFF); }
    idle();
}

/// BAR 5 read through the grant: only a `dcn::Reg`, which can name no MSI-X byte, and only within the mapping.
struct Registers(Mmio);
impl batches::Bus for Registers {
    fn read(&mut self, reg: dcn::Reg) -> Option<u32> {
        let offset = reg.offset() as usize;
        (offset + 4 <= self.0.len()).then(|| self.0.read32(offset))
    }
}

/// The build that may read nothing has nothing mapped.
struct Unmapped;
impl batches::Bus for Unmapped { fn read(&mut self, _: dcn::Reg) -> Option<u32> { None } }

struct Log;
impl batches::Out for Log {
    fn line(&mut self, args: core::fmt::Arguments) { mind::println!("[DISPLAY] {}", args); mind::time::sleep(LINE_PACE_MS); }
    fn pause(&mut self, ms: u32) { mind::time::sleep(ms as usize); }
}

// The grant held, nothing done with it.
fn idle() -> ! { loop { mind::time::sleep(3_600_000); } }
