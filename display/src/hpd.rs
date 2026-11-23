//! 213-DRV-0096: HDMI-A-1's monitor plugged and unplugged, after steps 1 to 4. The display engine's interrupt is not
//! display's, so HPD 1's sense (`DC_HPD_INT_STATUS`, step 1's batch 1) is polled. A change counts once it has held
//! `SETTLE_US`; a shorter one (a bounce) does nothing. An unplug takes the stages back (`light::take_back`, screen_gone
//! first); a monitor plugged in runs steps 1 to 4 again. `tests/display_hpd_host.rs` checks it.

use crate::light::Outcome;

/// How often HPD 1's sense is read.
pub const POLL_MS: usize = 50;
/// How long a change of the sense must hold before it counts: past an HDMI sink's HPD pulse (at least 100 ms, when it
/// says its EDID changed) and a connector's bounce, short against a person's plugging.
pub const SETTLE_US: u64 = 500_000;

/// The second output, as HPD 1 is followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Stages are in place: an unplug takes them back.
    Lit,
    /// None in place: a monitor plugged in runs steps 1 to 4.
    Dark,
    /// A stage failed or the build undid them: nothing more this boot (its mark stays).
    Stopped,
}

/// What a settled change asks of display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { None, TakeBack, LightUp }

/// The state after a run of steps 3 and 4: lit when stages are left in place, stopped after a failure or the build's
/// own undo, dark when none ran (no monitor, a refusal, a build of no stage).
pub fn state_after(outcome: Option<Outcome>) -> State {
    match outcome {
        Some(Outcome::Stopped { done }) | Some(Outcome::NotReady { done, .. }) if done > 0 => State::Lit,
        Some(Outcome::Failed { .. }) | Some(Outcome::Undone { .. }) => State::Stopped,
        _ => State::Dark,
    }
}

/// HPD 1 followed: its settled sense, a change seen since and when, and the output's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Follow { state: State, sense: bool, change: Option<u64> }

impl Follow {
    pub fn new(state: State, sense: bool) -> Follow { Follow { state, sense, change: None } }
    pub fn state(&self) -> State { self.state }
    /// After display acted on an action: the state its run or take-back left.
    pub fn set(&mut self, state: State) { self.state = state; }
    /// One read of the sense at `now_us`: an action once a change has held `SETTLE_US`, else none.
    pub fn feed(&mut self, sense: bool, now_us: u64) -> Action {
        if sense == self.sense { self.change = None; return Action::None; }
        let since = *self.change.get_or_insert(now_us);
        if now_us.saturating_sub(since) < SETTLE_US { return Action::None; }
        (self.sense, self.change) = (sense, None);
        match (self.state, sense) {
            (State::Lit, false) => Action::TakeBack,
            (State::Dark, true) => Action::LightUp,
            _ => Action::None,
        }
    }
}
