#![no_std]
#![no_main]
// display (213-DRV-0086, DRV's): the Ryzen's Radeon display engine. It logs the grant as KRN's stub did (213-KRN-0270,
// 0271), then runs step 1 (the firmware's display state read in batches, as many as BATCHES_ALLOWED says) and step 3
// (first light on HDMI-A-1 in stages, as many as STAGES_ALLOWED says; at 0 nothing is written and no writer is made).
extern crate alloc;
#[allow(dead_code)] mod atom;
#[allow(dead_code)] mod batches;
#[allow(dead_code)] mod dcn;
#[allow(dead_code)] mod edid;
#[allow(dead_code)] mod light;
#[allow(dead_code)] mod timing;
use alloc::boxed::Box;
use mind::abi::{BootInfo, CAP_KIND_MEMORY, CAP_KIND_MMIO, SLOT_DEV0, SLOT_DEV1, SLOT_IRQ, SLOT_MEM};
use mind::dev::{cap_info, device_config, Mmio};

/// How many of step 1's eight batches this build may read. 0 maps no register and reads none: it logs each batch and the
/// registers it would read. Raised one run at a time, on the DRV owner session's word (213-DRV-0086).
const BATCHES_ALLOWED: u8 = 0;
/// How many of step 3's eight stages this build may run. 0 makes no writer: it logs each stage with every write it would
/// make. Raised one run at a time, on the DRV owner session's word, and only with BATCHES_ALLOWED at 8 (all of step 1).
const STAGES_ALLOWED: u8 = 0;
/// When true, the stages that ran are undone, last first, before display idles: the undo is seen on the hardware and
/// nothing is left for the next boot. Set on the DRV owner session's word, with STAGES_ALLOWED.
const UNDO_AFTER_STAGES: bool = false;
/// Between two log lines, so that a burst is not dropped.
const LINE_PACE_MS: usize = 20;
/// Before step 1, so that the boot's own lines are kept first.
const START_PAUSE_MS: usize = 5000;
const RADEON: u32 = 0x1638_1002;

// init's slots for display (213-KRN-0271): the engine's registers (BAR 5 from byte 0), the SMU mailbox's page (BAR 5 from
// 0x58000), the second screen's range of BAR 0, and the VBIOS image (the ACPI VFCT table, read-only) where an interrupt
// would be: the display has none of its own.
const ENGINE: usize = SLOT_DEV0;
const MAILBOX: usize = SLOT_DEV1;
const SECOND_SCREEN: usize = SLOT_MEM;
const VBIOS: usize = SLOT_IRQ;

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let device = mind::process::args_str().trim();
    let (kind, _, engine) = cap_info(ENGINE);
    if kind != CAP_KIND_MMIO { mind::println!("[DISPLAY] NO REGISTERS GRANTED"); idle(); }
    let (Ok(id), Ok(command), Ok(bar)) = (device_config(ENGINE, 0), device_config(ENGINE, 4), device_config(ENGINE, 0x24)) else {
        mind::println!("[DISPLAY] PCI CONFIG UNAVAILABLE");
        idle();
    };
    mind::println!("[DISPLAY] PCI {:04X}:{:04X} (DEVICE {}), COMMAND {:04X}, BAR 5 AT {:08X}: {} KIB OF ITS REGISTERS GRANTED",
                   id & 0xFFFF, id >> 16, device, command & 0xFFFF, bar & !0xF, engine >> 10);
    let size = |slot: usize, kind: usize| { let (k, _, len) = cap_info(slot); (k == kind).then_some(len) };
    match size(MAILBOX, CAP_KIND_MMIO) { Some(len) => mind::println!("[DISPLAY] THE SMU MAILBOX: {} BYTES GRANTED", len), None => mind::println!("[DISPLAY] NO SMU MAILBOX GRANTED") }
    match size(SECOND_SCREEN, CAP_KIND_MMIO) { Some(len) => mind::println!("[DISPLAY] BAR 0 FOR A SECOND SCREEN: {} KIB GRANTED", len >> 10), None => mind::println!("[DISPLAY] NO SECOND SCREEN'S MEMORY GRANTED") }
    let vfct = size(VBIOS, CAP_KIND_MEMORY).and_then(|_| mind::mem::Mapping::new(VBIOS).ok());
    match &vfct {
        Some(table) => { let length = table.as_slice().get(4..8).map_or(0, |l| u32::from_le_bytes([l[0], l[1], l[2], l[3]]) as usize); vbios(&table.as_slice()[..length.min(table.len())]) }
        None => mind::println!("[DISPLAY] NO VBIOS IMAGE GRANTED"),
    }
    let mut store = batches::Store::default();
    let (read, mmio) = step1(id, engine, &mut store);
    let written = step3(&store, mmio, vfct.as_ref().map(|t| t.as_slice()));
    match (read, written) {
        (0, 0) => mind::println!("[DISPLAY] NO REGISTER READ OR WRITTEN"),
        (r, 0) => mind::println!("[DISPLAY] {} REGISTERS READ, NONE WRITTEN", r),
        (r, w) => mind::println!("[DISPLAY] {} REGISTERS READ BY STEP 1, {} WRITES BY STEP 3", r, w),
    }
    if let Ok(after) = device_config(ENGINE, 4) { mind::println!("[DISPLAY] COMMAND {:04X} AFTER STEP 1", after & 0xFFFF); }
    idle();
}

