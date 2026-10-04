use crate::ipc::Endpoint;

/// Seconds since midnight from the CMOS RTC (via the ring 3 `rtc` driver, idl/rtc.wit), no time zone.
pub fn seconds_since_midnight() -> Option<usize> {
    let seconds = crate::idl::rtc::now(Endpoint::RTC).ok()??;
    (seconds < 24 * 3600).then_some(seconds as usize)
}

/// Today's date (year, month 1-12, day 1-31) from the CMOS RTC, no time zone.
pub fn date() -> Option<(u32, u32, u32)> {
    let days = crate::idl::rtc::date(Endpoint::RTC).ok()??;
    Some(civil_from_days(days))
}

/// Days since 2000-01-01 for a valid date in 2000..=9999.
pub fn days_from_civil(year: u32, month: u32, day: u32) -> Option<u32> {
    let leap = |y: u32| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    const DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if !(2000..=9999).contains(&year) || !(1..=12).contains(&month) { return None; }
    let length = DAYS[month as usize - 1] + (month == 2 && leap(year)) as u32;
    if day == 0 || day > length { return None; }
    let years: u32 = (2000..year).map(|y| 365 + leap(y) as u32).sum();
    let months: u32 = (1..month).map(|m| DAYS[m as usize - 1] + (m == 2 && leap(year)) as u32).sum();
    Some(years + months + day - 1)
}

/// (year, month, day) of a day number since 2000-01-01.
pub fn civil_from_days(mut days: u32) -> (u32, u32, u32) {
    let leap = |y: u32| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let mut year = 2000;
    loop { let length = 365 + leap(year) as u32; if days < length { break; } days -= length; year += 1; }
    const DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1;
    loop { let length = DAYS[month - 1] + (month == 2 && leap(year)) as u32; if days < length { break; } days -= length; month += 1; }
    (year, month as u32, days + 1)
}
