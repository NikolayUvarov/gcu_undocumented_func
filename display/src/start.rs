//! 213-DRV-0099: display's start as the setting's plan asks (`setting::Plan`): step 1 on the engine's window, step 2 and
//! steps 3 and 4 on its writer, what each part took and what was read and written; then what follows the stages, HPD 1
//! (213-DRV-0096) and the standing check (213-DRV-0103). Steps 2 to 4 take the HDMI socket whose HPD senses a monitor,
//! HDMI-A-1 first, and both senses are followed after them, HDMI-A-2 as far as the plan lets it (213-DRV-0112). The other
//! socket's monitor gets a run of its own beside the first, on the next free pipe, as SECOND_HDMI asks (213-DRV-0113);
//! with it off one socket is lit at a time (the engine has four pipes). main.rs runs it on the Radeon's windows, the host cases
//! (`tests/display_light_host.rs`) on a model: a plan that reads nothing maps nothing, one that writes nothing makes no
//! writer.

use crate::batches::{self, Bus, Order, Out, Store};
use crate::dcn::{self, field::{DC_HPD_EN, DC_HPD_SENSE}, Output, Reg, R};
use crate::light::Clock;
use crate::setting::{self, Plan};
use crate::{atom, ddc, edid, hpd, light, timing};
use alloc::boxed::Box;
use core::fmt;

pub const RADEON: u32 = 0x1638_1002;
/// Before step 1 in the survey's order, so that the boot's own lines are kept first.
pub const START_PAUSE_MS: u32 = 5000;
/// The VICs HDMI-A-1's EDID lists (read under Linux, 2026-11-21): the mode when the EDID gives none (step 2 reads it).
pub const HDMI_A_1_VICS: [u8; 17] = [1, 2, 3, 4, 5, 16, 17, 18, 19, 20, 31, 63, 93, 94, 95, 96, 97];
/// HDMI-A-2's monitor is not known (213-DRV-0112): with no EDID, the mode first light was shown in, CTA VIC 16.
pub const HDMI_A_2_VICS: [u8; 1] = [16];

/// An output's mode when its EDID gives none.
pub fn fallback(o: &Output) -> timing::Timing {
    timing::first_light_mode(if *o == light::HDMI_A_1 { &HDMI_A_1_VICS[..] } else { &HDMI_A_2_VICS[..] })
}

/// What display starts on: the engine's window, read only, and the same window with its writes.
pub trait Device {
    /// Step 1's window: reads only.
    type Window: Bus;
    /// Step 2's I2C engine and steps 3 and 4: reads, guarded writes, time, slot 5's surface and the compositor.
    type Writer: light::Engine;
    /// The engine's window mapped; called only when the plan reads a batch.
    fn map(&mut self) -> Option<Self::Window>;
    /// The writer over the mapped window; made only when the plan writes.
    fn writer(&mut self, window: Self::Window) -> Self::Writer;
    fn now_us(&mut self) -> u64;
    /// A line of display's own, straight to the log.
    fn say(&mut self, args: fmt::Arguments);
}

/// Steps 3 and 4's run as it stands, for an unplug's taking back (213-DRV-0096).
pub struct Run<'a> { pub light: Box<light::Light>, pub interp: Box<atom::Interpreter<'a>>, pub journal: Box<light::Journal>, pub outcome: light::Outcome, pub times: light::Times }

/// What the start did: the registers step 1 read, the writes steps 3 and 4 left in their journal (the second run's with
/// them), the writer if one was made, the run, and the second HDMI run beside it when its stages ran (213-DRV-0113).
pub struct Started<'a, W> { pub read: usize, pub written: usize, pub writer: Option<W>, pub run: Option<Run<'a>>, pub second: Option<Run<'a>> }

pub fn order_name(order: Order) -> &'static str { if order == Order::Survey { "SURVEY'S" } else { "PRODUCTION" } }