/// Step 1 on the engine's window: the registers it read, kept in `store`, and the window's mapping when it made one.
fn step1(id: u32, engine: usize, store: &mut batches::Store) -> (usize, Option<Mmio>) {
    mind::println!("[DISPLAY] STEP 1 MAY READ {} OF {} BATCHES, WRITES NOTHING", BATCHES_ALLOWED, batches::BATCHES);
    if id != RADEON { mind::println!("[DISPLAY] NOT THE RADEON 1002:1638: STEP 1 NOT RUN"); return (0, None); }
    if engine < dcn::DCN_BYTES.end as usize { mind::println!("[DISPLAY] THE ENGINE'S WINDOW IS BELOW {:#X} BYTES: STEP 1 NOT RUN", dcn::DCN_BYTES.end); return (0, None); }
    mind::time::sleep(START_PAUSE_MS);
    if BATCHES_ALLOWED == 0 { return (batches::run_into(&mut Unmapped, 0, &mut Log, store).registers_read, None); }
    let Ok(mmio) = Mmio::map(ENGINE) else { mind::println!("[DISPLAY] THE ENGINE'S WINDOW NOT MAPPED: STEP 1 NOT RUN"); return (0, None) };
    let mut registers = Registers(mmio);
    let read = batches::run_into(&mut registers, BATCHES_ALLOWED, &mut Log, store).registers_read;
    (read, Some(registers.0))
}

/// Step 3: the writes it made. At STAGES_ALLOWED 0 it only describes the stages, for the pipe step 1 found or, with
/// step 1 not read, for the survey's expectation.
fn step3(store: &batches::Store, mmio: Option<Mmio>, vfct: Option<&[u8]>) -> usize {
    let timing = timing::first_light_mode(&HDMI_A_1_VICS);
    let (found, expected) = match light::assess(store) {
        Ok(found) => (found, false),
        Err(why) if STAGES_ALLOWED == 0 => { mind::println!("[DISPLAY] STEP 3 SHOWN FOR THE SURVEY'S EXPECTATION: {}", why); (light::EXPECTED, true) }
        Err(why) => { mind::println!("[DISPLAY] STEP 3 NOT RUN: {}", why); return 0; }
    };
    let plan = match light::Light::new(found.0, found.1, timing) {
        Ok(plan) => plan,
        Err(why) => { mind::println!("[DISPLAY] STEP 3 NOT RUN: {}", why); return 0; }
    };
    if STAGES_ALLOWED == 0 { light::describe(&plan, expected, &mut Log); return 0; }
    // Only here is a writer made: a build of STAGES_ALLOWED 0 has none.
    let Some(mmio) = mmio else { mind::println!("[DISPLAY] STEP 3 NOT RUN: STEP 1 DID NOT MAP THE ENGINE'S WINDOW"); return 0 };
    let Some((0x1002, 0x1638, image)) = vfct.and_then(atom::vfct_image) else { mind::println!("[DISPLAY] STEP 3 NOT RUN: NO VBIOS IMAGE FOR 1002:1638"); return 0 };
    let Ok(image) = atom::Image::new(image) else { mind::println!("[DISPLAY] STEP 3 NOT RUN: THE VBIOS IMAGE HAS NO ATOM ROM HEADER"); return 0 };
    let mut interp = Box::new(atom::Interpreter::new(image));
    let mut journal = Box::new(light::Journal::default());
    if UNDO_AFTER_STAGES {
        light::run_then_undo(&plan, &mut Writer(mmio), &mut interp, &mut journal, STAGES_ALLOWED, &mut Log);
    } else {
        light::run(&plan, &mut Writer(mmio), &mut interp, &mut journal, STAGES_ALLOWED, &mut Log);
    }
    journal.len()
}

