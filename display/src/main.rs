#![no_std]
#![no_main]
// display (213-DRV-0086, DRV's): the Ryzen's Radeon display engine. It logs the grant as KRN's stub did (213-KRN-0270,
// 0271), then does what the setting after the device's index in its argument says (213-DRV-0099, setting.rs): `off`,
// also with no word or one it does not know, maps no register; `read` runs step 1 (the firmware's display state read in
// batches) and describes the stages; `light` runs steps 1 to 4 in the production order (213-DRV-0098: the EDID, first
// light on HDMI-A-1 and MIND Core's picture, 213-DRV-0092 and 0095), then follows HPD 1 (213-DRV-0096) and reads both
// pipes' underflow statuses (213-DRV-0103). HDMI-A-2, the second HDMI socket, is read and its HPD followed, and it is
// driven as far as HDMI_A_2 below lets it (213-DRV-0112). start.rs is that order, on the Radeon's windows here.
extern crate alloc;
#[allow(dead_code)] mod atom;
#[allow(dead_code)] mod batches;
#[allow(dead_code)] mod dcn;
#[allow(dead_code)] mod ddc;
#[allow(dead_code)] mod edid;
#[allow(dead_code)] mod hpd;
#[allow(dead_code)] mod light;
#[allow(dead_code)] mod picture;
mod screen;
#[allow(dead_code)] mod setting;
#[allow(dead_code)] mod start;
#[allow(dead_code)] mod timing;
use mind::abi::{BootInfo, CAP_KIND_MEMORY, CAP_KIND_MMIO, SLOT_DEV0, SLOT_DEV1, SLOT_IRQ, SLOT_MEM};
// The paths screen.rs is written against (its host cases model them).
use mind::{abi, dev, idl, ipc, sys};
use mind::dev::{cap_info, device_config, Mmio};

/// Bring-up only, all off in a release (213-DRV-0099): they change only what the `light` setting does, and a build with
/// one set says so in its first line. SURVEY_ORDER: steps 1 to 4 in the survey's order of boots 70 to 81, every batch
/// named and paused, every value, write and plan logged before it is made, 1 s before each stage's writes and 3 s after
/// it; else the production order (213-DRV-0098). UNDO_AFTER_STAGES: the stages that ran are undone, last first, before
/// display idles, and nothing follows them. STAGES_ALLOWED: at most this many of the 16 stages (1 to 8 first light, 9 to
/// 16 the picture); below 15 no standing check, at 0 the stages only described. HDMI_A_2 (213-DRV-0112): how far display
/// goes on the Ryzen's second HDMI socket when its monitor is the one there: Read (the release's) reads its blocks only
/// (step 1's batch 12, again at each of its plugs), Edid also reads its EDID over DDC2 and describes its stages,
/// FirstLight also runs first light's 8 stages there, Full all 16 (STAGES_ALLOWED bounds both outputs). HDMI_FIRST: the
/// socket taken when both have a monitor, HdmiA1 (the release's) or HdmiA2 (HDMI-A-1 then dark, named as not driven).
const SURVEY_ORDER: bool = true; // BENCH ONLY, NEVER TO LAND
const UNDO_AFTER_STAGES: bool = false; // BENCH ONLY, NEVER TO LAND
const STAGES_ALLOWED: u8 = dcn::STAGES;
const HDMI_A_2: setting::HdmiA2 = setting::HdmiA2::FirstLight; // BENCH ONLY, NEVER TO LAND
const HDMI_FIRST: setting::HdmiFirst = setting::HdmiFirst::HdmiA2; // BENCH ONLY, NEVER TO LAND
const BRINGUP: setting::Bringup = setting::Bringup { survey_order: SURVEY_ORDER, undo_after_stages: UNDO_AFTER_STAGES, stages_allowed: STAGES_ALLOWED,
                                                     hdmi_a_2: HDMI_A_2, hdmi_first: HDMI_FIRST };
