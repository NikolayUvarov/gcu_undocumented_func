//! 213-DRV-0099: display's start as the setting's plan asks (`setting::Plan`): step 1 on the engine's window, step 2 and
//! steps 3 and 4 on its writer, what each part took and what was read and written; then what follows the stages, HPD 1
//! (213-DRV-0096) and the standing check (213-DRV-0103). Steps 2 to 4 take the HDMI socket whose HPD senses a monitor,
//! HDMI-A-1 first, and both senses are followed after them; one socket is lit at a time (one spare pipe), HDMI-A-2 as far
//! as the plan lets it (213-DRV-0112). main.rs runs it on the Radeon's windows, the host cases
//! (`tests/display_light_host.rs`) on a model: a plan that reads nothing maps nothing, one that writes nothing makes no
//! writer.

use crate::batches::{self, Bus, Order, Out, Store};
use crate::dcn::{self, field::{DC_HPD_EN, DC_HPD_SENSE}, Output, Reg, R};
use crate::light::Clock;
use crate::setting::Plan;
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

/// What the start did: the registers step 1 read, the writes steps 3 and 4 left in their journal, the writer if one was
/// made, and the run.
pub struct Started<'a, W> { pub read: usize, pub written: usize, pub writer: Option<W>, pub run: Option<Run<'a>> }

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
    let t3 = d.now_us();
    let run = step3(d, plan, output, &store, writer.as_mut(), vfct, edid.as_ref(), build, out);
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
    Started { read, written, writer, run }
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

/// The socket steps 2 to 4 take (213-DRV-0112): the one whose HPD senses a monitor; HDMI-A-1 first when both do, the other
/// named as not driven (one spare pipe); HDMI-A-2 only where the plan takes it; HDMI-A-1 when none does, as before.
fn choose<D: Device>(d: &mut D, plan: &Plan, s: &Store) -> Output {
    let sensed = |o: &Output| s.field(R::HpdIntStatus, o.hpd, DC_HPD_SENSE) == Some(1);
    match (sensed(&light::HDMI_A_1), sensed(&light::HDMI_A_2)) {
        (true, true) => { d.say(format_args!("HDMI-A-2 NOT DRIVEN: HPD 2 SENSES A MONITOR THERE TOO; HDMI-A-1 IS TAKEN FIRST, AND THERE IS ONE SPARE PIPE")); light::HDMI_A_1 }
        (false, true) if plan.takes(&light::HDMI_A_2) => {
            d.say(format_args!("HDMI-A-2 TAKEN: HPD 2 SENSES A MONITOR, HPD 1 NONE; HDMI_A_2 {}: {}", plan.hdmi_a_2.word(), plan.hdmi_a_2.what()));
            light::HDMI_A_2
        }
        (false, true) => { not_driven(d, plan); light::HDMI_A_1 }
        _ => light::HDMI_A_1,
    }
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

/// After steps 1 to 4: both HDMI sockets' HPD followed when the plan asks (213-DRV-0096, 213-DRV-0112), read every
/// `hpd::POLL_MS`, and the standing check (213-DRV-0103), once a second while stage 15's picture is up. An unplug of the
/// lit socket that holds takes its stages back (screen_gone first), and lights the other socket if its monitor is in; a
/// monitor plugged in that holds, while none is lit, runs steps 1 to 4 again on its socket on the same writer, step 1 and
/// the EDID read anew; one plugged into the socket not lit is named, not driven (one spare pipe). A new underflow or
/// request timeout undoes the stages as the watch does; after a failure nothing more is tried.
pub struct Follower<'a> {
    run: Option<Run<'a>>, state: hpd::State, f: Option<hpd::Pair>, account: light::Standing, next: u64, standing: bool,
    /// Each socket's HPD sense, when it is followed.
    senses: [Option<Reg>; 2],
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
        Some(Follower { run, state, f, account, next, standing: plan.standing_check, senses })
    }

    /// How long display sleeps between two polls.
    pub fn sleep_ms(&self) -> usize { if self.f.is_some() { hpd::POLL_MS } else { (light::STANDING_US / 1000) as usize } }
    pub fn state(&self) -> hpd::State { self.state }
    /// The output steps 3 and 4 last ran on (lit while the state is), if any.
    pub fn output(&self) -> Option<Output> { self.run.as_ref().map(|r| r.light.choice.output) }

    /// One poll after the sleep: the standing check when it is due, then both HPD senses.
    pub fn poll<D: Device>(&mut self, d: &mut D, w: &mut D::Writer, plan: &Plan, vfct: Option<&'a [u8]>, build: &'static str, out: &mut dyn Out) {
        // The standing check, once a second, while pipe 1 scans the picture (stage 15 in place).
        if self.standing && self.state == hpd::State::Lit && w.now_us() >= self.next {
            self.next = w.now_us() + light::STANDING_US;
            if let Some(r) = self.run.as_mut().filter(|r| done(r) >= 15) {
                if let Err(why) = light::standing_check(&r.light, w, &r.journal, &mut self.account, out) {
                    let since = r.times.offered_us.map_or(0, |o| w.now_us().saturating_sub(r.times.start_us + o));
                    let stages = done(r);
                    light::standing_undo(&r.light, w, &mut r.interp, &mut r.journal, stages, why, since, out);
                    r.outcome = light::Outcome::Failed { stage: 15, why };
                    self.state = hpd::State::Stopped;
                    if let Some(f) = self.f.as_mut() { let on = f.on(); f.set(self.state, on); }
                }
            }
        }
        let Some(pair) = self.f.as_mut() else { return };
        let senses = self.senses.map(|r| r.and_then(|r| w.read(r)).map(|v| DC_HPD_SENSE.get(v) == 1));
        match pair.feed(senses, w.now_us()) {
            hpd::Event::TakeBack(i) => {
                let Some(r) = self.run.as_mut() else { pair.set(hpd::State::Dark, i); self.state = hpd::State::Dark; return };
                let stages = done(r);
                let unchanged = light::take_back(&r.light, w, &mut r.interp, &mut r.journal, stages, out);
                self.account.dark(w.now_us()); // the check's account: no time between two spans the dark
                r.outcome = light::Outcome::Stopped { done: 0 };
                self.state = if unchanged { hpd::State::Dark } else { hpd::State::Stopped };
                pair.set(self.state, i);
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
            }
            hpd::Event::Busy(i) => {
                let (o, lit) = (light::SOCKETS[i], light::SOCKETS[pair.on()]);
                d.say(format_args!("{} NOT DRIVEN: A MONITOR PLUGGED IN THERE; {} IS LIT, AND THERE IS ONE SPARE PIPE", light::name(&o), light::name(&lit)));
                if i == 1 { batches::read_second(w, true, out, &mut Store::default()); }
            }
            // HDMI-A-2's monitor taken out while it was not driven: said, and its blocks read again.
            hpd::Event::Gone(1) => {
                d.say(format_args!("HDMI-A-2: ITS MONITOR TAKEN OUT, NOT DRIVEN"));
                batches::read_second(w, true, out, &mut Store::default());
            }
            hpd::Event::Gone(_) | hpd::Event::None => {}
        }
    }
}

/// A plan that reads nothing has nothing mapped.
struct Unmapped;
impl Bus for Unmapped { fn read(&mut self, _: Reg) -> Option<u32> { None } }
