#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State { Ready, Sleeping(u64), BlockedSend(usize), BlockedRecv(usize), BlockedReply(usize), BlockedIrq(u8), BlockedFlush, Exited }

impl State {
    pub fn wake(&mut self, now: u64) {
        if let Self::Sleeping(deadline) = *self { if now.wrapping_sub(deadline) < (1 << 63) { *self = Self::Ready; } }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Sleeping(_) => "SLEEPING",
            Self::BlockedSend(_) | Self::BlockedRecv(_) | Self::BlockedReply(_) => "IPC_WAIT",
            Self::BlockedIrq(_) => "IRQ_WAIT",
            Self::BlockedFlush => "FLUSH_WAIT",
            Self::Exited => "EXITED",
        }
    }
}
/// The first of `count` slots after `current`, round robin, for which `ready` holds; 0 if none.
pub fn next_where(count: usize, current: usize, ready: impl Fn(usize) -> bool) -> usize {
    (1..=count).map(|step| (current + step) % count).find(|&slot| ready(slot)).unwrap_or(0)
}
