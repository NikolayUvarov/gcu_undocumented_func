//! The shell's way to its first frame (213-APP-0075, for 213-KRN-0098): lines for the system log that tell a shell that
//! never drew from one that waited on a call, and from a screen that never showed what it drew. Host-tested in
//! tests/shell_host.rs.
use alloc::format;
use alloc::string::String;

/// A step before the first frame that takes longer than this is named in the system log.
pub const SLOW_MS: u64 = 5000;

/// Times the steps until the first frame; after it, nothing.
pub struct Start { since: u64, drawn: bool }

impl Start {
    pub fn new(now: u64) -> Self { Self { since: now, drawn: false } }

    pub fn drawn(&self) -> bool { self.drawn }

    /// No frame will come (no screen): nothing more is timed.
    pub fn stop(&mut self) { self.drawn = true; }

    /// `what` ended at `now` (milliseconds since boot): its line when it took longer than `SLOW_MS`.
    pub fn step(&mut self, what: &str, now: u64) -> Option<String> {
        if self.drawn { return None; }
        let took = now.saturating_sub(self.since);
        self.since = now;
        (took > SLOW_MS).then(|| format!("[SHELL] {} TOOK {} MS BEFORE THE FIRST FRAME", what, took))
    }

    /// The first frame, `width` × `height` pixels, presented at `now`: its line, once.
    pub fn frame(&mut self, width: usize, height: usize, now: u64) -> Option<String> {
        if core::mem::replace(&mut self.drawn, true) { return None; }
        Some(format!("[SHELL] FIRST FRAME {}X{} AT {}.{} S", width, height, now / 1000, now % 1000 / 100))
    }
}