/// Steps 1 to 4 as `plan` asks, on the device with PCI identity `id` and an engine's window of `engine` bytes, from
/// `started` (display's start on the device's clock).
pub fn start<'a, D: Device>(d: &mut D, plan: &Plan, id: u32, engine: usize, vfct: Option<&'a [u8]>, build: &'static str, started: u64, out: &mut dyn Out) -> Started<'a, D::Writer> {
    let mut store = Store::default();
    let t1 = d.now_us();
    let (read, window) = step1(d, plan, id, engine, &mut store, out);
    let t2 = d.now_us();
    // Only here is a writer made: a plan that writes nothing has none, and step 3 only describes its stages.
    let mut writer = window.filter(|_| plan.writes()).map(|w| d.writer(w));
    let output = choose(d, plan, &store);
    let edid = step2(d, plan, output, &store, writer.as_mut(), out);
    // The other socket's monitor, not taken: its EDID read all the same, before any stage runs.
    let other = light::other(&output).filter(|o| sensed(&store, o));
    let other_edid = match (other, writer.as_mut()) { (Some(o), Some(w)) => untaken_edid(plan, o, &store, w, out), _ => None };
    let t3 = d.now_us();
    let mut run = step3(d, plan, output, &store, writer.as_mut(), vfct, edid.as_ref(), build, out);
    let (written, times) = (run.as_ref().map_or(0, |r| r.journal.len()), run.as_ref().map(|r| r.times));
    // How long each part took, from display's start (213-DRV-0098): to the offer when stage 16 made it.
    let ms = |us: u64| (us / 1000, us % 1000 / 100);
    let (a, b, c) = (ms(t1.saturating_sub(started)), ms(t2.saturating_sub(t1)), ms(t3.saturating_sub(t2)));
    let name = order_name(plan.order);
    match times.and_then(|t| t.offered_us.map(|o| t.start_us + o)) {
        Some(offered) => {
            let (e, f) = (ms(offered.saturating_sub(started)), ms(offered.saturating_sub(t3)));
            d.say(format_args!("THE SECOND SCREEN OFFERED {}.{} MS AFTER DISPLAY'S START, IN THE {} ORDER: BEFORE STEP 1 {}.{} MS, STEP 1 {}.{} MS, STEP 2 {}.{} MS, STEP 3 TO THE OFFER {}.{} MS",
                e.0, e.1, name, a.0, a.1, b.0, b.1, c.0, c.1, f.0, f.1));
        }
        None => { let e = ms(d.now_us().saturating_sub(t3)); d.say(format_args!("NO SECOND SCREEN OFFERED, IN THE {} ORDER: BEFORE STEP 1 {}.{} MS, STEP 1 {}.{} MS, STEP 2 {}.{} MS, STEP 3 {}.{} MS", name, a.0, a.1, b.0, b.1, c.0, c.1, e.0, e.1)) }
    }
    match (read, written) {
        (0, 0) => d.say(format_args!("NO REGISTER READ OR WRITTEN")),
        (r, 0) => d.say(format_args!("{} REGISTERS READ, NONE WRITTEN", r)),
        (r, w) => d.say(format_args!("{} REGISTERS READ BY STEP 1, {} WRITES BY STEP 3", r, w)),
    }
    // The other socket's monitor beside the first lit (213-DRV-0113), after the first's lines, as SECOND_HDMI asks.
    let second = match (other, writer.as_mut()) {
        (Some(o), Some(w)) => second_run(d, plan, run.as_mut(), o, other_edid.as_ref(), w, vfct, build, out),
        (None, _) if plan.second_hdmi != setting::SecondHdmi::Off => {
            d.say(format_args!("SECOND HDMI RUN NOT MADE (SECOND_HDMI {}): NO MONITOR SENSED IN THE OTHER HDMI SOCKET", plan.second_hdmi.word()));
            None
        }
        _ => None,
    };
    let written = written + second.as_ref().map_or(0, |r| r.journal.len());
    Started { read, written, writer, run, second }
}

/// Step 1 on the engine's window: the registers it read, kept in `store`, and the window when it mapped one.
fn step1<D: Device>(d: &mut D, plan: &Plan, id: u32, engine: usize, store: &mut Store, out: &mut dyn Out) -> (usize, Option<D::Window>) {
    d.say(format_args!("STEP 1 MAY READ {} OF {} BATCHES, WRITES NOTHING; THE {} ORDER", plan.batches, batches::BATCHES, order_name(plan.order)));
    if id != RADEON { d.say(format_args!("NOT THE RADEON 1002:1638: STEP 1 NOT RUN")); return (0, None); }
    if engine < dcn::DCN_BYTES.end as usize { d.say(format_args!("THE ENGINE'S WINDOW IS BELOW {:#X} BYTES: STEP 1 NOT RUN", dcn::DCN_BYTES.end)); return (0, None); }
    if plan.order == Order::Survey { out.pause(START_PAUSE_MS); }
    if plan.batches == 0 {
        let read = batches::read_into(&mut Unmapped, 0, plan.order, out, store).registers_read;
        batches::read_second(&mut Unmapped, false, out, store);
        return (read, None);
    }
    let Some(mut window) = d.map() else { d.say(format_args!("THE ENGINE'S WINDOW NOT MAPPED: STEP 1 NOT RUN")); return (0, None) };
    let read = read_all(&mut window, plan, out, store);
    (read, Some(window))
}

/// Step 1's batches as the plan asks, then batch 12, HDMI-A-2's (213-DRV-0112), when all eleven are read: the registers read.
fn read_all(bus: &mut impl Bus, plan: &Plan, out: &mut dyn Out, store: &mut Store) -> usize {
    let read = batches::read_into(bus, plan.batches, plan.order, out, store).registers_read;
    read + batches::read_second(bus, plan.batches >= batches::BATCHES, out, store).registers_read
}

/// The socket steps 2 to 4 take (213-DRV-0112): the one whose HPD senses a monitor. When both do, HDMI-A-1, or HDMI-A-2 in
/// a build that takes it first (`HdmiFirst`) where the plan takes it; the other is named as not driven (display lights
/// one socket at a time). HDMI-A-2 only where the plan takes it; HDMI-A-1 when none does, as before.
fn choose<D: Device>(d: &mut D, plan: &Plan, s: &Store) -> Output {
    let second_first = plan.hdmi_first == setting::HdmiFirst::HdmiA2;
    match (sensed(s, &light::HDMI_A_1), sensed(s, &light::HDMI_A_2)) {
        (true, true) if second_first && plan.takes(&light::HDMI_A_2) => {
            if plan.second_hdmi == setting::SecondHdmi::Off {
                d.say(format_args!("HDMI-A-1 NOT DRIVEN: HPD 1 SENSES A MONITOR THERE TOO; HDMI-A-2 IS TAKEN FIRST (HDMI_FIRST HDMI-A-2), AND DISPLAY LIGHTS ONE HDMI SOCKET AT A TIME FOR NOW"));
            } else { taken_second(d, plan, &light::HDMI_A_1, &light::HDMI_A_2); }
            d.say(format_args!("HDMI-A-2 TAKEN: HPD 2 SENSES A MONITOR, TAKEN FIRST; HDMI_A_2 {}: {}", plan.hdmi_a_2.word(), plan.hdmi_a_2.what()));
            light::HDMI_A_2
        }
        (true, true) if second_first => { not_driven(d, plan); light::HDMI_A_1 }
        (true, true) if plan.second_hdmi != setting::SecondHdmi::Off => { taken_second(d, plan, &light::HDMI_A_2, &light::HDMI_A_1); light::HDMI_A_1 }
        (true, true) => { d.say(format_args!("HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE TOO; HDMI-A-1 IS TAKEN FIRST, AND DISPLAY LIGHTS ONE HDMI SOCKET AT A TIME FOR NOW")); light::HDMI_A_1 }
        (false, true) if plan.takes(&light::HDMI_A_2) => {
            d.say(format_args!("HDMI-A-2 TAKEN: HPD 2 SENSES A MONITOR, HPD 1 NONE; HDMI_A_2 {}: {}", plan.hdmi_a_2.word(), plan.hdmi_a_2.what()));
            light::HDMI_A_2
        }
        (false, true) => { not_driven(d, plan); light::HDMI_A_1 }
        _ => light::HDMI_A_1,
    }
}

