// Host cases of display's following of HPD 1 (213-DRV-0096): a change counts once it has held hpd::SETTLE_US, a bounce
// does nothing, an unplug of a lit output takes it back, a plug of a dark one lights it, a stopped one does neither.
#![allow(dead_code)] // the driver uses parts these cases do not
#[path = "../display/src/timing.rs"] mod timing;
#[path = "../display/src/edid.rs"] mod edid;
#[path = "../display/src/dcn.rs"] mod dcn;
#[path = "../display/src/batches.rs"] mod batches;
#[path = "../display/src/atom.rs"] mod atom;
#[path = "../display/src/light.rs"] mod light;
#[path = "../display/src/picture.rs"] mod picture;
#[path = "../display/src/hpd.rs"] mod hpd;

use hpd::{state_after, Action, Follow, State, POLL_MS, SETTLE_US};
use light::{Failure, Outcome};

const POLL_US: u64 = POLL_MS as u64 * 1000;

// The actions a trace of senses gives, one read a poll from `t0`: (the poll's time, the action) for each that acts.
fn trace(f: &mut Follow, t0: u64, senses: &[(bool, u64)], act: &mut dyn FnMut(Action, &mut Follow)) -> Vec<(u64, Action)> {
    let mut out = vec![];
    let mut t = t0;
    for &(sense, for_us) in senses {
        let end = t + for_us;
        while t < end {
            let a = f.feed(sense, t);
            if a != Action::None { out.push((t, a)); act(a, f); }
            t += POLL_US;
        }
    }
    out
}
// What display does: a take-back leaves the output dark, a plug lights it.
fn display(a: Action, f: &mut Follow) { f.set(if a == Action::TakeBack { State::Dark } else { State::Lit }); }

#[test]
fn an_unplug_that_holds_takes_a_lit_output_back_once_and_a_plug_that_holds_lights_it_again() {
    let mut f = Follow::new(State::Lit, true);
    let got = trace(&mut f, 0, &[(true, 1_000_000), (false, 2_000_000), (true, 2_000_000)], &mut display);
    // Each change counts at the first poll that finds it held SETTLE_US: once, not again while it holds.
    assert_eq!(got, [(1_000_000 + SETTLE_US, Action::TakeBack), (3_000_000 + SETTLE_US, Action::LightUp)]);
    assert_eq!(f.state(), State::Lit);
}

#[test]
fn a_bounce_shorter_than_the_settle_does_neither() {
    for (state, sense) in [(State::Lit, true), (State::Dark, false)] {
        let mut f = Follow::new(state, sense);
        // Away for less than the settle, back, away again: each run counted afresh.
        let away = SETTLE_US - POLL_US;
        let got = trace(&mut f, 0, &[(!sense, away), (sense, POLL_US), (!sense, away), (sense, 1_000_000)], &mut display);
        assert!(got.is_empty(), "{:?} {:?}", state, got);
        // Flickering at every poll for seconds: nothing either.
        let flicker: Vec<(bool, u64)> = (0..100).map(|i| (if i % 2 == 0 { !sense } else { sense }, POLL_US)).collect();
        assert!(trace(&mut f, 2_000_000, &flicker, &mut display).is_empty(), "{:?}", state);
        assert_eq!(f.state(), state);
    }
}

#[test]
fn a_change_to_what_the_output_already_is_and_a_stopped_output_do_nothing() {
    // Dark and unplugged, lit and plugged in: nothing to do.
    let mut f = Follow::new(State::Dark, true);
    assert!(trace(&mut f, 0, &[(false, 2_000_000)], &mut display).is_empty());
    let mut f = Follow::new(State::Lit, false);
    assert!(trace(&mut f, 0, &[(true, 2_000_000)], &mut display).is_empty());
    // After a failure: neither an unplug nor a plug acts.
    let mut f = Follow::new(State::Stopped, true);
    assert!(trace(&mut f, 0, &[(false, 2_000_000), (true, 2_000_000), (false, 2_000_000)], &mut display).is_empty());
    // A read that comes late (display kept from running) still counts the change once it has held.
    let mut f = Follow::new(State::Lit, true);
    assert_eq!(f.feed(false, 10_000_000), Action::None);
    assert_eq!(f.feed(false, 10_000_000 + 3 * SETTLE_US), Action::TakeBack);
}

