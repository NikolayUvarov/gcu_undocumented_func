//! Host test of the calendar helpers in libmind/src/rtc.rs (days since 2000-01-01).
#[allow(dead_code)]
mod ipc { #[derive(Clone, Copy)] pub struct Endpoint(pub usize); impl Endpoint { pub const RTC: Endpoint = Endpoint(2); } }
mod idl { pub mod rtc { pub fn now(_: crate::ipc::Endpoint) -> Result<Option<u32>, ()> { Ok(None) } pub fn date(_: crate::ipc::Endpoint) -> Result<Option<u32>, ()> { Ok(None) } } }
mod time { pub fn monotonic_ns() -> u64 { 0 } }
#[path = "../libmind/src/wallclock.rs"]
#[allow(dead_code)]
mod wallclock;
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

// The CMOS RTC's registers (rtc/src/cmos.rs, 211-KRN-0051): what is written reads back, in every mode.
#[path = "../rtc/src/cmos.rs"]
mod cmos;

#[test]
fn cmos_time_round_trips_in_every_mode() {
    for mode in [0x00u8, 0x02, 0x04, 0x06] { // BCD or binary, 12 or 24 hours
        for seconds in (0..86_400).step_by(7) {
            let [s, m, h] = cmos::encode_time(mode, seconds).unwrap();
            assert_eq!(cmos::decode_time([s, m, h, mode]), Some(seconds as usize), "mode {mode:#x}, {seconds} s");
        }
        assert_eq!(cmos::encode_time(mode, 86_400), None);
    }
    // 13:05:09 in BCD and 12 hours: 1 PM.
    assert_eq!(cmos::encode_time(0x00, 13 * 3600 + 5 * 60 + 9), Some([0x09, 0x05, 0x81]));
    // Midnight in 12 hours is 12 AM.
    assert_eq!(cmos::encode_time(0x00, 0), Some([0, 0, 0x12]));
}

#[test]
fn cmos_date_registers() {
    let days = rtc::days_from_civil(2026, 10, 9).unwrap();
    // A Friday (6 with Sunday 1); BCD day, month and year.
    assert_eq!(cmos::encode_date(0x00, days, (2026, 10, 9)), Some([6, 0x09, 0x10, 0x26]));
    assert_eq!(cmos::encode_date(0x04, days, (2026, 10, 9)), Some([6, 9, 10, 26]));
    assert_eq!(cmos::encode_date(0x00, 0, (2000, 1, 1)), Some([7, 0x01, 0x01, 0x00])); // a Saturday
    assert_eq!(cmos::encode_date(0x00, 36_525, (2100, 1, 1)), None); // two digits of year: 2000-2099 only
}
