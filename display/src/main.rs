#![no_std]
#![no_main]
// display (213-DRV-0086, DRV's): the Ryzen's Radeon display engine. It logs the grant as KRN's stub did (213-KRN-0270,
// 0271), then runs step 1 (the firmware's display state read in batches, as many as BATCHES_ALLOWED says) and steps 3
// and 4 (first light on HDMI-A-1, then MIND Core's picture, 213-DRV-0092, in stages, as many as STAGES_ALLOWED says; at 0
// nothing is written and no writer is made). SURVEY_ORDER chooses the survey's order or the production one (213-DRV-0098).
extern crate alloc;
#[allow(dead_code)] mod atom;
#[allow(dead_code)] mod batches;
#[allow(dead_code)] mod dcn;
#[allow(dead_code)] mod ddc;
#[allow(dead_code)] mod edid;
#[allow(dead_code)] mod light;
#[allow(dead_code)] mod picture;
mod screen;
#[allow(dead_code)] mod timing;
use alloc::boxed::Box;
use mind::abi::{BootInfo, CAP_KIND_MEMORY, CAP_KIND_MMIO, SLOT_DEV0, SLOT_DEV1, SLOT_IRQ, SLOT_MEM};
// The paths screen.rs is written against (its host cases model them).
use mind::{abi, dev, idl, ipc, sys};
use mind::dev::{cap_info, device_config, Mmio};

/// How many of step 1's eleven batches this build may read (the eleventh, HDMI-A-1's DDC line, for 213-DRV-0095). 0
/// maps no register and reads none: it logs each batch and the registers it would read. Raised one run at a time, on
/// the DRV owner session's word (213-DRV-0086).
const BATCHES_ALLOWED: u8 = 0;
/// How many of the stages this build may run: 1 to 8 step 3's (first light), 9 to 16 step 4's (213-DRV-0092). 0 makes no
/// writer: it logs each stage with every write it would make. Raised one run at a time, on the DRV owner session's word,
/// and only with BATCHES_ALLOWED at 10 or more (step 1's first ten).
const STAGES_ALLOWED: u8 = 0;
/// When true, step 2 reads HDMI-A-1's EDID over its DDC line before step 3 (213-DRV-0095), the I2C engine put back as
/// batch 11 found it; its result is logged only, and its failure leaves steps 3 and 4 as they are. Needs
/// BATCHES_ALLOWED 11. Set on the DRV owner session's word.
const EDID_ALLOWED: bool = false;
/// When true, steps 3 and 4 light HDMI-A-1 in HDMI signalling (213-DRV-0095): table 4 and 76 in HDMI mode, the HDMI
/// engine, its general control packet and an AVI infoframe; the mode stays 1920x1080 (VIC 16). Set on the DRV owner
/// session's word.
const HDMI_SIGNALLING: bool = false;
/// When true, with EDID_ALLOWED, steps 3 and 4 take the EDID's preferred timing in HDMI signalling when it fits
/// (`light::mode_from_edid`): the Ryzen's monitor's 3440x1440 at 60 Hz, 324 MHz; else today's mode. Set on the DRV owner
/// session's word.
const MODE_FROM_EDID: bool = false;
/// When true, the stages that ran are undone, last first, before display idles: the undo is seen on the hardware and
/// nothing is left for the next boot. Set on the DRV owner session's word, with STAGES_ALLOWED.
const UNDO_AFTER_STAGES: bool = false;
/// When true, steps 1 to 4 run in the survey's order, for bring-up: every batch named and paused, every value, write and
/// plan logged before it is made, 1 s before each stage's writes and 3 s after it. When false, in the production order
/// (213-DRV-0098): the same writes and checks in the same order, step 1 reading only what steps 2 to 4 take, no pause, a
/// line a stage with its time, the detail only when a part fails. Either way nothing is read or written at 0 batches
/// and 0 stages. Set on the DRV owner session's word.
const SURVEY_ORDER: bool = false;
/// Between two log lines, so that a burst is not dropped.
const LINE_PACE_MS: usize = 20;
/// Before step 1 in the survey's order, so that the boot's own lines are kept first.
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
    let started = now_us();
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
    let order = if SURVEY_ORDER { batches::Order::Survey } else { batches::Order::Production };
    let mut buf = alloc::vec![0u8; if SURVEY_ORDER { 0 } else { batches::HELD_BYTES }];
    let (mut survey, mut held) = (Log, batches::Held::new(Log, &mut buf));
    let out: &mut dyn batches::Out = if SURVEY_ORDER { &mut survey } else { &mut held };
    let mut store = batches::Store::default();
    let t1 = now_us();
    let (read, mmio) = step1(id, engine, order, &mut store, out);
    let t2 = now_us();
    let edid = step2(&store, mmio.as_ref(), out);
    let t3 = now_us();
    let (written, times) = step3(&store, mmio, vfct.as_ref().map(|t| t.as_slice()), edid.as_ref(), out);
    // How long each part took, from display's start (213-DRV-0098): to the offer when stage 16 made it.
    let ms = |us: u64| (us / 1000, us % 1000 / 100);
    let (a, b, c) = (ms(t1 - started), ms(t2 - t1), ms(t3 - t2));
    let name = if SURVEY_ORDER { "SURVEY'S" } else { "PRODUCTION" };
    match times.and_then(|t| t.offered_us.map(|o| t.start_us + o)) {
        Some(offered) => {
            let (d, e) = (ms(offered.saturating_sub(started)), ms(offered.saturating_sub(t3)));
            mind::println!("[DISPLAY] THE SECOND SCREEN OFFERED {}.{} MS AFTER DISPLAY'S START, IN THE {} ORDER: BEFORE STEP 1 {}.{} MS, STEP 1 {}.{} MS, STEP 2 {}.{} MS, STEP 3 TO THE OFFER {}.{} MS",
                d.0, d.1, name, a.0, a.1, b.0, b.1, c.0, c.1, e.0, e.1);
        }
        None => { let d = ms(now_us() - t3); mind::println!("[DISPLAY] NO SECOND SCREEN OFFERED, IN THE {} ORDER: BEFORE STEP 1 {}.{} MS, STEP 1 {}.{} MS, STEP 2 {}.{} MS, STEP 3 {}.{} MS", name, a.0, a.1, b.0, b.1, c.0, c.1, d.0, d.1) }
    }
    match (read, written) {
        (0, 0) => mind::println!("[DISPLAY] NO REGISTER READ OR WRITTEN"),
        (r, 0) => mind::println!("[DISPLAY] {} REGISTERS READ, NONE WRITTEN", r),
        (r, w) => mind::println!("[DISPLAY] {} REGISTERS READ BY STEP 1, {} WRITES BY STEP 3", r, w),
    }
    if let Ok(after) = device_config(ENGINE, 4) { mind::println!("[DISPLAY] COMMAND {:04X} AFTER STEP 1", after & 0xFFFF); }
    idle();
}

