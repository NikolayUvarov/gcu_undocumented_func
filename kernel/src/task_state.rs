#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Empty: a free slot in the host tests' lists
pub enum State { Empty, Ready, Sleeping(u64), BlockedSend(usize), BlockedRecv(usize), BlockedReply(usize), BlockedIrq(u8), BlockedFlush, Exited }

impl State {
    pub fn wake(&mut self, now: u64) {
        if let Self::Sleeping(deadline) = *self { if now.wrapping_sub(deadline) < (1 << 63) { *self = Self::Ready; } }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "EMPTY",
            Self::Ready => "READY",
            Self::Sleeping(_) => "SLEEPING",
            Self::BlockedSend(_) | Self::BlockedRecv(_) | Self::BlockedReply(_) => "IPC_WAIT",
            Self::BlockedIrq(_) => "IRQ_WAIT",
            Self::BlockedFlush => "FLUSH_WAIT",
            Self::Exited => "EXITED",
        }
    }
}
#[allow(dead_code)] // the host tests (tests/runtime.rs) pick from a list of states
pub fn next(states: &[State], current: usize) -> usize {
    next_by(states.len(), current, |slot| states[slot] == State::Ready)
}
// Round robin over `len` slots after `current`: the first that is ready, or 0.
#[allow(dead_code)] // the kernel picks from a CPU's own slots (next_in); the host tests compare the two
pub fn next_by(len: usize, current: usize, ready: impl Fn(usize) -> bool) -> usize {
    (1..=len).map(|step| (current + step) % len).find(|&slot| ready(slot)).unwrap_or(0)
}
// The same round robin over a sorted list of slots (one CPU's tasks): the first ready slot after `current`, else from
// the start up to and including `current`, or 0.
pub fn next_in(slots: &[usize], current: usize, ready: impl Fn(usize) -> bool) -> usize {
    let after = slots.partition_point(|&slot| slot <= current);
    slots[after..].iter().chain(&slots[..after]).copied().find(|&slot| ready(slot)).unwrap_or(0)
}