/// Both sockets with a monitor and a build that runs a second HDMI run (213-DRV-0113): `o` is taken after `first`'s stages.
fn taken_second<D: Device>(d: &mut D, plan: &Plan, o: &Output, first: &Output) {
    d.say(format_args!("{} TAKEN SECOND, AFTER {}'S STAGES: HPD {} SENSES A MONITOR THERE TOO (SECOND_HDMI {}: {})", light::name(o), light::name(first), o.hpd + 1,
                       plan.second_hdmi.word(), plan.second_hdmi.what()));
}

/// Whether step 1 found output `o`'s HPD sensing a monitor.
fn sensed(s: &Store, o: &Output) -> bool { s.field(R::HpdIntStatus, o.hpd, DC_HPD_SENSE) == Some(1) }

/// The EDID of a socket with a monitor that steps 3 and 4 do not take (213-DRV-0112), where the plan reads its EDID: over
/// its DDC line as step 2 reads it, on the engine as `store` found it, given back as found, each line named for the socket.
/// Only the I2C engine's own registers and that line's speed, set-up and pads are written (step 2's guard); nothing a lit
/// output reads, nor the other line's. What it says, for a second HDMI run's mode (213-DRV-0113).
fn untaken_edid(plan: &Plan, o: Output, store: &Store, w: &mut impl light::Engine, out: &mut dyn Out) -> Option<edid::Edid> {
    let line = ddc::line_of(&o).filter(|_| plan.edid_on(&o))?;
    out.note(format_args!("{} NOT TAKEN: ITS EDID READ OVER {} ALL THE SAME, THE I2C ENGINE GIVEN BACK AS FOUND; NOTHING ELSE OF IT WRITTEN", light::name(&o), line.name));
    let mut named = Naming { inner: out, name: light::name(&o) };
    let r = ddc::read_on(line, store, w, &mut named).ok()?;
    ddc::describe(&r, &mut named)
}

/// The I2C engine and the socket's HPD and DDC line as they are now, read only: batch 1, batch 11 and, for HDMI-A-2,
/// batch 12; for an EDID read of a socket not taken after start.
fn engine_now(w: &mut impl Bus, plan: &Plan, o: &Output, out: &mut dyn Out) -> Store {
    let mut store = Store::default();
    batches::read_one(w, 1, plan.order, out, &mut store);
    batches::read_one(w, 11, plan.order, out, &mut store);
    if *o == light::HDMI_A_2 { batches::read_second(w, true, out, &mut store); }
    store
}

/// A log whose every line names a socket: the EDID read of one not taken.
struct Naming<'a> { inner: &'a mut dyn Out, name: &'static str }
impl Out for Naming<'_> {
    fn line(&mut self, args: fmt::Arguments) { self.inner.line(format_args!("{}: {}", self.name, args)) }
    fn pause(&mut self, ms: u32) { self.inner.pause(ms) }
    fn note(&mut self, args: fmt::Arguments) { self.inner.note(format_args!("{}: {}", self.name, args)) }
    fn part(&mut self) { self.inner.part() }
    fn failed(&mut self) { self.inner.failed() }
}

/// HDMI-A-2's monitor not driven because the build goes no further there than reading its blocks.
fn not_driven<D: Device>(d: &mut D, plan: &Plan) {
    d.say(format_args!("HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE; THIS BUILD GOES NO FURTHER THAN {} (HDMI_A_2 {})", plan.hdmi_a_2.what(), plan.hdmi_a_2.word()));
}

/// Step 2 (213-DRV-0095): the EDID over the output's DDC line, on the writer, and what it says.
fn step2<D: Device>(d: &mut D, plan: &Plan, output: Output, store: &Store, writer: Option<&mut D::Writer>, out: &mut dyn Out) -> Option<edid::Edid> {
    let Some(line) = ddc::line_of(&output) else { d.say(format_args!("STEP 2 NOT RUN: {} HAS NO DDC LINE HERE", light::name(&output))); return None };
    if !plan.edid_on(&output) { d.say(format_args!("STEP 2 (THE EDID OVER {}) NOT RUN: THE SETTING DOES NOT ASK FOR IT", line.name)); return None; }
    let Some(w) = writer else { d.say(format_args!("STEP 2 NOT RUN: STEP 1 DID NOT MAP THE ENGINE'S WINDOW")); return None };
    let read = ddc::read_on(line, store, w, out).ok()?;
    ddc::describe(&read, out)
}

