#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State { Empty, Ready, Sleeping(u64), BlockedIpc, BlockedIrq(u8), Exited }

impl State {
    pub fn wake(&mut self, now: u64) {
        if let Self::Sleeping(deadline) = *self { if now.wrapping_sub(deadline) < (1 << 63) { *self = Self::Ready; } }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "EMPTY",
            Self::Ready => "READY",
            Self::Sleeping(_) => "SLEEPING",
            Self::BlockedIpc => "IPC_WAIT",
            Self::BlockedIrq(_) => "IRQ_WAIT",
            Self::Exited => "EXITED",
        }
    }
}
pub fn next(states: &[State], current: usize) -> usize {
    (1..=states.len()).map(|step| (current + step) % states.len()).find(|&slot| states[slot] == State::Ready).unwrap_or(0)
}
