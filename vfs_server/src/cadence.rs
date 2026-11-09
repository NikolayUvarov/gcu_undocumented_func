// When the system log is saved (211-KRN-0223): in the boot's first BOOT_MS every record within SAVE_MS, so that the
// log of a boot that hangs reaches its last seconds; after that a record at or above a warning within SAVE_MS, the
// others at most ORDINARY_MS after the last save or once MANY wait (well within logd's ring of 2048), so that a quiet
// machine's disk is not written every few seconds (a USB boot disk was written every 11 s, about 25 commands a save).
// Each record is looked at once. Pure, for the host tests.

pub const SAVE_MS: u64 = 2000;
pub const ORDINARY_MS: u64 = 30_000;
pub const BOOT_MS: u64 = 120_000; // since the kernel started
pub const MANY: u64 = 256;
pub const WARNING: u8 = 2; // idl/log.wit's levels: 0 debug, 1 info, 2 warning, 3 error

pub struct Cadence { seen: u64, saved_at: u64 }

impl Cadence {
    pub const fn new() -> Cadence { Cadence { seen: 0, saved_at: 0 } }

    /// Whether to save now, `now_ms` since the kernel started: `records` are (sequence number, level) from
    /// `max(seen, next)` on, `next` the first not saved. It stops reading at the first warning.
    pub fn wanted(&mut self, next: u64, now_ms: u64, records: impl IntoIterator<Item = (u64, u8)>) -> bool {
        self.seen = self.seen.max(next);
        for (seq, level) in records {
            if seq < self.seen { continue; }
            if level >= WARNING { return true; }
            self.seen = seq + 1;
        }
        self.seen > next && (now_ms < BOOT_MS || self.seen - next >= MANY || now_ms >= self.saved_at.saturating_add(ORDINARY_MS))
    }

    /// A save was made at `now_ms`.
    pub fn saved(&mut self, now_ms: u64) { self.saved_at = now_ms; }

    /// The first record not looked at yet.
    pub fn seen(&self, next: u64) -> u64 { self.seen.max(next) }
}
