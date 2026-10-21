use crate::ipc::Endpoint;

/// The badge of the rtc client that may set the clock: init gives it to the shell (rtc.wit 1.2, 211-KRN-0051).
pub const BADGE_SET: u16 = 1;

/// Seconds since midnight UTC from the RTC (via the ring 3 `rtc` driver, idl/rtc.wit); the clocks show `Clock`'s or
/// `local_now`'s, in the zone.
pub fn seconds_since_midnight() -> Option<usize> {
    let seconds = crate::idl::rtc::now(Endpoint::RTC).ok()??;
    (seconds < 24 * 3600).then_some(seconds as usize)
}

/// The local time of day for a program that shows it often (a clock face): the RTC service is read about once a minute
/// and the seconds in between counted from the monotonic clock; the date is read again after local midnight
/// (000-APP-0012). The zone is read with the RTC, so a new one shows within a minute (000-APP-0099).
pub struct Clock { wall: crate::wallclock::WallClock, date: Option<(u32, u32, u32)>, zone: Zone }

impl Clock {
    pub const fn new() -> Self { Self { wall: crate::wallclock::WallClock::new(), date: None, zone: Zone::UTC } }
    /// Seconds since local midnight.
    pub fn seconds_since_midnight(&mut self) -> Option<usize> {
        let shown = &mut self.zone;
        self.wall.seconds(crate::time::monotonic_ns(), || {
            let utc = seconds_since_midnight()?;
            *shown = zone();
            Some(shown.local_seconds(utc as u32) as usize)
        })
    }
    /// Today's local date.
    pub fn date(&mut self) -> Option<(u32, u32, u32)> {
        if self.wall.date_due() || self.date.is_none() { self.date = local_now().map(|(days, _)| civil_from_days(days)); }
        self.date
    }
    /// The zone the time is shown in, as last read.
    pub fn zone(&self) -> Zone { self.zone }
    /// Milliseconds to wait before the shown second can change, `poll_ms` while the RTC is being read.
    pub fn wait_ms(&self, poll_ms: u64) -> usize { self.wall.wait_ms(crate::time::monotonic_ns(), poll_ms) as usize }
}

/// A time zone: its offset from UTC in minutes, east of it positive (UTC+03:00 is 180, UTC-05:30 is -330), with no
/// daylight saving (000-APP-0099). The RTC keeps UTC; the clocks show it with the zone the rtc service holds (rtc.wit 1.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Zone(i32);

impl Zone {
    pub const UTC: Zone = Zone(0);
    /// The zones in use run from UTC-12:00 to UTC+14:00.
    pub const WEST: i32 = -12 * 60;
    pub const EAST: i32 = 14 * 60;
    pub fn from_minutes(minutes: i32) -> Option<Zone> { (Self::WEST..=Self::EAST).contains(&minutes).then_some(Zone(minutes)) }
    pub fn minutes(self) -> i32 { self.0 }
    pub fn from_offset(offset: &crate::idl::rtc::Offset) -> Option<Zone> {
        let minutes = i32::try_from(offset.minutes).ok()?;
        Self::from_minutes(if offset.west { -minutes } else { minutes })
    }
    pub fn offset(self) -> crate::idl::rtc::Offset { crate::idl::rtc::Offset { west: self.0 < 0, minutes: self.0.unsigned_abs() } }
    /// `+03:00`, `-05:30`, `+3`, `UTC+3`, `UTC`, `0`: hours and minutes (two digits after a colon), within the zones in use.
    pub fn parse(text: &str) -> Option<Zone> {
        let text = text.trim();
        let text = ["UTC", "utc", "GMT", "gmt"].iter().find_map(|p| text.strip_prefix(p)).unwrap_or(text);
        if text.is_empty() || text == "0" || text == "Z" { return Some(Zone::UTC); }
        let (sign, rest) = match text.as_bytes()[0] { b'+' => (1, &text[1..]), b'-' => (-1, &text[1..]), _ => return None };
        let (hours, minutes) = rest.split_once(':').map_or((rest, None), |(h, m)| (h, Some(m)));
        let digits = |part: &str, most: usize| !part.is_empty() && part.len() <= most && part.bytes().all(|b| b.is_ascii_digit());
        if !digits(hours, 2) || minutes.is_some_and(|m| m.len() != 2 || !digits(m, 2)) { return None; }
        let (hours, minutes): (i32, i32) = (hours.parse().ok()?, minutes.map_or(Some(0), |m| m.parse().ok())?);
        if minutes > 59 { return None; }
        Self::from_minutes(sign * (hours * 60 + minutes))
    }
    /// Seconds since local midnight from seconds since midnight UTC.
    pub fn local_seconds(self, utc: u32) -> u32 { (utc as i64 + self.0 as i64 * 60).rem_euclid(86_400) as u32 }
    /// Local days since 2000-01-01 and seconds since midnight from UTC's; before 2000-01-01, its first second.
    pub fn local(self, days: u32, seconds: u32) -> (u32, u32) { shift(days, seconds, self.0 as i64 * 60).unwrap_or((0, 0)) }
    /// UTC's days and seconds from local ones (`date set`); None before 2000-01-01 UTC.
    pub fn utc(self, days: u32, seconds: u32) -> Option<(u32, u32)> { shift(days, seconds, -(self.0 as i64) * 60) }
}

