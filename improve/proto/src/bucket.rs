//! A token bucket on the hub's clock (docs/hub/README.md, 5.2): `size` tokens at most, `per_day` added over a day, one
//! taken by each request. A clock that went back adds nothing and takes nothing away.
//!
//! Credit is kept in units of a millisecond times the day's rate, so a token is DAY_MS units whatever the rate, and a
//! refill between two calls a moment apart is never lost to rounding.

use crate::DAY_MS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bucket {
    size: u32,
    per_day: u32,
    units: u64,
    at_ms: u64,
}

impl Bucket {
    /// A full bucket at `now_ms`.
    pub fn new(size: u32, per_day: u32, now_ms: u64) -> Bucket { Bucket { size, per_day, units: size as u64 * DAY_MS, at_ms: now_ms } }

    /// The same bucket with another size and rate (a user whose band changed), its credit kept within the new size.
    pub fn resized(mut self, size: u32, per_day: u32, now_ms: u64) -> Bucket {
        self.refill(now_ms);
        self.size = size;
        self.per_day = per_day;
        self.units = self.units.min(size as u64 * DAY_MS);
        self
    }

    fn refill(&mut self, now_ms: u64) {
        if now_ms <= self.at_ms { return; }
        let added = (now_ms - self.at_ms) as u128 * self.per_day as u128;
        self.units = (self.units as u128 + added).min(self.size as u128 * DAY_MS as u128) as u64;
        self.at_ms = now_ms;
    }

    /// Takes one token at `now_ms`; otherwise how many milliseconds until one is there (None: never, a rate of 0).
    pub fn take(&mut self, now_ms: u64) -> Result<(), Option<u64>> {
        self.refill(now_ms);
        if self.units >= DAY_MS {
            self.units -= DAY_MS;
            return Ok(());
        }
        if self.per_day == 0 || self.size == 0 { return Err(None); }
        Err(Some((DAY_MS - self.units).div_ceil(self.per_day as u64)))
    }

    /// Whole tokens there at `now_ms`, without taking one.
    pub fn tokens(&self, now_ms: u64) -> u32 {
        let mut copy = *self;
        copy.refill(now_ms);
        (copy.units / DAY_MS) as u32
    }
}
