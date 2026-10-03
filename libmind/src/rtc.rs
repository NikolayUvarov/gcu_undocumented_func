use crate::abi::RTC_UNAVAILABLE;
use crate::ipc::{Endpoint, Message};

/// Seconds since midnight from the CMOS RTC (via the ring 3 `rtc` driver), no time zone.
pub fn seconds_since_midnight() -> Option<usize> {
    let reply = Endpoint::RTC.call(&Message::default(), 0).ok()?;
    let seconds = reply.data[0];
    (seconds != RTC_UNAVAILABLE && seconds < 24 * 3600).then_some(seconds)
}