fn now_us() -> u64 { mind::time::monotonic_ns() / 1000 }

/// Step 1 on the engine's window: the registers it read, kept in `store`, and the window's mapping when it made one.
fn step1(id: u32, engine: usize, order: batches::Order, store: &mut batches::Store, out: &mut dyn batches::Out) -> (usize, Option<Mmio>) {
    mind::println!("[DISPLAY] STEP 1 MAY READ {} OF {} BATCHES, WRITES NOTHING; THE {} ORDER", BATCHES_ALLOWED, batches::BATCHES,
                   if SURVEY_ORDER { "SURVEY'S" } else { "PRODUCTION" });
    if id != RADEON { mind::println!("[DISPLAY] NOT THE RADEON 1002:1638: STEP 1 NOT RUN"); return (0, None); }
    if engine < dcn::DCN_BYTES.end as usize { mind::println!("[DISPLAY] THE ENGINE'S WINDOW IS BELOW {:#X} BYTES: STEP 1 NOT RUN", dcn::DCN_BYTES.end); return (0, None); }
    if SURVEY_ORDER { mind::time::sleep(START_PAUSE_MS); }
    if BATCHES_ALLOWED == 0 { return (batches::read_into(&mut Unmapped, 0, order, out, store).registers_read, None); }
    let Ok(mmio) = Mmio::map(ENGINE) else { mind::println!("[DISPLAY] THE ENGINE'S WINDOW NOT MAPPED: STEP 1 NOT RUN"); return (0, None) };
    let mut registers = Registers(mmio);
    let read = batches::read_into(&mut registers, BATCHES_ALLOWED, order, out, store).registers_read;
    (read, Some(registers.0))
}

/// Step 2 (213-DRV-0095): the EDID over DDC1, on step 1's mapping, and what it says.
fn step2(store: &batches::Store, mmio: Option<&Mmio>, out: &mut dyn batches::Out) -> Option<edid::Edid> {
    if !EDID_ALLOWED { mind::println!("[DISPLAY] STEP 2 (THE EDID OVER DDC1) NOT RUN: THIS BUILD DOES NOT ASK FOR IT"); return None; }
    let Some(mmio) = mmio else { mind::println!("[DISPLAY] STEP 2 NOT RUN: STEP 1 DID NOT MAP THE ENGINE'S WINDOW"); return None };
    let read = ddc::read(store, &mut Ddc(mmio), out).ok()?;
    ddc::describe(&read, out)
}

// Step 2's access to the engine: reads, the writes ddc.rs's guard lets through, and time.
struct Ddc<'a>(&'a Mmio);
impl batches::Bus for Ddc<'_> {
    fn read(&mut self, reg: dcn::Reg) -> Option<u32> {
        let offset = reg.offset() as usize;
        (offset + 4 <= self.0.len()).then(|| self.0.read32(offset))
    }
}
impl light::Poke for Ddc<'_> {
    fn write(&mut self, reg: light::Writable, value: u32) {
        let offset = reg.reg().offset() as usize;
        if offset + 4 <= self.0.len() { self.0.write32(offset, value); }
    }
}
impl light::Clock for Ddc<'_> {
    fn now_us(&mut self) -> u64 { mind::time::monotonic_ns() / 1000 }
    fn delay_us(&mut self, us: u32) {
        let end = mind::time::monotonic_ns() + us as u64 * 1000;
        while mind::time::monotonic_ns() < end { core::hint::spin_loop(); }
    }
}