/// Steps 3 and 4 on `output`: their run, when their stages ran. With no stage in the plan they only describe the stages,
/// for the pipe step 1 found or, with step 1 not read, for the survey's expectation.
fn step3<'a, D: Device>(d: &mut D, plan: &Plan, output: Output, store: &Store, writer: Option<&mut D::Writer>, vfct: Option<&'a [u8]>, edid: Option<&edid::Edid>, build: &'static str, out: &mut dyn Out) -> Option<Run<'a>> {
    let (timing, signal, why) = light::mode_for(edid, fallback(&output));
    if plan.edid_on(&output) { d.say(format_args!("STEP 3'S MODE: {}x{} AT {} KHZ IN {:?} SIGNALLING, {}", timing.h_active, timing.v_active, timing.pixel_khz, signal, why)); }
    let stages = plan.stages_on(&output);
    let plan3 = match light::prepare_on(store, output, timing, signal, build) {
        Ok(p) => p.with_watch(plan.watch_frames),
        Err(why) if stages == 0 => {
            d.say(format_args!("STEP 3 SHOWN FOR THE SURVEY'S EXPECTATION: {}", why));
            let (f, c) = light::expected(output);
            if let Ok(p) = light::Light::new_with(f, c, timing, signal) { light::describe(&p.with_build(build).with_watch(plan.watch_frames), true, out); }
            return None;
        }
        Err(why) => { d.say(format_args!("STEP 3 NOT RUN: {}", why)); return None; }
    };
    if stages == 0 { light::describe(&plan3, false, out); return None; }
    let Some(writer) = writer else { d.say(format_args!("STEP 3 NOT RUN: STEP 1 DID NOT MAP THE ENGINE'S WINDOW")); return None };
    let Some((0x1002, 0x1638, image)) = vfct.and_then(atom::vfct_image) else { d.say(format_args!("STEP 3 NOT RUN: NO VBIOS IMAGE FOR 1002:1638")); return None };
    let Ok(image) = atom::Image::new(image) else { d.say(format_args!("STEP 3 NOT RUN: THE VBIOS IMAGE HAS NO ATOM ROM HEADER")); return None };
    let mut interp = Box::new(atom::Interpreter::new(image));
    let mut journal = Box::new(light::Journal::default());
    let (outcome, times) = light::run_timed(&plan3, writer, &mut interp, &mut journal, stages, plan.undo_after_stages, out);
    Some(Run { light: Box::new(plan3), interp, journal, outcome, times })
}

/// The stages a run left in place.
fn done(r: &Run) -> u8 { match r.outcome { light::Outcome::Stopped { done } | light::Outcome::NotReady { done, .. } => done, _ => 0 } }

/// The second HDMI run (213-DRV-0113): on `o`, the socket with a monitor not taken first, beside `first`, display's lit run,
/// as SECOND_HDMI asks; nothing at Off. Step 1 read again with the first's timing generator counted out of the running,
/// the pipe it takes read as the firmware left it (batches 13 and 14), then first light's stages there with the firmware's
/// pipe and the first's read after each, or their description; at `Full` all sixteen, its picture in the screen the first
/// does not hold (the third: slot 7's pixels offered through slot 8), and after first light only when that screen's slots
/// are granted. The first's guard refuses the second's blocks from its stages on, and reads its pipe while they stay. The
/// run when its stages ran.
fn second_run<'a, D: Device>(d: &mut D, plan: &Plan, first: Option<&mut Run<'a>>, o: Output, edid: Option<&edid::Edid>, w: &mut D::Writer,
                             vfct: Option<&'a [u8]>, build: &'static str, out: &mut dyn Out) -> Option<Run<'a>> {
    let s = plan.second_hdmi;
    if s == setting::SecondHdmi::Off { return None; }
    let name = light::name(&o);
    let Some(first) = first.filter(|r| done(r) >= dcn::FIRST_LIGHT) else {
        d.say(format_args!("{}: SECOND HDMI RUN NOT MADE (SECOND_HDMI {}): THE FIRST SOCKET'S FIRST LIGHT IS NOT IN PLACE", name, s.word()));
        return None;
    };
    let lit = first.light.choice;
    d.say(format_args!("{}: SECOND HDMI RUN BESIDE {} ON OTG{} (SECOND_HDMI {}): {}", name, light::name(&lit.output), lit.pipe.otg, s.word(), s.what()));
    let start = d.now_us();
    let mut named = Naming { inner: out, name };
    let mut store = Store::default();
    let mut ours = [false; 4];
    ours[lit.pipe.otg as usize & 3] = true;
    let mut read = batches::read_into_beside(w, plan.batches, plan.order, &mut named, &mut store, ours).registers_read;
    if o == light::HDMI_A_2 { read += batches::read_second(w, plan.batches >= batches::BATCHES, &mut named, &mut store).registers_read; }
    let refused = |d: &mut D, why: &dyn fmt::Display| d.say(format_args!("{}: SECOND HDMI RUN NOT MADE: {}", name, why));
    let next = match light::assess_beside(&store, o, &first.light.firmware, &lit) { Ok((_, c)) => c.pipe.otg, Err(why) => { refused(d, &why); return None; } };
    read += batches::read_pipe(w, next, &mut named, &mut store).registers_read;
    let (timing, signal, why) = light::mode_for(edid, fallback(&o));
    if plan.edid_on(&o) {
        d.say(format_args!("{}: STEP 3'S MODE: {}x{} AT {} KHZ IN {:?} SIGNALLING, {}", name, timing.h_active, timing.v_active, timing.pixel_khz, signal, why));
    }
    let screen = first.light.screen.other();
    let second = match light::prepare_beside(&store, o, &first.light, timing, signal, build) {
        Ok(l) => l.with_watch(plan.watch_frames).with_screen(screen),
        Err(why) => { refused(d, &why); return None; }
    };
    let mut stages = plan.second_stages(&o);
    // Past first light only with its screen's pixels and client granted: the third's are slots 7 and 8 (213-KRN-0285).
    if stages > dcn::FIRST_LIGHT && screen == light::Screen::Third {
        let (pixels, client) = light::Screens::third_granted(w);
        if !(pixels && client) {
            let word = |g: bool| if g { "GRANTED" } else { "NOT GRANTED" };
            d.say(format_args!("{}: THE THIRD SCREEN'S SLOTS: SLOT 7 (ITS PIXELS) {}, SLOT 8 (ITS CLIENT) {}: THE SECOND HDMI RUN STOPS AFTER FIRST LIGHT, ITS PATTERN LEFT",
                               name, word(pixels), word(client)));
            stages = dcn::FIRST_LIGHT;
        }
    }
    if stages == 0 {
        light::describe(&second, false, &mut named);
        d.say(format_args!("{}: SECOND HDMI RUN SHOWN FOR OTG{}, {} REGISTERS READ, NOTHING WRITTEN (SECOND_HDMI {}, HDMI_A_2 {})", name, next, read, s.word(), plan.hdmi_a_2.word()));
        return None;
    }
    let Some((0x1002, 0x1638, image)) = vfct.and_then(atom::vfct_image) else { refused(d, &"NO VBIOS IMAGE FOR 1002:1638"); return None };
    let Ok(image) = atom::Image::new(image) else { refused(d, &"THE VBIOS IMAGE HAS NO ATOM ROM HEADER"); return None };
    if let Err(why) = first.light.refuse(&second.choice) { refused(d, &why); return None; }
    let mut interp = Box::new(atom::Interpreter::new(image));
    let mut journal = Box::new(light::Journal::default());
    let (outcome, times) = light::run_timed(&second, w, &mut interp, &mut journal, stages, plan.second_undone(), &mut named);
    let run = Run { light: Box::new(second), interp, journal, outcome, times };
    let state = hpd::state_after(Some(outcome));
    // Its stages in place: the first's undo and take-back read its pipe as the second's stages read the first's.
    if state == hpd::State::Lit {
        if let Err(why) = first.light.watch_beside(&run.light.choice, &mut first.journal, w) { d.say(format_args!("{}: ITS PIPE NOT READ BESIDE {}: {}", name, light::name(&lit.output), why)); }
    }
    let ms = d.now_us().saturating_sub(start) / 1000;
    let offered = if run.times.offered_us.is_some() { if screen == light::Screen::Third { "; OFFERED AS THE THIRD SCREEN THROUGH SLOT 8" } else { "; OFFERED AS THE SECOND SCREEN" } } else { "" };
    d.say(format_args!("{}: SECOND HDMI RUN ON OTG{} {:?}: {} STAGES IN PLACE, {} REGISTERS READ, {} WRITES, {} MS; {} {:?} ON OTG{}{}", name, next, state, done(&run), read,
                       run.journal.len(), ms, light::name(&lit.output), hpd::state_after(Some(first.outcome)), lit.pipe.otg, offered));
    Some(run)
}