#[test]
fn the_output_is_lit_while_stages_are_in_place_stopped_after_a_failure_and_dark_otherwise() {
    assert_eq!(state_after(Some(Outcome::Stopped { done: 16 })), State::Lit);
    assert_eq!(state_after(Some(Outcome::Stopped { done: 8 })), State::Lit);
    assert_eq!(state_after(Some(Outcome::NotReady { done: 15, stage: 16, why: Failure::NoSurface })), State::Lit);
    assert_eq!(state_after(Some(Outcome::Stopped { done: 0 })), State::Dark);
    assert_eq!(state_after(None), State::Dark);
    assert_eq!(state_after(Some(Outcome::Failed { stage: 5, why: Failure::NoFrames(0) })), State::Stopped);
    assert_eq!(state_after(Some(Outcome::Failed { stage: 0, why: Failure::StartedAgain })), State::Stopped);
    assert_eq!(state_after(Some(Outcome::Undone { done: 16 })), State::Stopped);
    // The settle is past an HDMI sink's HPD pulse of 100 ms, and a few polls long.
    assert!(SETTLE_US > 100_000 && SETTLE_US >= 4 * POLL_US);
}

// --- both HDMI sockets (213-DRV-0112) ---

use hpd::{Event, Pair};

// Both sockets' senses, one read a poll from `t0`: the events, display acting on each as start.rs does (a take-back
// leaves the output dark on its socket, a light-up lights the socket it names; busy and gone change nothing).
fn trace2(p: &mut Pair, t0: u64, senses: &[([Option<bool>; 2], u64)]) -> Vec<(u64, Event)> {
    let mut out = vec![];
    let mut t = t0;
    for &(s, for_us) in senses {
        let end = t + for_us;
        while t < end {
            let e = p.feed(s, t);
            match e { Event::TakeBack(i) => p.set(State::Dark, i), Event::LightUp(i) => p.set(State::Lit, i), _ => {} }
            if e != Event::None { out.push((t, e)); }
            t += POLL_US;
        }
    }
    out
}
const S: u64 = 1_000_000;
fn both(a: bool, b: bool) -> [Option<bool>; 2] { [Some(a), Some(b)] }

#[test]
fn a_monitor_in_the_second_socket_alone_lights_it_and_its_unplug_takes_it_back() {
    let mut p = Pair::new(State::Dark, 0, both(false, false));
    let got = trace2(&mut p, 0, &[(both(false, false), S), (both(false, true), 2 * S), (both(false, false), 2 * S)]);
    assert_eq!(got, [(S + SETTLE_US, Event::LightUp(1)), (3 * S + SETTLE_US, Event::TakeBack(1))]);
    assert_eq!((p.state(), p.on()), (State::Dark, 1));
    // Plugged in at boot and not lit (the build only reads it): nothing until it changes; out, then in again, lights it.
    let mut p = Pair::new(State::Dark, 0, both(false, true));
    assert!(trace2(&mut p, 0, &[(both(false, true), 3 * S)]).is_empty());
    assert_eq!(trace2(&mut p, 3 * S, &[(both(false, false), S), (both(false, true), S)]), [(3 * S + SETTLE_US, Event::Gone(1)), (4 * S + SETTLE_US, Event::LightUp(1))]);
}

