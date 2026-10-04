use crate::ipc::Endpoint;

/// Seconds since midnight from the CMOS RTC (via the ring 3 `rtc` driver, idl/rtc.wit), no time zone.
pub fn seconds_since_midnight() -> Option<usize> {
    let seconds = crate::idl::rtc::now(Endpoint::RTC).ok()??;
    (seconds < 24 * 3600).then_some(seconds as usize)
}