/// Step 3: the writes it made, and its times when its stages ran. At STAGES_ALLOWED 0 it only describes the stages, for
/// the pipe step 1 found or, with step 1 not read, for the survey's expectation.
fn step3(store: &batches::Store, mmio: Option<Mmio>, vfct: Option<&[u8]>, edid: Option<&edid::Edid>, out: &mut dyn batches::Out) -> (usize, Option<light::Times>) {
    let today = timing::first_light_mode(&HDMI_A_1_VICS);
    let (found, expected) = match light::assess(store) {
        Ok(found) => (found, false),
        Err(why) if STAGES_ALLOWED == 0 => { mind::println!("[DISPLAY] STEP 3 SHOWN FOR THE SURVEY'S EXPECTATION: {}", why); (light::EXPECTED, true) }
        Err(why) => { mind::println!("[DISPLAY] STEP 3 NOT RUN: {}", why); return (0, None); }
    };
    let signal = if HDMI_SIGNALLING { dcn::Signal::Hdmi } else { dcn::Signal::Dvi };
    let (timing, signal) = if MODE_FROM_EDID {
        let (timing, signal, why) = light::mode_from_edid(edid, today, signal);
        mind::println!("[DISPLAY] STEP 3'S MODE: {}x{} AT {} KHZ IN {:?} SIGNALLING, {}", timing.h_active, timing.v_active, timing.pixel_khz, signal, why);
        (timing, signal)
    } else { (today, signal) };
    let plan = match light::Light::new_with(found.0, found.1, timing, signal) {
        Ok(plan) => plan.with_reference(light::reference(store, &found.0, &found.1)).with_build(option_env!("MIND_BUILD_COMMIT").unwrap_or("UNKNOWN")),
        Err(why) => { mind::println!("[DISPLAY] STEP 3 NOT RUN: {}", why); return (0, None); }
    };
    if STAGES_ALLOWED == 0 { light::describe(&plan, expected, out); return (0, None); }
    // Only here is a writer made: a build of STAGES_ALLOWED 0 has none.
    let Some(mmio) = mmio else { mind::println!("[DISPLAY] STEP 3 NOT RUN: STEP 1 DID NOT MAP THE ENGINE'S WINDOW"); return (0, None) };
    let Some((0x1002, 0x1638, image)) = vfct.and_then(atom::vfct_image) else { mind::println!("[DISPLAY] STEP 3 NOT RUN: NO VBIOS IMAGE FOR 1002:1638"); return (0, None) };
    let Ok(image) = atom::Image::new(image) else { mind::println!("[DISPLAY] STEP 3 NOT RUN: THE VBIOS IMAGE HAS NO ATOM ROM HEADER"); return (0, None) };
    let mut interp = Box::new(atom::Interpreter::new(image));
    let mut journal = Box::new(light::Journal::default());
    // Step 4's surface: the start of BAR 0's slot 5, when granted (213-KRN-0271).
    let surface = Mmio::map(SECOND_SCREEN).ok();
    let (_, times) = light::run_timed(&plan, &mut Writer(mmio, surface), &mut interp, &mut journal, STAGES_ALLOWED, UNDO_AFTER_STAGES, out);
    (journal.len(), Some(times))
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

/// Steps 3 and 4's engine: reads, writes (only a `light::Writable`), time, and step 4's surface in BAR 0's slot 5
/// (its start, write-combined). Made only when STAGES_ALLOWED is above 0.
struct Writer(Mmio, Option<Mmio>);
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
// Stage 16 through screen.rs's client of the compositor, slot 5 lent as its pixels.
impl light::Screens for Writer {
    fn offer(&mut self, width: u16, height: u16, pitch: u32) -> Result<(), light::ScreenRefused> { screen::Compositor { pixels: SECOND_SCREEN }.offer(width, height, pitch) }
    fn gone(&mut self) -> Result<(), light::ScreenRefused> { screen::Compositor { pixels: SECOND_SCREEN }.gone() }
}
impl light::Canvas for Writer {
    fn row(&mut self, y: usize, pitch: usize, pixels: &[u32]) -> bool {
        let (Some(surface), start) = (&self.1, y * pitch * 4) else { return false };
        if pixels.len() > pitch || start + pixels.len() * 4 > surface.len() { return false; }
        for (x, p) in pixels.iter().enumerate() { surface.write32(start + x * 4, *p); }
        true
    }
    fn sample(&mut self, x: usize, y: usize, pitch: usize) -> Option<u32> {
        let surface = self.1.as_ref()?;
        let at = (y * pitch + x) * 4;
        (x < pitch && at + 4 <= surface.len()).then(|| surface.read32(at))
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