#[test]
fn with_a_monitor_in_each_the_lit_one_stays_the_other_is_busy_and_lights_once_the_first_is_taken_out() {
    // Both in, HDMI-A-1 lit: its unplug takes it back, and HDMI-A-2 lights at the next poll.
    let mut p = Pair::new(State::Lit, 0, both(true, true));
    let got = trace2(&mut p, 0, &[(both(true, true), S), (both(false, true), 2 * S)]);
    assert_eq!(got, [(S + SETTLE_US, Event::TakeBack(0)), (S + SETTLE_US + POLL_US, Event::LightUp(1))]);
    // HDMI-A-1's monitor back: busy, not lit. HDMI-A-2's out: taken back, and HDMI-A-1 lights.
    let got = trace2(&mut p, 3 * S, &[(both(true, true), 2 * S), (both(true, false), 2 * S)]);
    assert_eq!(got, [(3 * S + SETTLE_US, Event::Busy(0)), (5 * S + SETTLE_US, Event::TakeBack(1)), (5 * S + SETTLE_US + POLL_US, Event::LightUp(0))]);
    assert_eq!((p.state(), p.on()), (State::Lit, 0));
    // HDMI-A-1 lit alone: a monitor plugged into HDMI-A-2 is busy, taken out gone; HDMI-A-1's unplug lights nothing after it.
    let mut p = Pair::new(State::Lit, 0, both(true, false));
    let got = trace2(&mut p, 0, &[(both(true, true), 2 * S), (both(true, false), 2 * S), (both(false, false), 2 * S)]);
    assert_eq!(got, [(SETTLE_US, Event::Busy(1)), (2 * S + SETTLE_US, Event::Gone(1)), (4 * S + SETTLE_US, Event::TakeBack(0))]);
    assert_eq!(p.state(), State::Dark);
}

#[test]
fn a_monitor_moved_from_one_socket_to_the_other_and_back_is_lit_each_time() {
    let mut p = Pair::new(State::Lit, 0, both(true, false));
    let got = trace2(&mut p, 0, &[(both(false, false), 2 * S), (both(false, true), 2 * S), (both(false, false), 2 * S), (both(true, false), 2 * S)]);
    assert_eq!(got, [(SETTLE_US, Event::TakeBack(0)), (2 * S + SETTLE_US, Event::LightUp(1)), (4 * S + SETTLE_US, Event::TakeBack(1)), (6 * S + SETTLE_US, Event::LightUp(0))]);
    // Moved at once (out of one and into the other in the same poll): the lit socket's unplug first, then the other lights.
    let mut p = Pair::new(State::Lit, 0, both(true, false));
    let got = trace2(&mut p, 0, &[(both(false, true), 2 * S)]);
    assert_eq!(got.iter().map(|g| g.1).collect::<Vec<_>>(), [Event::TakeBack(0), Event::LightUp(1)]);
    assert!(got[1].0 > got[0].0 && got[1].0 <= got[0].0 + 2 * POLL_US, "{:?}", got);
}

#[test]
fn a_bounce_on_either_socket_and_a_stopped_output_do_nothing_and_with_hpd_2_not_followed_the_pair_is_hpd_1s_follow() {
    let away = SETTLE_US - POLL_US;
    let mut p = Pair::new(State::Lit, 1, both(false, true));
    assert!(trace2(&mut p, 0, &[(both(true, true), away), (both(false, false), away), (both(false, true), S)]).is_empty());
    let mut p = Pair::new(State::Stopped, 0, both(true, false));
    assert!(trace2(&mut p, 0, &[(both(false, false), 2 * S), (both(false, true), 2 * S), (both(true, true), 2 * S)]).is_empty());
    // HPD 2 not followed: on every trace the cases above give `Follow`, the same actions at the same polls.
    let traces: [(State, bool, Vec<(bool, u64)>); 5] = [
        (State::Lit, true, vec![(true, S), (false, 2 * S), (true, 2 * S)]), (State::Dark, false, vec![(true, away), (false, POLL_US), (true, S)]),
        (State::Dark, true, vec![(false, 2 * S)]), (State::Lit, false, vec![(true, 2 * S)]), (State::Stopped, true, vec![(false, 2 * S), (true, 2 * S)]),
    ];
    for (state, sense, senses) in traces {
        let mut f = Follow::new(state, sense);
        let want = trace(&mut f, 0, &senses, &mut display);
        let mut p = Pair::new(state, 0, [Some(sense), None]);
        let wide: Vec<([Option<bool>; 2], u64)> = senses.iter().map(|&(s, us)| ([Some(s), None], us)).collect();
        let got: Vec<(u64, Action)> = trace2(&mut p, 0, &wide).into_iter().filter_map(|(t, e)| match e {
            Event::TakeBack(0) => Some((t, Action::TakeBack)), Event::LightUp(0) => Some((t, Action::LightUp)), Event::Gone(0) => None, other => panic!("{:?}", other) }).collect();
        assert_eq!(got, want, "{:?} {} {:?}", state, sense, senses);
    }
}
