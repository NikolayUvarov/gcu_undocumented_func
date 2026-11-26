//! 213-DRV-0096: HDMI-A-1's monitor plugged and unplugged, after steps 1 to 4. The display engine's interrupt is not
//! display's, so HPD 1's sense (`DC_HPD_INT_STATUS`, step 1's batch 1) is polled. A change counts once it has held
//! `SETTLE_US`; a shorter one (a bounce) does nothing. An unplug takes the stages back (`light::take_back`, screen_gone
//! first); a monitor plugged in runs steps 1 to 4 again. Both HDMI sockets are followed by `Pair` (213-DRV-0112): one
//! spare pipe, so one socket lit at a time. `tests/display_hpd_host.rs` checks it.

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

/// One HPD sense settled (213-DRV-0112): a change counts once it has held `SETTLE_US`, as `Follow` counts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settle { sense: bool, change: Option<u64> }

impl Settle {
    pub fn new(sense: bool) -> Settle { Settle { sense, change: None } }
    /// The settled sense.
    pub fn sense(&self) -> bool { self.sense }
    /// One read of the sense at `now_us`: the new settled sense once a change has held `SETTLE_US`, else none.
    pub fn feed(&mut self, sense: bool, now_us: u64) -> Option<bool> {
        if sense == self.sense { self.change = None; return None; }
        let since = *self.change.get_or_insert(now_us);
        if now_us.saturating_sub(since) < SETTLE_US { return None; }
        (self.sense, self.change) = (sense, None);
        Some(sense)
    }
}

/// What a settled change of either socket asks of display (213-DRV-0112), by the socket's index in `light::SOCKETS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    None,
    /// The lit socket's monitor taken out: its stages taken back.
    TakeBack(usize),
    /// A monitor in a socket while none is lit: steps 1 to 4 on it.
    LightUp(usize),
    /// A monitor plugged into the socket not lit while the other is: not driven, one spare pipe.
    Busy(usize),
    /// A monitor taken out of a socket not lit.
    Gone(usize),
}

/// Both sockets' senses followed (213-DRV-0112). The lit socket's unplug takes it back, and if the other socket's monitor
/// is in, that one lights at the next poll. With none lit a plug lights its socket; with one lit a plug into the other is
/// busy. A stopped output does nothing more. A socket whose sense is not followed (None) is passed over. With the second
/// socket not followed it acts as `Follow` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pair { state: State, on: usize, settle: [Option<Settle>; 2], pending: Option<usize> }

impl Pair {
    /// `on`: the socket the state is of (the lit one, or the last taken); `senses`: each followed socket's sense now.
    pub fn new(state: State, on: usize, senses: [Option<bool>; 2]) -> Pair {
        Pair { state, on: on.min(1), settle: senses.map(|s| s.map(Settle::new)), pending: None }
    }
    pub fn state(&self) -> State { self.state }
    pub fn on(&self) -> usize { self.on }
    /// A socket's settled sense, if it is followed.
    pub fn sense(&self, socket: usize) -> Option<bool> { self.settle.get(socket).copied().flatten().map(|s| s.sense()) }
    /// After display acted on an event: the state its run or take-back left, and the socket it was on.
    pub fn set(&mut self, state: State, on: usize) { (self.state, self.on) = (state, on.min(1)); }
    /// One read of each followed socket at `now_us`; the lit socket's change first. One event a poll; a change of the
    /// other socket in the same poll counts at the next.
    pub fn feed(&mut self, senses: [Option<bool>; 2], now_us: u64) -> Event {
        if self.state == State::Stopped { return Event::None; }
        if let Some(s) = self.pending.take() {
            if self.state == State::Dark && self.sense(s) == Some(true) { return Event::LightUp(s); }
        }
        for s in [self.on, 1 - self.on] {
            let (Some(settle), Some(sense)) = (self.settle[s].as_mut(), senses[s]) else { continue };
            let Some(new) = settle.feed(sense, now_us) else { continue };
            let other = 1 - s;
            return match (self.state, s == self.on, new) {
                (State::Lit, true, false) => { if self.sense(other) == Some(true) { self.pending = Some(other); } Event::TakeBack(s) }
                (State::Lit, true, true) => Event::None,
                (State::Lit, false, true) => Event::Busy(s),
                (_, _, false) => Event::Gone(s),
                (_, _, true) => Event::LightUp(s),
            };
        }
        Event::None
    }
}
