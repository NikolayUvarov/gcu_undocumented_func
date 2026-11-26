//! 213-DRV-0099: display's start as the setting's plan asks (`setting::Plan`): step 1 on the engine's window, step 2 and
//! steps 3 and 4 on its writer, what each part took and what was read and written; then what follows the stages, HPD 1
//! (213-DRV-0096) and the standing check (213-DRV-0103). main.rs runs it on the Radeon's windows, the host cases
//! (`tests/display_light_host.rs`) on a model: a plan that reads nothing maps nothing, one that writes nothing makes no
//! writer.

use crate::batches::{self, Bus, Order, Out, Store};
use crate::dcn::{self, field::{DC_HPD_EN, DC_HPD_SENSE}, Reg, R};
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
    let edid = step2(d, plan, &store, writer.as_mut(), out);
    let t3 = d.now_us();
    let run = step3(d, plan, &store, writer.as_mut(), vfct, edid.as_ref(), build, out);
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

/// Step 2 (213-DRV-0095): the EDID over DDC1, on the writer, and what it says.
fn step2<D: Device>(d: &mut D, plan: &Plan, store: &Store, writer: Option<&mut D::Writer>, out: &mut dyn Out) -> Option<edid::Edid> {
    if !plan.edid { d.say(format_args!("STEP 2 (THE EDID OVER DDC1) NOT RUN: THE SETTING DOES NOT ASK FOR IT")); return None; }
    let Some(w) = writer else { d.say(format_args!("STEP 2 NOT RUN: STEP 1 DID NOT MAP THE ENGINE'S WINDOW")); return None };
    let read = ddc::read(store, w, out).ok()?;
    ddc::describe(&read, out)
}

/// Steps 3 and 4: their run, when their stages ran. With no stage in the plan they only describe the stages, for the pipe
/// step 1 found or, with step 1 not read, for the survey's expectation.
fn step3<'a, D: Device>(d: &mut D, plan: &Plan, store: &Store, writer: Option<&mut D::Writer>, vfct: Option<&'a [u8]>, edid: Option<&edid::Edid>, build: &'static str, out: &mut dyn Out) -> Option<Run<'a>> {
    let (timing, signal, why) = light::mode_for(edid, timing::first_light_mode(&HDMI_A_1_VICS));
    if plan.edid { d.say(format_args!("STEP 3'S MODE: {}x{} AT {} KHZ IN {:?} SIGNALLING, {}", timing.h_active, timing.v_active, timing.pixel_khz, signal, why)); }
    let plan3 = match light::prepare(store, timing, signal, build) {
        Ok(p) => p.with_watch(plan.watch_frames),
        Err(why) if plan.stages == 0 => {
            d.say(format_args!("STEP 3 SHOWN FOR THE SURVEY'S EXPECTATION: {}", why));
            let (f, c) = light::EXPECTED;
            if let Ok(p) = light::Light::new_with(f, c, timing, signal) { light::describe(&p.with_build(build).with_watch(plan.watch_frames), true, out); }
            return None;
        }
        Err(why) => { d.say(format_args!("STEP 3 NOT RUN: {}", why)); return None; }
    };
    if plan.stages == 0 { light::describe(&plan3, false, out); return None; }
    let Some(writer) = writer else { d.say(format_args!("STEP 3 NOT RUN: STEP 1 DID NOT MAP THE ENGINE'S WINDOW")); return None };
    let Some((0x1002, 0x1638, image)) = vfct.and_then(atom::vfct_image) else { d.say(format_args!("STEP 3 NOT RUN: NO VBIOS IMAGE FOR 1002:1638")); return None };
    let Ok(image) = atom::Image::new(image) else { d.say(format_args!("STEP 3 NOT RUN: THE VBIOS IMAGE HAS NO ATOM ROM HEADER")); return None };
    let mut interp = Box::new(atom::Interpreter::new(image));
    let mut journal = Box::new(light::Journal::default());
    let (outcome, times) = light::run_timed(&plan3, writer, &mut interp, &mut journal, plan.stages, plan.undo_after_stages, out);
    Some(Run { light: Box::new(plan3), interp, journal, outcome, times })
}

/// The stages a run left in place.
fn done(r: &Run) -> u8 { match r.outcome { light::Outcome::Stopped { done } | light::Outcome::NotReady { done, .. } => done, _ => 0 } }

/// After steps 1 to 4: HPD 1 followed when the plan asks (213-DRV-0096), read every `hpd::POLL_MS`, and the standing
/// check (213-DRV-0103), once a second while stage 15's picture is up. An unplug that holds takes the stages back
/// (screen_gone first); a monitor plugged in that holds runs steps 1 to 4 again on the same writer, step 1 and the EDID
/// read anew; a new underflow or request timeout undoes them as the watch does; after a failure nothing more is tried.
pub struct Follower<'a> { run: Option<Run<'a>>, state: hpd::State, f: Option<hpd::Follow>, account: light::Standing, next: u64, standing: bool, sense: Reg }