/// After steps 1 to 4: both HDMI sockets' HPD followed when the plan asks (213-DRV-0096, 213-DRV-0112), read every
/// `hpd::POLL_MS`, and the standing check (213-DRV-0103), once a second while stage 15's picture is up. An unplug of the
/// lit socket that holds takes its stages back (screen_gone first), and lights the other socket if its monitor is in; a
/// monitor plugged in that holds, while none is lit, runs steps 1 to 4 again on its socket on the same writer, step 1 and
/// the EDID read anew; one plugged into the socket not lit is named, not driven, or with SECOND_HDMI gets the second HDMI
/// run beside the lit one (213-DRV-0113), whose unplug takes back its own stages (screen_gone through its own client first),
/// the first's left lit. The first's unplug with the second lit leaves the second the only one, on its pipe and screen,
/// followed as the first was; a monitor plugged in again then gets the pipe and screen left free. Each run's picture has
/// its standing check, which reads the other display pipe too: a new underflow or request timeout undoes the run on whose
/// pipe it is (the firmware's: the run whose check found it), as the watch does; after a failure nothing more is tried.
pub struct Follower<'a> {
    run: Option<Run<'a>>, state: hpd::State, f: Option<hpd::Pair>, account: light::Standing, next: u64, standing: bool,
    /// Each socket's HPD sense, when it is followed.
    senses: [Option<Reg>; 2],
    /// The second HDMI run beside the first, while its stages are in place (213-DRV-0113), and its standing check's account.
    second: Option<Run<'a>>, account2: light::Standing,
}

/// The display pipe a standing check's failure is on, if it names one (213-DRV-0113).
fn pipe_hit(why: &light::Failure) -> Option<u8> {
    match why { light::Failure::Underflow { offset, .. } | light::Failure::RequestTimeout { offset, .. } => light::status_pipe(*offset).map(|(_, n)| n), _ => None }
}

/// A socket's index in `light::SOCKETS`.
fn socket(o: &Output) -> usize { light::SOCKETS.iter().position(|s| s == o).unwrap_or(0) }

