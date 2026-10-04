//! Host test of the calendar helpers in libmind/src/rtc.rs (days since 2000-01-01).
#[allow(dead_code)]
mod ipc { #[derive(Clone, Copy)] pub struct Endpoint(pub usize); impl Endpoint { pub const RTC: Endpoint = Endpoint(2); } }
mod idl { pub mod rtc { pub fn now(_: crate::ipc::Endpoint) -> Result<Option<u32>, ()> { Ok(None) } pub fn date(_: crate::ipc::Endpoint) -> Result<Option<u32>, ()> { Ok(None) } } }
#[path = "../libmind/src/rtc.rs"]
#[allow(dead_code)]
mod rtc;

#[test]
fn days_round_trip() {
    assert_eq!(rtc::days_from_civil(2000, 1, 1), Some(0));
    assert_eq!(rtc::days_from_civil(2000, 3, 1), Some(60), "2000 is a leap year");
    assert_eq!(rtc::days_from_civil(2026, 10, 4), Some(9773));
    assert_eq!(rtc::days_from_civil(2100, 2, 29), None, "2100 is not a leap year");
    assert_eq!(rtc::days_from_civil(2026, 13, 1), None);
    for days in 0..40_000 {
        let (y, m, d) = rtc::civil_from_days(days);
        assert_eq!(rtc::days_from_civil(y, m, d), Some(days));
    }
}
