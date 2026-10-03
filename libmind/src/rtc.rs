use crate::abi::RTC_UNAVAILABLE;
use crate::ipc::{Endpoint, Message};

/// Секунды от полуночи по CMOS RTC (через драйвер `rtc` в ring 3), без часового пояса.
pub fn seconds_since_midnight() -> Option<usize> {
    let reply = Endpoint::RTC.call(&Message::default(), 0).ok()?;
    let seconds = reply.data[0];
    (seconds != RTC_UNAVAILABLE && seconds < 24 * 3600).then_some(seconds)
}