/// The VICs HDMI-A-1's EDID lists (read under Linux, 2026-11-21); the EDID over DDC is step 2.
const HDMI_A_1_VICS: [u8; 17] = [1, 2, 3, 4, 5, 16, 17, 18, 19, 20, 31, 63, 93, 94, 95, 96, 97];



// The VFCT's first image (AMD's layout, as Linux's amdgpu_bios.c reads it): its PCI identity, length and ROM signature.
fn vbios(table: &[u8]) {
    let u16_at = |at: usize| table.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |at: usize| table.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let image = u32_at(52).map(|offset| offset as usize);
    match image.and_then(|at| Some((u16_at(at + 12)?, u16_at(at + 14)?, u32_at(at + 24)? as usize, at + 28))) {
        Some((vendor, id, length, at)) if table.starts_with(b"VFCT") => {
            let rom = table.get(at..at + 2) == Some(&[0x55, 0xAA][..]);
            mind::println!("[DISPLAY] VBIOS: VFCT {} BYTES, AN IMAGE FOR {:04X}:{:04X} OF {} BYTES, ROM SIGNATURE {}",
                           table.len(), vendor, id, length, if rom { "55AA" } else { "MISSING" });
        }
        _ => mind::println!("[DISPLAY] VBIOS: THE GRANTED TABLE IS NOT A VFCT ONE CAN READ"),
    }
}

/// The engine's window of BAR 5 (its byte 0 is BAR 5's): only a `dcn::Reg`, which can name no MSI-X byte, and only within
/// the window. The SMU mailbox is in another slot and is not read by step 1.
struct Registers(Mmio);
impl batches::Bus for Registers {
    fn read(&mut self, reg: dcn::Reg) -> Option<u32> {
        let offset = reg.offset() as usize;
        (offset + 4 <= self.0.len()).then(|| self.0.read32(offset))
    }
}

/// Step 3's engine: reads, writes (only a `light::Writable`) and time. Made only when STAGES_ALLOWED is above 0.
struct Writer(Mmio);
impl batches::Bus for Writer {
    fn read(&mut self, reg: dcn::Reg) -> Option<u32> {
        let offset = reg.offset() as usize;
        (offset + 4 <= self.0.len()).then(|| self.0.read32(offset))
    }
}
impl light::Poke for Writer {
    fn write(&mut self, reg: light::Writable, value: u32) {
        let offset = reg.reg().offset() as usize;
        if offset + 4 <= self.0.len() { self.0.write32(offset, value); }
    }
}
impl light::Clock for Writer {
    fn now_us(&mut self) -> u64 { mind::time::monotonic_ns() / 1000 }
    fn delay_us(&mut self, us: u32) {
        if us >= 1000 { mind::time::sleep(us as usize / 1000); return; }
        let end = mind::time::monotonic_ns() + us as u64 * 1000;
        while mind::time::monotonic_ns() < end { core::hint::spin_loop(); }
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

// The grant held, nothing more done with it.
fn idle() -> ! { loop { mind::time::sleep(3_600_000); } }