impl<'a> Follower<'a> {
    /// What follows the stages, when the plan asks for it and does not undo them; None, and display idles.
    pub fn new<D: Device>(d: &mut D, w: &mut D::Writer, run: Option<Run<'a>>, plan: &Plan) -> Option<Follower<'a>> {
        if !(plan.hotplug || plan.standing_check) || plan.undo_after_stages { return None; }
        let (Some(sense), Some(control)) = (R::HpdIntStatus.at(light::HDMI_A_1.hpd), R::HpdControl.at(light::HDMI_A_1.hpd)) else { return None };
        let state = hpd::state_after(run.as_ref().map(|r| r.outcome));
        let mut f = None;
        if plan.hotplug {
            if w.read(control).map(|v| DC_HPD_EN.get(v)) != Some(1) { d.say(format_args!("HPD 1 IS NOT ENABLED: ITS SENSE IS NOT FOLLOWED")); }
            else if let Some(now) = w.read(sense).map(|v| DC_HPD_SENSE.get(v) == 1) {
                d.say(format_args!("HPD 1 FOLLOWED EVERY {} MS, A CHANGE COUNTED AFTER {} MS: SENSE {}, THE SECOND OUTPUT {:?}", hpd::POLL_MS,
                                   hpd::SETTLE_US / 1000, now as u8, state));
                f = Some(hpd::Follow::new(state, now));
            } else { d.say(format_args!("HPD 1 CANNOT BE READ: NOT FOLLOWED")); }
        }
        if f.is_none() && !plan.standing_check { return None; }
        if plan.standing_check {
            d.say(format_args!("THE STANDING CHECK: BOTH PIPES' UNDERFLOW STATUSES READ EVERY {} MS WHILE THE SECOND PICTURE IS UP; THE WATCH {} FRAMES",
                               light::STANDING_US / 1000, run.as_ref().map_or(light::WATCH_FRAMES, |r| r.light.watch_frames)));
        }
        let account = light::Standing::new(w.now_us());
        let next = w.now_us() + light::STANDING_US;
        Some(Follower { run, state, f, account, next, standing: plan.standing_check, sense })
    }

    /// How long display sleeps between two polls.
    pub fn sleep_ms(&self) -> usize { if self.f.is_some() { hpd::POLL_MS } else { (light::STANDING_US / 1000) as usize } }
    pub fn state(&self) -> hpd::State { self.state }

    /// One poll after the sleep: the standing check when it is due, then HPD 1's sense.
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
                    if let Some(f) = self.f.as_mut() { f.set(self.state); }
                }
            }
        }
        let Some(follow) = self.f.as_mut() else { return };
        let Some(sense) = w.read(self.sense).map(|v| DC_HPD_SENSE.get(v) == 1) else { return };
        match follow.feed(sense, w.now_us()) {
            hpd::Action::TakeBack => {
                let Some(r) = self.run.as_mut() else { follow.set(hpd::State::Dark); self.state = hpd::State::Dark; return };
                let stages = done(r);
                let unchanged = light::take_back(&r.light, w, &mut r.interp, &mut r.journal, stages, out);
                self.account.dark(w.now_us()); // the check's account: no time between two spans the dark
                r.outcome = light::Outcome::Stopped { done: 0 };
                self.state = if unchanged { hpd::State::Dark } else { hpd::State::Stopped };
                follow.set(self.state);
            }
            hpd::Action::LightUp => {
                drop(self.run.take()); // the last run's journal and interpreter let go before the next is made
                let mut store = Store::default();
                read_all(w, plan, out, &mut store);
                let edid = if plan.edid { ddc::read(&store, w, out).ok().and_then(|r| ddc::describe(&r, out)) } else { None };
                self.run = step3(d, plan, &store, Some(&mut *w), vfct, edid.as_ref(), build, out);
                self.state = hpd::state_after(self.run.as_ref().map(|r| r.outcome));
                follow.set(self.state);
                self.next = w.now_us() + light::STANDING_US;
                d.say(format_args!("A MONITOR PLUGGED IN: STEPS 1 TO 4 RAN AGAIN, THE SECOND OUTPUT {:?}", self.state));
            }
            hpd::Action::None => {}
        }
    }
}

/// A plan that reads nothing has nothing mapped.
struct Unmapped;
impl Bus for Unmapped { fn read(&mut self, _: Reg) -> Option<u32> { None } }
