//! The device's clock from the pinned server's `Date` (801-EFF-0040, for 801's S5; 801-KRN-0255 gives the agent `rtc`'s
//! setting client): set when more than five minutes off, never before the build's commit time nor past 2099.

/// How far off the device's clock may be before the server's time is set on it; nearer is `timed`'s or the user's.
pub const OFF_S: u64 = 300;
/// The last second the RTC keeps: 2099-12-31 23:59:59 UTC.
pub const LAST_UNIX: u64 = 4_102_444_799;

/// The time to set from a server's `Date`, or none: the device's clock within `OFF_S` of it, or the date before `floor`
/// (the build's commit time) or past 2099. A device clock that cannot be read is set.
pub fn to_set(server: u64, device: Option<u64>, floor: u64) -> Option<u64> {
    if server < floor || server > LAST_UNIX { return None; }
    match device { Some(d) if d.abs_diff(server) <= OFF_S => None, _ => Some(server) }
}
