//! Host test of the calendar helpers in libmind/src/rtc.rs (days since 2000-01-01).
#[allow(dead_code)]
mod ipc { #[derive(Clone, Copy)] pub struct Endpoint(pub usize); impl Endpoint { pub const RTC: Endpoint = Endpoint(2); } }
// The rtc service as the tests set it: UTC's seconds and date, and the zone (rtc.wit 1.3).
mod idl {
    pub mod rtc {
        use std::sync::atomic::{AtomicI64, Ordering::Relaxed};
        pub static NOW: AtomicI64 = AtomicI64::new(-1);
        pub static DATE: AtomicI64 = AtomicI64::new(-1);
        pub static ZONE: AtomicI64 = AtomicI64::new(0);
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct Offset { pub west: bool, pub minutes: u32 }
        #[allow(dead_code)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Error { Rights, Invalid, Unavailable }
        fn read(cell: &AtomicI64) -> Result<Option<u32>, ()> { let v = cell.load(Relaxed); Ok((v >= 0).then_some(v as u32)) }
        pub fn now(_: crate::ipc::Endpoint) -> Result<Option<u32>, ()> { read(&NOW) }
        pub fn date(_: crate::ipc::Endpoint) -> Result<Option<u32>, ()> { read(&DATE) }
        pub fn zone(_: crate::ipc::Endpoint) -> Result<Offset, ()> { let z = ZONE.load(Relaxed); Ok(Offset { west: z < 0, minutes: z.unsigned_abs() as u32 }) }
        pub fn set_zone(_: crate::ipc::Endpoint, zone: &Offset) -> Result<Result<(), Error>, ()> {
            ZONE.store(if zone.west { -(zone.minutes as i64) } else { zone.minutes as i64 }, Relaxed);
            Ok(Ok(()))
        }
    }
}
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

// 211-APP-0042: `date set YYYY-MM-DD HH:MM[:SS]`, as rtc.wit 1.2 `set` takes it.
#[test]
fn a_setting_is_parsed() {
    assert_eq!(rtc::parse_setting("2026-10-09 14:30"), Some((rtc::days_from_civil(2026, 10, 9).unwrap(), 14 * 3600 + 30 * 60)));
    assert_eq!(rtc::parse_setting(" 2000-01-01   00:00:00 "), Some((0, 0)));
    assert_eq!(rtc::parse_setting("2099-12-31 23:59:59"), Some((rtc::days_from_civil(2099, 12, 31).unwrap(), 86_399)));
    for wrong in ["2100-01-01 00:00", "1999-12-31 23:59", "2026-02-29 10:00", "2026-13-01 10:00", "2026-10-09 24:00", "2026-10-09 10:60", "2026-10-09 10:00:60",
                  "2026-10-09", "26-10-09 10:00", "2026-1-9 10:00", "2026-10-09 10:00 x", "2026/10/09 10:00", "2026-10-09 10.00", "2026-10-09 1:00", ""] {
        assert_eq!(rtc::parse_setting(wrong), None, "{:?}", wrong);
    }
}

// Time zones (000-APP-0099): a fixed offset from UTC, as typed and shown.
#[test]
fn a_zone_is_parsed_and_named() {
    use rtc::Zone;
    for (text, minutes) in [("+03:00", 180), ("+3", 180), ("UTC+3", 180), ("utc+03:00", 180), ("-05:30", -330), ("GMT-5:30", -330),
                            ("UTC", 0), ("0", 0), ("Z", 0), ("+14:00", 840), ("-12:00", -720), ("+05:45", 345)] {
        assert_eq!(Zone::parse(text).map(Zone::minutes), Some(minutes), "{text}");
    }
    for wrong in ["3", "+15", "-12:30", "+3:5", "+03:60", "+003", "++3", "UTC+", "+3:00:00", "Moscow", "+ 3"] {
        assert_eq!(Zone::parse(wrong), None, "{wrong}");
    }
    let named = |minutes| Zone::from_minutes(minutes).unwrap().to_string();
    assert_eq!((named(0), named(180), named(-330), named(345)), ("UTC".into(), "UTC+03:00".into(), "UTC-05:30".into(), "UTC+05:45".into()));
    for minutes in Zone::WEST..=Zone::EAST {
        let zone = Zone::from_minutes(minutes).unwrap();
        assert_eq!(Zone::parse(&zone.to_string()), Some(zone));
        assert_eq!(Zone::from_offset(&zone.offset()), Some(zone));
    }
    assert_eq!(Zone::from_offset(&idl::rtc::Offset { west: false, minutes: 841 }), None);
    // data/clock.conf, as the shell writes it and as a person might.
    let written = format!("# The time zone the clocks show the time in; the RTC keeps UTC (date zone).\nzone {}\n", Zone::parse("+03:00").unwrap());
    assert_eq!(rtc::configured_zone(&written).map(Zone::minutes), Some(180));
    assert_eq!(rtc::configured_zone("  zone   -05:30  \r\nzone +01:00\n").map(Zone::minutes), Some(-330), "the first line counts");
    for wrong in ["", "# zone +03:00\n", "zones +03:00", "zone\n", "zone Moscow\n"] { assert_eq!(rtc::configured_zone(wrong), None, "{wrong:?}"); }
}

#[test]
fn local_time_crosses_midnight_both_ways() {
    use rtc::Zone;
    let (east, west) = (Zone::parse("+03:00").unwrap(), Zone::parse("-05:30").unwrap());
    let day = rtc::days_from_civil(2026, 10, 21).unwrap();
    assert_eq!(east.local(day, 22 * 3600 + 30 * 60), (day + 1, 3600 + 30 * 60), "22:30 UTC is 01:30 the next day at UTC+3");
    assert_eq!(west.local(day, 2 * 3600), (day - 1, 20 * 3600 + 30 * 60), "02:00 UTC is 20:30 the day before at UTC-5:30");
    assert_eq!(east.local_seconds(22 * 3600 + 30 * 60), 3600 + 30 * 60);
    assert_eq!(west.local_seconds(2 * 3600), 20 * 3600 + 30 * 60);
    for (days, seconds) in [(day, 0), (day, 86_399), (day + 400, 12_345)] {
        for zone in [east, west, Zone::UTC] {
            let (d, s) = zone.local(days, seconds);
            assert_eq!(zone.utc(d, s), Some((days, seconds)), "{zone} {days} {seconds}");
        }
    }
    assert_eq!(east.utc(0, 3600), None, "01:00 on 2000-01-01 at UTC+3 is before the RTC's first day");
    assert_eq!(west.local(0, 0), (0, 0));
}

#[test]
fn a_clock_face_shows_the_zones_time() {
    use std::sync::atomic::Ordering::Relaxed;
    let day = rtc::days_from_civil(2026, 10, 21).unwrap() as i64;
    idl::rtc::DATE.store(day, Relaxed);
    idl::rtc::NOW.store(22 * 3600 + 30 * 60, Relaxed);
    idl::rtc::ZONE.store(180, Relaxed);
    let mut clock = rtc::Clock::new();
    assert_eq!(clock.seconds_since_midnight(), Some(3600 + 30 * 60));
    assert_eq!(clock.date(), Some((2026, 10, 22)));
    assert_eq!(clock.zone().to_string(), "UTC+03:00");
    assert_eq!(rtc::local_date_time(), Some(((2026, 10, 22), 3600 + 30 * 60)));
    rtc::set_zone(rtc::Zone::parse("-05:30").unwrap()).unwrap();
    assert_eq!(rtc::zone().minutes(), -330);
    assert_eq!(rtc::local_date_time(), Some(((2026, 10, 21), 17 * 3600)));
    idl::rtc::ZONE.store(0, Relaxed);
}
