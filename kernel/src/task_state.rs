#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Empty,
    Ready,
    Sleeping(u64),
    Exited,
}

impl State {
    pub fn wake(&mut self, now: u64) {
        if let Self::Sleeping(deadline) = *self {
            if now.wrapping_sub(deadline) < (1 << 63) {
                *self = Self::Ready;
            }
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "EMPTY",
            Self::Ready => "READY",
            Self::Sleeping(_) => "SLEEPING",
            Self::Exited => "EXITED",
        }
    }
}

// Slot zero is the shell/idle task. It is always a runnable fallback, even while
// a program owns the display. The next task is chosen in round-robin order.
pub fn next(states: &[State], current: usize) -> usize {
    (1..=states.len())
        .map(|step| (current + step) % states.len())
        .find(|&slot| states[slot] == State::Ready)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_robin_includes_shell_and_skips_sleeping_and_dead_tasks() {
        let states = [
            State::Ready,
            State::Ready,
            State::Sleeping(10),
            State::Exited,
            State::Ready,
        ];
        assert_eq!(next(&states, 0), 1);
        assert_eq!(next(&states, 1), 4);
        assert_eq!(next(&states, 4), 0);
    }
    #[test]
    fn sleepers_wake_at_deadline_including_timer_wraparound() {
        let mut state = State::Sleeping(20);
        state.wake(19);
        assert_eq!(state, State::Sleeping(20));
        state.wake(20);
        assert_eq!(state, State::Ready);
        state = State::Sleeping(5);
        state.wake(u64::MAX - 2);
        assert_eq!(state, State::Sleeping(5));
        state.wake(5);
        assert_eq!(state, State::Ready);
    }
    #[test]
    fn no_program_runnable_returns_to_idle_shell() {
        assert_eq!(
            next(&[State::Ready, State::Sleeping(100), State::Exited], 0),
            0
        );
        let mut dead = State::Exited;
        dead.wake(u64::MAX);
        assert_eq!(dead, State::Exited);
    }
}