fn shift(days: u32, seconds: u32, by: i64) -> Option<(u32, u32)> {
    let total = days as i64 * 86_400 + seconds as i64 + by;
    (total >= 0).then(|| ((total / 86_400) as u32, (total % 86_400) as u32))
}

impl core::fmt::Display for Zone {
    /// `UTC`, `UTC+03:00`, `UTC-05:30`.
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.0 == 0 { return f.write_str("UTC"); }
        let minutes = self.0.unsigned_abs();
        write!(f, "UTC{}{:02}:{:02}", if self.0 < 0 { '-' } else { '+' }, minutes / 60, minutes % 60)
    }
}

/// The zone in `data/clock.conf`'s text, which the shell keeps (`date zone`): its first line `zone <offset>`; other
/// lines and `#` comments are left alone.
pub fn configured_zone(text: &str) -> Option<Zone> {
    text.lines().map(str::trim).find_map(|line| line.strip_prefix("zone").filter(|rest| rest.starts_with(' ')).map(str::trim)).and_then(Zone::parse)
}

/// The zone the clocks show the time in (rtc.wit 1.3); UTC when the rtc service does not say.
pub fn zone() -> Zone { crate::idl::rtc::zone(Endpoint::RTC).ok().and_then(|offset| Zone::from_offset(&offset)).unwrap_or(Zone::UTC) }

/// Sets the zone, through a client with the setting badge (the shell's).
pub fn set_zone(zone: Zone) -> Result<(), crate::idl::rtc::Error> {
    crate::idl::rtc::set_zone(Endpoint::RTC, &zone.offset()).map_err(|_| crate::idl::rtc::Error::Unavailable)?
}

/// The local date and time from one reading of the RTC: days since 2000-01-01 and seconds since midnight in the zone.
/// A UTC midnight between the reads is read again.
pub fn local_now() -> Option<(u32, u32)> {
    let first = crate::idl::rtc::now(Endpoint::RTC).ok()??;
    let days = crate::idl::rtc::date(Endpoint::RTC).ok()??;
    let seconds = crate::idl::rtc::now(Endpoint::RTC).ok()??;
    let days = if seconds < first { crate::idl::rtc::date(Endpoint::RTC).ok()?? } else { days };
    (seconds < 86_400).then(|| zone().local(days, seconds))
}

/// Today's local date (year, month 1-12, day 1-31) and seconds since local midnight.
pub fn local_date_time() -> Option<((u32, u32, u32), u32)> { local_now().map(|(days, seconds)| (civil_from_days(days), seconds)) }

/// Today's date in UTC (year, month 1-12, day 1-31) from the RTC.
pub fn date() -> Option<(u32, u32, u32)> {
    let days = crate::idl::rtc::date(Endpoint::RTC).ok()??;
    Some(civil_from_days(days))
}

/// Seconds since 1970-01-01 UTC, the time the RTC keeps (rtc.wit 1.3).
pub fn unix_time() -> Option<u64> {
    let (year, month, day) = date()?;
    let days = days_from_civil(year, month, day)? as u64 + 10957; // 1970-01-01 to 2000-01-01
    Some(days * 86400 + seconds_since_midnight()? as u64)
}

/// The clock's setting from `YYYY-MM-DD HH:MM[:SS]` (`date set`, 211-APP-0042): days since 2000-01-01 and seconds
/// since midnight, in the time zone it is typed in (`Zone::utc` gives the RTC's); None unless the date is real and in 2000..=2099 (what `rtc.wit` 1.2 `set` takes) and the time too.
pub fn parse_setting(text: &str) -> Option<(u32, u32)> {
    let mut words = text.split_whitespace();
    let (date, time) = (words.next()?, words.next()?);
    if words.next().is_some() { return None; }
    let number = |part: &str, digits: usize| (part.len() == digits && part.bytes().all(|b| b.is_ascii_digit())).then(|| part.parse::<u32>().ok()).flatten();
    let mut d = date.split('-');
    let (year, month, day) = (number(d.next()?, 4)?, number(d.next()?, 2)?, number(d.next()?, 2)?);
    if d.next().is_some() || year > 2099 { return None; }
    let mut t = time.split(':');
    let (hour, minute) = (number(t.next()?, 2)?, number(t.next()?, 2)?);
    let second = match t.next() { Some(part) => number(part, 2)?, None => 0 };
    if t.next().is_some() || hour > 23 || minute > 59 || second > 59 { return None; }
    Some((days_from_civil(year, month, day)?, hour * 3600 + minute * 60 + second))
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
