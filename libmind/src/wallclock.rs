//! The time of day from a clock that is expensive to read (the RTC service): read again about once a minute, and the
//! seconds in between counted from the monotonic clock (000-APP-0012). The reads go to a function the caller gives,
//! so the counting is tested on the host.

/// How often the slow clock is read again.
pub const RESYNC_NS: u64 = 60_000_000_000;
/// How long a resynchronization waits for the slow clock's second to change before it takes the reading as it is.
const ALIGN_NS: u64 = 1_500_000_000;
const SECOND_NS: u64 = 1_000_000_000;
const DAY: usize = 86_400;

/// Seconds since midnight, counted from the last reading of a slow clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct WallClock {
    anchor: Option<(usize, u64)>, // a reading and the monotonic time its second began
    synced_ns: u64,               // when the last resynchronization began
    aligning: Option<usize>,      // during a resynchronization: the first reading, waited out until it changes
    last: Option<usize>,
    new_day: bool,
}

impl WallClock {
    pub const fn new() -> Self { Self { anchor: None, synced_ns: 0, aligning: None, last: None, new_day: true } }

    /// Seconds since midnight at monotonic time `now_ns`. `read` reads the slow clock; it is called only while
    /// resynchronizing: about once a minute, then until its second changes (a dozen calls at 100 ms).
    pub fn seconds(&mut self, now_ns: u64, mut read: impl FnMut() -> Option<usize>) -> Option<usize> {
        let due = self.anchor.is_none() || now_ns.saturating_sub(self.synced_ns) >= RESYNC_NS;
        let seconds = if due || self.aligning.is_some() {
            let s = read().filter(|&s| s < DAY)?;
            match self.aligning {
                // The second changed: the count starts at its beginning.
                Some(first) if first != s => { self.anchor = Some((s, now_ns)); self.aligning = None; }
                // A clock whose second does not change is taken as it is.
                Some(_) if now_ns.saturating_sub(self.synced_ns) >= ALIGN_NS => { self.anchor = Some((s, now_ns)); self.aligning = None; }
                Some(_) => {}
                None => { self.aligning = Some(s); self.synced_ns = now_ns; self.new_day = true; }
            }
            s
        } else {
            let (s, at) = self.anchor?;
            (s + (now_ns.saturating_sub(at) / SECOND_NS) as usize) % DAY
        };
        if self.last.is_some_and(|last| seconds < last) { self.new_day = true; }
        self.last = Some(seconds);
        Some(seconds)
    }

    /// True once after midnight and after each resynchronization: the date may have changed and should be read again.
    pub fn date_due(&mut self) -> bool { core::mem::replace(&mut self.new_day, false) }
}
