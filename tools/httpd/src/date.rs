//! Times as HTTP writes them (IMF-fixdate) and as the log does (RFC 3339, UTC), without a calendar library.

use std::time::{SystemTime, UNIX_EPOCH};

/// Year, month, day from days since 1970-01-01 (H. Hinnant's civil_from_days).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

fn seconds(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) { Ok(d) => d.as_secs() as i64, Err(e) => -(e.duration().as_secs() as i64) }
}

/// `Tue, 10 Nov 2026 23:09:00 GMT`
pub fn http(t: SystemTime) -> String {
    let s = seconds(t);
    let (days, rest) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    let (y, m, d) = civil(days);
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{}, {:02} {} {} {:02}:{:02}:{:02} GMT", DAYS[days.rem_euclid(7) as usize], d, MONTHS[m as usize - 1], y, rest / 3600, rest / 60 % 60, rest % 60)
}

/// `2026-11-10T23:09:00Z`
pub fn rfc3339(t: SystemTime) -> String {
    let s = seconds(t);
    let (days, rest) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest / 60 % 60, rest % 60)
}

/// `2026-11-10`: the day a log file holds.
pub fn day(t: SystemTime) -> String { rfc3339(t)[..10].to_string() }

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn dates() {
        let at = |s: u64| UNIX_EPOCH + Duration::from_secs(s);
        assert_eq!(http(at(0)), "Thu, 01 Jan 1970 00:00:00 GMT");
        assert_eq!(http(at(784_111_777)), "Sun, 06 Nov 1994 08:49:37 GMT"); // RFC 9110's example
        assert_eq!(rfc3339(at(1_794_352_140)), "2026-11-10T23:09:00Z");
        assert_eq!(http(at(951_782_400)), "Tue, 29 Feb 2000 00:00:00 GMT");
        assert_eq!(day(at(1_794_352_140)), "2026-11-10");
    }
}
