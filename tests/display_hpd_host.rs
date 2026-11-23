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