impl<'a> Follower<'a> {
    /// What follows the stages, when the plan asks for it and does not undo them; None, and display idles.
    pub fn new<D: Device>(d: &mut D, w: &mut D::Writer, run: Option<Run<'a>>, plan: &Plan) -> Option<Follower<'a>> {
        if !(plan.hotplug || plan.standing_check) || plan.undo_after_stages { return None; }
        let state = hpd::state_after(run.as_ref().map(|r| r.outcome));
        let on = run.as_ref().map_or(0, |r| socket(&r.light.choice.output));
        let (mut senses, mut now) = ([None; 2], [None; 2]);
        if plan.hotplug {
            for (i, o) in light::SOCKETS.iter().enumerate() {
                let n = o.hpd + 1;
                let (Some(sense), Some(control)) = (R::HpdIntStatus.at(o.hpd), R::HpdControl.at(o.hpd)) else { continue };
                if w.read(control).map(|v| DC_HPD_EN.get(v)) != Some(1) { d.say(format_args!("HPD {} IS NOT ENABLED: ITS SENSE IS NOT FOLLOWED", n)); continue; }
                let Some(v) = w.read(sense).map(|v| DC_HPD_SENSE.get(v) == 1) else { d.say(format_args!("HPD {} CANNOT BE READ: NOT FOLLOWED", n)); continue };
                (senses[i], now[i]) = (Some(sense), Some(v));
                if i == 0 {
                    d.say(format_args!("HPD 1 FOLLOWED EVERY {} MS, A CHANGE COUNTED AFTER {} MS: SENSE {}, THE SECOND OUTPUT {:?}", hpd::POLL_MS,
                                       hpd::SETTLE_US / 1000, v as u8, state));
                } else {
                    d.say(format_args!("HPD 2 FOLLOWED TOO, FOR HDMI-A-2 (HDMI_A_2 {}): SENSE {}; THE SECOND OUTPUT {:?} ON {}", plan.hdmi_a_2.word(), v as u8,
                                       state, light::name(&light::SOCKETS[on])));
                }
            }
        }
        let f = senses.iter().any(|s| s.is_some()).then(|| hpd::Pair::new(state, on, now));
        if f.is_none() && !plan.standing_check { return None; }
        if plan.standing_check {
            d.say(format_args!("THE STANDING CHECK: BOTH PIPES' UNDERFLOW STATUSES READ EVERY {} MS WHILE THE SECOND PICTURE IS UP; THE WATCH {} FRAMES",
                               light::STANDING_US / 1000, run.as_ref().map_or(light::WATCH_FRAMES, |r| r.light.watch_frames)));
        }
        let account = light::Standing::new(w.now_us());
        let next = w.now_us() + light::STANDING_US;
        Some(Follower { run, state, f, account, next, standing: plan.standing_check, senses, second: None, account2: account })
    }

    /// With the start's second HDMI run (213-DRV-0113): its socket's unplug followed while its stages are in place; after a
    /// failure or the build's undo it is not tried again.
    pub fn with_second(mut self, second: Option<Run<'a>>) -> Follower<'a> {
        let state = second.as_ref().map(|r| hpd::state_after(Some(r.outcome)));
        if let Some(f) = self.f.as_mut() { f.set_second(state); }
        self.second = second.filter(|_| state == Some(hpd::State::Lit));
        self
    }
    /// The second HDMI run's state, while its socket is followed.
    pub fn second_state(&self) -> Option<hpd::State> { self.f.as_ref().and_then(|f| f.second()) }
    /// The output the second HDMI run is lit on, if any.
    pub fn second_output(&self) -> Option<Output> { self.second.as_ref().map(|r| r.light.choice.output) }
    /// The pipe (OTG) and screen each run holds (213-DRV-0113).
    pub fn held(&self) -> Option<(u8, light::Screen)> { self.run.as_ref().map(|r| (r.light.choice.pipe.otg, r.light.screen)) }
    pub fn second_held(&self) -> Option<(u8, light::Screen)> { self.second.as_ref().map(|r| (r.light.choice.pipe.otg, r.light.screen)) }

    /// How long display sleeps between two polls.
    pub fn sleep_ms(&self) -> usize { if self.f.is_some() { hpd::POLL_MS } else { (light::STANDING_US / 1000) as usize } }
    pub fn state(&self) -> hpd::State { self.state }
    /// The output steps 3 and 4 last ran on (lit while the state is), if any.
    pub fn output(&self) -> Option<Output> { self.run.as_ref().map(|r| r.light.choice.output) }

    /// The first run's picture undone after its standing check (213-DRV-0103): the mark kept, nothing tried again on it.
    fn undo_first<E: light::Engine>(&mut self, w: &mut E, why: light::Failure, out: &mut dyn Out) {
        let Some(r) = self.run.as_mut() else { return };
        let since = r.times.offered_us.map_or(0, |o| w.now_us().saturating_sub(r.times.start_us + o));
        let stages = done(r);
        light::standing_undo(&r.light, w, &mut r.interp, &mut r.journal, stages, why, since, out);
        r.outcome = light::Outcome::Failed { stage: 15, why };
        self.state = hpd::State::Stopped;
        if let Some(s) = self.second.as_mut() { s.light.alone(&mut s.journal); }
        if let Some(f) = self.f.as_mut() { let on = f.on(); f.set(self.state, on); }
    }
    /// The second HDMI run's picture undone after a standing check (213-DRV-0113): its screen let go through its own
    /// client first, its mark kept; the first's lit pipe no longer reads it.
    fn undo_second<E: light::Engine>(&mut self, w: &mut E, why: light::Failure, out: &mut dyn Out) {
        let Some(mut r) = self.second.take() else { return };
        let since = r.times.offered_us.map_or(0, |o| w.now_us().saturating_sub(r.times.start_us + o));
        let stages = done(&r);
        light::standing_undo(&r.light, w, &mut r.interp, &mut r.journal, stages, why, since, &mut Naming { inner: out, name: light::name(&r.light.choice.output) });
        if let Some(first) = self.run.as_mut() { first.light.alone(&mut first.journal); }
        if let Some(f) = self.f.as_mut() { f.set_second(Some(hpd::State::Stopped)); }
    }
    /// A standing check's failure named with its pipe and the run it undoes, in a build with a second HDMI run.
    fn say_hit<D: Device>(&self, d: &mut D, plan: &Plan, why: &light::Failure, second: bool) {
        if plan.second_hdmi == setting::SecondHdmi::Off { return; }
        let run = if second { self.second.as_ref() } else { self.run.as_ref() };
        let Some(r) = run else { return };
        let (unit, n) = match why { light::Failure::Underflow { offset, .. } | light::Failure::RequestTimeout { offset, .. } => light::status_pipe(*offset).unwrap_or(("?", 9)), _ => ("?", 9) };
        let whose = if n == r.light.firmware.otg { "THE FIRMWARE'S" } else if n == r.light.choice.pipe.otg { light::name(&r.light.choice.output) } else { "THE OTHER LIT PIPE'S" };
        d.say(format_args!("THE STANDING CHECK: {} ON PIPE {}'S {} ({}): {}'S RUN ON OTG{} UNDONE", why, n, unit, whose, light::name(&r.light.choice.output), r.light.choice.pipe.otg));
    }
    /// Both sockets as they are now, after each change, in a build with a second HDMI run (213-DRV-0113).
    fn say_sockets<D: Device>(&self, d: &mut D, plan: &Plan) {
        if plan.second_hdmi == setting::SecondHdmi::Off { return; }
        let lit = |r: &Run| {
            let screen = if r.times.offered_us.is_some() { r.light.screen.name() } else { "NO" };
            (r.light.choice.pipe.otg, done(r), screen)
        };
        let mut said: [(&str, Option<(u8, u8, &str)>); 2] = [("", None); 2];
        for i in 0..light::SOCKETS.len() {
            let first = self.run.as_ref().filter(|r| socket(&r.light.choice.output) == i);
            let second = self.second.as_ref().filter(|r| socket(&r.light.choice.output) == i);
            said[i] = match (first, second) {
                (Some(r), _) if self.state == hpd::State::Lit => ("LIT", Some(lit(r))),
                (Some(_), _) if self.state == hpd::State::Stopped => ("STOPPED AFTER A FAILURE", None),
                (_, Some(r)) => ("LIT", Some(lit(r))),
                _ => (match self.f.as_ref().and_then(|f| f.sense(i)) { Some(true) => "A MONITOR, NOT LIT", Some(false) => "NO MONITOR", None => "NOT FOLLOWED" }, None),
            };
        }
        let part = |i: usize| -> Sockets { Sockets(light::name(&light::SOCKETS[i]), said[i].0, said[i].1) };
        d.say(format_args!("HDMI SOCKETS NOW: {}; {}", part(0), part(1)));
    }

    /// One poll after the sleep: the standing check when it is due, then both HPD senses.
    pub fn poll<D: Device>(&mut self, d: &mut D, w: &mut D::Writer, plan: &Plan, vfct: Option<&'a [u8]>, build: &'static str, out: &mut dyn Out) {
        // The standing checks, once a second, of each run whose picture is up (stage 15 in place), the first's then the second's.
        if self.standing && w.now_us() >= self.next {
            self.next = w.now_us() + light::STANDING_US;
            let first_up = self.state == hpd::State::Lit && self.run.as_ref().is_some_and(|r| done(r) >= 15);
            let found = match self.run.as_mut() { Some(r) if first_up => light::standing_check(&r.light, w, &r.journal, &mut self.account, out).err(), _ => None };
            if let Some(why) = found {
                let on_second = self.second.as_ref().is_some_and(|s| pipe_hit(&why) == Some(s.light.choice.pipe.otg));
                self.say_hit(d, plan, &why, on_second);
                if on_second { self.undo_second(w, why, out); } else { self.undo_first(w, why, out); }
                self.say_sockets(d, plan);
            }
            let second_up = self.second.as_ref().is_some_and(|r| done(r) >= 15);
            let found = match self.second.as_mut() {
                Some(r) if second_up => { let name = light::name(&r.light.choice.output); light::standing_check(&r.light, w, &r.journal, &mut self.account2, &mut Naming { inner: out, name }).err() }
                _ => None,
            };
            if let Some(why) = found {
                let on_first = self.run.as_ref().is_some_and(|r| self.state == hpd::State::Lit && pipe_hit(&why) == Some(r.light.choice.pipe.otg));
                self.say_hit(d, plan, &why, !on_first);
                if on_first { self.undo_first(w, why, out); } else { self.undo_second(w, why, out); }
                self.say_sockets(d, plan);
            }
        }
        let Some(pair) = self.f.as_mut() else { return };
        let senses = self.senses.map(|r| r.and_then(|r| w.read(r)).map(|v| DC_HPD_SENSE.get(v) == 1));
        match pair.feed(senses, w.now_us()) {
            // The second HDMI run's socket: its own stages taken back, the first left lit (213-DRV-0113).
            hpd::Event::TakeBack(i) if self.second.as_ref().map(|r| socket(&r.light.choice.output)) == Some(i) => {
                let Some(mut r) = self.second.take() else { return };
                let stages = done(&r);
                let unchanged = light::take_back(&r.light, w, &mut r.interp, &mut r.journal, stages, &mut Naming { inner: out, name: light::name(&light::SOCKETS[i]) });
                if let Some(first) = self.run.as_mut() { first.light.alone(&mut first.journal); }
                self.account2.dark(w.now_us());
                pair.set_second(Some(if unchanged { hpd::State::Dark } else { hpd::State::Stopped }));
                self.say_sockets(d, plan);
            }
            hpd::Event::TakeBack(i) => {
                let Some(r) = self.run.as_mut() else { pair.set(hpd::State::Dark, i); self.state = hpd::State::Dark; return };
                let stages = done(r);
                let unchanged = light::take_back(&r.light, w, &mut r.interp, &mut r.journal, stages, out);
                self.account.dark(w.now_us()); // the check's account: no time between two spans the dark
                r.outcome = light::Outcome::Stopped { done: 0 };
                self.state = if unchanged { hpd::State::Dark } else { hpd::State::Stopped };
                pair.set(self.state, i);
                // The second HDMI run beside it now the only one lit, followed as the first was (213-DRV-0113).
                if let (true, Some(mut s)) = (unchanged, self.second.take()) {
                    s.light.alone(&mut s.journal);
                    let (o, otg) = (s.light.choice.output, s.light.choice.pipe.otg);
                    self.state = hpd::state_after(Some(s.outcome));
                    pair.set(self.state, socket(&o));
                    pair.set_second(None);
                    self.run = Some(s);
                    self.account = self.account2;
                    self.next = w.now_us() + light::STANDING_US;
                    d.say(format_args!("{} NOW THE ONLY HDMI SOCKET LIT, ON OTG{}: FOLLOWED AS THE FIRST WAS; THE SECOND OUTPUT {:?}", light::name(&o), otg, self.state));
                }
                self.say_sockets(d, plan);
            }
            hpd::Event::LightUp(i) => {
                let output = light::SOCKETS[i];
                // Not taken: named, its blocks read again with its monitor in; the output stays dark.
                if !plan.takes(&output) {
                    not_driven(d, plan);
                    batches::read_second(w, true, out, &mut Store::default());
                    return;
                }
                drop(self.run.take()); // the last run's journal and interpreter let go before the next is made
                let mut store = Store::default();
                read_all(w, plan, out, &mut store);
                let edid = match ddc::line_of(&output) {
                    Some(line) if plan.edid_on(&output) => ddc::read_on(line, &store, w, out).ok().and_then(|r| ddc::describe(&r, out)),
                    _ => None,
                };
                self.run = step3(d, plan, output, &store, Some(&mut *w), vfct, edid.as_ref(), build, out);
                self.state = hpd::state_after(self.run.as_ref().map(|r| r.outcome));
                pair.set(self.state, i);
                self.next = w.now_us() + light::STANDING_US;
                if i == 0 { d.say(format_args!("A MONITOR PLUGGED IN: STEPS 1 TO 4 RAN AGAIN, THE SECOND OUTPUT {:?}", self.state)); }
                else { d.say(format_args!("A MONITOR ON {}: STEPS 1 TO 4 RAN AGAIN ON IT, THE SECOND OUTPUT {:?}", light::name(&output), self.state)); }
                self.say_sockets(d, plan);
            }
            // A monitor beside the lit socket with SECOND_HDMI on, and no second run failed or undone: the second run there.
            hpd::Event::Busy(i) if plan.second_hdmi != setting::SecondHdmi::Off && matches!(pair.second(), None | Some(hpd::State::Dark)) => {
                let (o, lit) = (light::SOCKETS[i], light::SOCKETS[pair.on()]);
                d.say(format_args!("{}: A MONITOR PLUGGED IN THERE; {} IS LIT: THE SECOND HDMI RUN NEXT (SECOND_HDMI {})", light::name(&o), light::name(&lit), plan.second_hdmi.word()));
                let store = engine_now(w, plan, &o, out);
                let edid = untaken_edid(plan, o, &store, w, out);
                let second = second_run(d, plan, self.run.as_mut(), o, edid.as_ref(), w, vfct, build, out);
                let state = second.as_ref().map(|r| hpd::state_after(Some(r.outcome)));
                pair.set_second(state);
                self.second = second.filter(|_| state == Some(hpd::State::Lit));
                self.account2 = light::Standing::new(w.now_us());
                self.say_sockets(d, plan);
            }
            hpd::Event::Busy(i) => {
                let (o, lit) = (light::SOCKETS[i], light::SOCKETS[pair.on()]);
                if plan.second_hdmi == setting::SecondHdmi::Off {
                    d.say(format_args!("{} NOT DRIVEN: A MONITOR PLUGGED IN THERE; {} IS LIT, AND DISPLAY LIGHTS ONE HDMI SOCKET AT A TIME FOR NOW", light::name(&o), light::name(&lit)));
                } else {
                    d.say(format_args!("{} NOT DRIVEN: A MONITOR PLUGGED IN THERE; {} IS LIT, AND THE SECOND HDMI RUN FAILED OR WAS UNDONE: NOT TRIED AGAIN THIS BOOT", light::name(&o), light::name(&lit)));
                }
                // Its EDID, where the plan reads it, on the engine as it is now; else its blocks read again (HDMI-A-2's).
                if plan.edid_on(&o) { let store = engine_now(w, plan, &o, out); untaken_edid(plan, o, &store, w, out); }
                else if i == 1 { batches::read_second(w, true, out, &mut Store::default()); }
            }
            // A monitor taken out of a socket not driven: said when it is HDMI-A-2's or the other socket is lit (HDMI-A-1
            // alone stays as quiet as before), and HDMI-A-2's blocks read again.
            hpd::Event::Gone(i) if i == 1 || pair.state() == hpd::State::Lit => {
                d.say(format_args!("{}: ITS MONITOR TAKEN OUT, NOT DRIVEN", light::name(&light::SOCKETS[i])));
                if i == 1 { batches::read_second(w, true, out, &mut Store::default()); }
            }
            hpd::Event::Gone(_) | hpd::Event::None => {}
        }
    }
}

// One socket in the sockets' line: its name, its state, and where a lit one is (OTG, stages, screen).
struct Sockets(&'static str, &'static str, Option<(u8, u8, &'static str)>);
impl fmt::Display for Sockets {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.2 {
            Some((otg, stages, screen)) => write!(f, "{} {} ON OTG{} ({} STAGES, {} SCREEN)", self.0, self.1, otg, stages, screen),
            None => write!(f, "{} {}", self.0, self.1),
        }
    }
}

/// A plan that reads nothing has nothing mapped.
struct Unmapped;
impl Bus for Unmapped { fn read(&mut self, _: Reg) -> Option<u32> { None } }