/// Between two log lines, so that a burst is not dropped.
const LINE_PACE_MS: usize = 20;

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
    let args = setting::parse(mind::process::args_str());
    if !BRINGUP.is_release() { mind::println!("[DISPLAY] {}", BRINGUP); }
    mind::println!("[DISPLAY] {}", args);
    let plan = setting::plan(args.setting, BRINGUP);
    let (kind, _, engine) = cap_info(ENGINE);
    if kind != CAP_KIND_MMIO { mind::println!("[DISPLAY] NO REGISTERS GRANTED"); idle(); }
    let (Ok(id), Ok(command), Ok(bar)) = (device_config(ENGINE, 0), device_config(ENGINE, 4), device_config(ENGINE, 0x24)) else {
        mind::println!("[DISPLAY] PCI CONFIG UNAVAILABLE");
        idle();
    };
    mind::println!("[DISPLAY] PCI {:04X}:{:04X} (DEVICE {}), COMMAND {:04X}, BAR 5 AT {:08X}: {} KIB OF ITS REGISTERS GRANTED",
                   id & 0xFFFF, id >> 16, args.device, command & 0xFFFF, bar & !0xF, engine >> 10);
    let size = |slot: usize, kind: usize| { let (k, _, len) = cap_info(slot); (k == kind).then_some(len) };
    match size(MAILBOX, CAP_KIND_MMIO) { Some(len) => mind::println!("[DISPLAY] THE SMU MAILBOX: {} BYTES GRANTED", len), None => mind::println!("[DISPLAY] NO SMU MAILBOX GRANTED") }
    match size(SECOND_SCREEN, CAP_KIND_MMIO) { Some(len) => mind::println!("[DISPLAY] BAR 0 FOR A SECOND SCREEN: {} KIB GRANTED", len >> 10), None => mind::println!("[DISPLAY] NO SECOND SCREEN'S MEMORY GRANTED") }
    let vfct = size(VBIOS, CAP_KIND_MEMORY).and_then(|_| mind::mem::Mapping::new(VBIOS).ok());
    match &vfct {
        Some(table) => { let length = table.as_slice().get(4..8).map_or(0, |l| u32::from_le_bytes([l[0], l[1], l[2], l[3]]) as usize); vbios(&table.as_slice()[..length.min(table.len())]) }
        None => mind::println!("[DISPLAY] NO VBIOS IMAGE GRANTED"),
    }
    let survey = plan.order == batches::Order::Survey;
    let mut buf = alloc::vec![0u8; if survey { 0 } else { batches::HELD_BYTES }];
    let (mut log, mut held) = (Log, batches::Held::new(Log, &mut buf));
    let out: &mut dyn batches::Out = if survey { &mut log } else { &mut held };
    let image = vfct.as_ref().map(|t| t.as_slice());
    let build = option_env!("MIND_BUILD_COMMIT").unwrap_or("UNKNOWN");
    let mut radeon = Radeon;
    let begun = start::start(&mut radeon, &plan, id, engine, image, build, started, out);
    if let Ok(after) = device_config(ENGINE, 4) { mind::println!("[DISPLAY] COMMAND {:04X} AFTER STEP 1", after & 0xFFFF); }
    if let Some(mut w) = begun.writer {
        if let Some(mut follower) = start::Follower::new(&mut radeon, &mut w, begun.run, &plan) {
            loop {
                mind::time::sleep(follower.sleep_ms());
                follower.poll(&mut radeon, &mut w, &plan, image, build, out);
            }
        }
    }
    idle();
}

fn now_us() -> u64 { mind::time::monotonic_ns() / 1000 }

/// The Radeon as init grants it: the engine's window of BAR 5 mapped only when step 1 reads, its writer only when the
/// setting writes, with BAR 0's slot 5 for step 4's surface.
struct Radeon;
impl start::Device for Radeon {
    type Window = Registers;
    type Writer = Writer;
    fn map(&mut self) -> Option<Registers> { Mmio::map(ENGINE).ok().map(Registers) }
    fn writer(&mut self, window: Registers) -> Writer { Writer(window.0, Mmio::map(SECOND_SCREEN).ok()) }
    fn now_us(&mut self) -> u64 { now_us() }
    fn say(&mut self, args: core::fmt::Arguments) { mind::println!("[DISPLAY] {}", args); }
}

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

/// Step 2's and steps 3 and 4's engine: reads, writes (only a `light::Writable`), time, and step 4's surface in BAR 0's slot 5
/// (its start, write-combined). Made only when the setting writes (start.rs).
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

struct Log;
impl batches::Out for Log {
    fn line(&mut self, args: core::fmt::Arguments) { mind::println!("[DISPLAY] {}", args); mind::time::sleep(LINE_PACE_MS); }
    fn pause(&mut self, ms: u32) { mind::time::sleep(ms as usize); }
}

// The grant held, nothing more done with it.
fn idle() -> ! { loop { mind::time::sleep(3_600_000); } }
