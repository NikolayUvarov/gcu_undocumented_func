//! Host tests of `wifi` (550-APP-0033): the rows a scan makes, the passphrase's rules, SSIDs as bytes, and the screen's
//! keys against the stand-in of idl/wifi.wit 1.0 (wifi/src/station.rs), as the QEMU test runs it with `--demo`; `wifi join`,
//! `status` and `leave` without a screen (wifi/src/headless.rs, 550-APP-0078); the country (wifi.wit 1.1, 550-APP-0082).
#![allow(dead_code)]
extern crate alloc;
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/keys.rs"]
mod keys;
#[path = "../libmind/src/util.rs"]
mod util;
#[path = "../libmind/src/tui/mod.rs"]
mod tui;
#[path = "../wifi/src/station.rs"]
mod station;
#[path = "../wifi/src/view.rs"]
mod view;
#[path = "../wifi/src/headless.rs"]
mod headless;
#[path = "../wifi/src/countries.rs"]
mod countries;
use keys::Key;
use station::*;
use view::*;

fn key(code: u16) -> Key { Key(keys::event(code, 0, 0)) }
fn char_key(c: char) -> Key { Key(keys::event(0, c as u32, 0)) }
fn typed(w: &mut Wifi, s: &mut dyn Station, text: &str) { for c in text.chars() { w.key(char_key(c), s); } }

fn screen(w: &Wifi) -> Vec<String> {
    let mut cells = vec![tui::Cell::BLANK; 100 * 30];
    let mut grid = tui::Grid::new(&mut cells, 100, 30);
    w.draw(&mut grid, &tui::DARK);
    (0..30).map(|y| (0..100).map(|x| grid.get(x, y).ch).collect::<String>()).collect()
}

#[test]
fn a_scan_makes_a_row_a_name_the_strongest_first() {
    let found = Demo::new().scan().unwrap();
    let rows = rows(&found);
    let names: Vec<String> = rows.iter().map(|r| shown(&r.ssid)).collect();
    // MIND-Lab's two access points are one row with the stronger's signal; the hidden network last.
    assert_eq!(names, ["MIND-Lab", "Home", "Cafe guests", "Office", "Caf\\xE9 TKIP", "Old router", "Neighbour", "Far away", ""]);
    assert_eq!((rows[0].weakness, rows[0].channel, rows[0].points), (48, 6, 2));
    assert!(rows[1].remembered && !rows[0].remembered);
    assert_eq!([48, 60, 71, 80, 88, 92, 0].map(bars), [4, 3, 2, 1, 0, 0, 0], "0: unknown");
    // An unknown signal goes after every known one.
    let mut quiet = found.clone();
    quiet[0].weakness = 0;
    quiet[1].weakness = 0;
    assert_eq!(rows_names(&quiet).last().map(String::as_str), Some("MIND-Lab"));
}

fn rows_names(found: &[Network]) -> Vec<String> { rows(found).iter().filter(|r| !r.ssid.is_empty()).map(|r| shown(&r.ssid)).collect() }

#[test]
fn an_ssid_is_bytes_shown_exactly() {
    assert_eq!(shown(b"MIND-Lab"), "MIND-Lab");
    assert_eq!(shown("Кафе".as_bytes()), "Кафе", "UTF-8 as it is");
    assert_eq!(shown(b"Caf\xE9"), "Caf\\xE9", "Latin-1: the byte, not a guess");
    assert_eq!(shown(b"\xC4\xE3\xBA\xC3"), "\\xC4\\xE3\\xBA\\xC3", "GBK");
    assert_eq!(shown(b"a\nb\x00"), "a\\x0Ab\\x00", "control characters");
    assert_eq!(shown(b""), "");
    assert_eq!(mac(0x024D_494E_4401), "02:4D:49:4E:44:01");
    assert_eq!((rate(108), rate(11), rate(0)), ("54".into(), "5.5".into(), "0".into()));
}

#[test]
fn the_passphrase_follows_the_protection() {
    assert!(check(Security::Wpa2, "short").is_err());
    assert!(check(Security::Wpa2, "correct horse").is_ok());
    assert!(check(Security::Wpa2, &"a".repeat(63)).is_ok());
    assert!(check(Security::Wpa2, &"a".repeat(64)).is_ok(), "64 hexadecimal digits: the PSK itself");
    assert!(check(Security::Wpa2, &"g".repeat(64)).is_err());
    assert!(check(Security::Wpa2, "пароль-пароль").is_err(), "printable ASCII only");
    assert!(check(Security::Open, "").is_ok());
    // wifi.wit 1.0: the station joins open and WPA2 networks only.
    for other in [Security::Wep, Security::Wpa3, Security::Enterprise, Security::Other] { assert!(check(other, "anything at all").is_err(), "{:?}", other); }
    assert!(Refusal::NotFound.text().contains("r scans again"));
}

#[test]
fn a_window_is_not_titled_wifi_twice() {
    // wm's title bar names the program: in a window the frame names only the demo (the maintainer's report, 2026-10-21).
    let mut demo = Demo::new();
    let mut w = Wifi::new(false);
    w.refresh(&mut demo);
    assert!(screen(&w)[0].contains(" wifi "), "on a screen of its own");
    w.windowed = true;
    assert!(!screen(&w)[0].contains("wifi"), "{}", screen(&w)[0]);
    let mut shown = Wifi::new(true);
    shown.windowed = true;
    shown.refresh(&mut demo);
    assert!(screen(&shown)[0].contains("DEMO") && !screen(&shown)[0].contains("wifi"), "{}", screen(&shown)[0]);
}

#[test]
fn a_protected_network_is_joined_with_its_passphrase_unseen() {
    let mut demo = Demo::new();
    let mut w = Wifi::new(true);
    w.refresh(&mut demo);
    assert!(screen(&w)[0].contains("DEMO"));
    assert!(w.status().starts_with("MODE=LIST ROWS=9 AT=MIND-Lab LINK=OFF"), "{}", w.status());
    // Enter on MIND-Lab asks for its passphrase; what is typed shows as dots, never as text, nor in the status line.
    w.key(key(abi::KEY_ENTER), &mut demo);
    assert!(w.status().starts_with("MODE=PASSPHRASE MIND-Lab REMEMBER=true"), "{}", w.status());
    typed(&mut w, &mut demo, "short");
    w.key(key(abi::KEY_ENTER), &mut demo);
    assert!(screen(&w).iter().any(|row| row.contains("8 to 63 characters")), "too short: said, still asking");
    for _ in 0..5 { w.key(key(abi::KEY_BACKSPACE), &mut demo); }
    typed(&mut w, &mut demo, DEMO_PASSPHRASE);
    let shown_now = screen(&w);
    assert!(shown_now.iter().any(|row| row.contains(&"•".repeat(DEMO_PASSPHRASE.len()))), "dots");
    assert!(!shown_now.iter().any(|row| row.contains(DEMO_PASSPHRASE)) && !w.status().contains("horse"));
    w.key(key(abi::KEY_ENTER), &mut demo);
    assert!(w.status().contains("LINK=JOINING MIND-Lab"), "{}", w.status());
    w.poll(&mut demo);
    // Joined: the access point, the signal, the channel and the rate; the address is the network stack's.
    assert!(w.status().contains("LINK=JOINED MIND-Lab 02:4D:49:4E:44:01"), "{}", w.status());
    assert_eq!(w.notice, "Joined MIND-Lab");
    let shown_now = screen(&w);
    assert!(shown_now[1].contains("Joined MIND-Lab: -48 dBm, channel 6, 54 Mb/s, access point 02:4D:49:4E:44:01"), "{}", shown_now[1]);
    assert!(shown_now[2].contains("ip shows it"), "{}", shown_now[2]);
    // Remembered now: the row says so; forgetting it leaves the network.
    assert!(w.rows[0].remembered);
    w.key(char_key('f'), &mut demo);
    assert!(w.notice.contains("MIND-Lab forgotten") && !w.rows[0].remembered && w.status().contains("LINK=OFF"), "{} / {}", w.notice, w.status());
}

#[test]
fn a_station_that_does_not_remember_joins_without_keeping_the_key() {
    // The station answers `remember` with unsupported until it keeps keys in keystore (550-NET-0018): wifi joins
    // without it and says so.
    let mut demo = Demo::new();
    demo.remembers = false;
    let mut w = Wifi::new(true);
    w.refresh(&mut demo);
    w.key(key(abi::KEY_ENTER), &mut demo);
    typed(&mut w, &mut demo, DEMO_PASSPHRASE);
    w.key(key(abi::KEY_ENTER), &mut demo);
    assert!(w.status().contains("LINK=JOINING MIND-Lab"), "{} / {}", w.status(), w.notice);
    w.poll(&mut demo);
    assert!(w.status().contains("LINK=JOINED MIND-Lab"), "{}", w.status());
    assert!(w.notice.contains("does not remember networks yet"), "{}", w.notice);
    assert!(!w.rows[0].remembered);
}

#[test]
fn a_wrong_passphrase_an_open_network_and_the_refusals() {
    let mut demo = Demo::new();
    let mut w = Wifi::new(true);
    w.refresh(&mut demo);
    w.key(key(abi::KEY_ENTER), &mut demo);
    w.key(key(abi::KEY_TAB), &mut demo); // not remembered
    typed(&mut w, &mut demo, "wrong horse");
    w.key(key(abi::KEY_ENTER), &mut demo);
    w.poll(&mut demo);
    assert!(w.status().contains("LINK=FAILED MIND-Lab WrongPassphrase"), "{}", w.status());
    assert!(screen(&w).iter().any(|row| row.contains("the passphrase is wrong")));
    // A remembered network joins without a question; an open one too.
    w.key(key(abi::KEY_DOWN), &mut demo);
    w.key(key(abi::KEY_ENTER), &mut demo);
    w.poll(&mut demo);
    assert!(w.status().contains("LINK=JOINED Home 02:48:4F:4D:45:03"), "{}", w.status());
    w.key(key(abi::KEY_DOWN), &mut demo);
    w.key(key(abi::KEY_ENTER), &mut demo);
    w.poll(&mut demo);
    assert!(w.status().contains("LINK=JOINED Cafe guests 02:43:41:46:45:04"), "{}", w.status());
    assert!(screen(&w)[1].contains("5.5 Mb/s"));
    // Enterprise, TKIP, WEP, WPA3 and a hidden network say why they are not joined.
    for (downs, said) in [(1, "802.1X"), (1, "TKIP"), (1, "WEP is broken"), (1, "WPA3"), (2, "hidden network")] {
        for _ in 0..downs { w.key(key(abi::KEY_DOWN), &mut demo); }
        w.key(key(abi::KEY_ENTER), &mut demo);
        assert!(w.notice.contains(said), "{}", w.notice);
        assert!(w.status().starts_with("MODE=LIST"));
    }
    // A name that is not UTF-8 is listed by its bytes.
    assert!(screen(&w).iter().any(|row| row.contains("Caf\\xE9 TKIP")));
    // d leaves; Esc in the passphrase goes back to the list; q ends.
    w.key(char_key('d'), &mut demo);
    assert!(w.status().contains("LINK=OFF"));
    for _ in 0..9 { w.key(key(abi::KEY_UP), &mut demo); }
    w.key(key(abi::KEY_ENTER), &mut demo);
    typed(&mut w, &mut demo, "abc");
    assert!(matches!(w.key(key(abi::KEY_ESC), &mut demo), Flow::Continue) && w.status().starts_with("MODE=LIST"));
    assert!(matches!(w.key(char_key('q'), &mut demo), Flow::Quit));
}

#[test]
fn the_stand_in_answers_as_wifi_wit_says() {
    let mut demo = Demo::new();
    assert_eq!(demo.join(b"Nowhere", "", false), Err(Refusal::NotFound));
    assert_eq!(demo.join(b"Neighbour", "a password", false), Err(Refusal::Unsupported), "WPA3");
    assert_eq!(demo.join(b"MIND-Lab", "", false), Err(Refusal::Invalid), "WPA2 without a passphrase");
    assert_eq!(demo.join(b"MIND-Lab", "short", false), Err(Refusal::Invalid));
    assert_eq!(demo.join(b"Home", "", false), Ok(()), "remembered");
}

#[test]
fn without_a_station_the_screen_says_why() {
    let mut absent = Absent(String::from("no station here"));
    let mut w = Wifi::new(false);
    w.refresh(&mut absent);
    assert_eq!(w.problem.as_deref(), Some("no station here"));
    let shown_now = screen(&w);
    assert!(shown_now.iter().any(|row| row.contains("No Wi-Fi station")) && shown_now.iter().any(|row| row.contains("wifi --demo")));
    assert!(matches!(w.key(key(abi::KEY_ENTER), &mut absent), Flow::Quit));
}

// A clock that moves only when the join pauses.
fn join_with(station: &mut dyn Station, ssid: &[u8], passphrase: &str, remember: bool) -> (u32, String, u64) {
    let clock = std::cell::Cell::new(0u64);
    let (code, line) = headless::join(station, ssid, passphrase, remember, &mut || clock.get(), &mut |ms| clock.set(clock.get() + ms));
    (code, line, clock.get())
}

#[test]
fn the_command_line_without_a_screen() {
    use headless::{parse, Command};
    assert_eq!(parse(""), Ok(None));
    assert_eq!(parse("--demo"), Ok(None));
    assert_eq!(parse("status"), Ok(Some(Command::Status)));
    assert_eq!(parse("--demo leave"), Ok(Some(Command::Leave)));
    assert_eq!(parse("join MIND-TEST --passphrase-file /data/wifijoin.txt"),
               Ok(Some(Command::Join { ssid: b"MIND-TEST".to_vec(), file: Some("/data/wifijoin.txt".into()), remember: false })));
    assert_eq!(parse("join \"Cafe guests\" --remember"), Ok(Some(Command::Join { ssid: b"Cafe guests".to_vec(), file: None, remember: true })));
    for wrong in ["join", "join --remember", "join A --passphrase-file", "join A --passphrase-file f --passphrase-file g", "join A B", "scan", "status now"] {
        assert_eq!(parse(wrong), Err(headless::USAGE), "{wrong}");
    }
}

#[test]
fn the_passphrase_file_names_its_network_first() {
    assert_eq!(headless::passphrase(b"MIND-TEST\nmind test passphrase\n", b"MIND-TEST").as_deref(), Ok("mind test passphrase"));
    assert_eq!(headless::passphrase(b"MIND-TEST\r\nmind test passphrase\r\n", b"MIND-TEST").as_deref(), Ok("mind test passphrase"));
    let other = headless::passphrase(b"Home\nsecret words\n", b"MIND-TEST");
    assert!(other.is_err() && !other.unwrap_err().contains("secret"), "the error never holds the file's lines");
    assert!(headless::passphrase(b"MIND-TEST\n", b"MIND-TEST").is_err());
    assert!(headless::passphrase(b"MIND-TEST\n\xFF\xFE\n", b"MIND-TEST").is_err());
}

#[test]
fn a_join_without_a_screen_reports_its_end_and_never_the_passphrase() {
    let mut demo = Demo::new();
    let (code, line, _) = join_with(&mut demo, b"MIND-Lab", DEMO_PASSPHRASE, false);
    assert_eq!((code, line.as_str()), (headless::JOINED, "[WIFI] JOINED MIND-Lab 02:4D:49:4E:44:01 CHANNEL 6 54 MB/S"));
    assert_eq!(headless::status(&mut demo), (headless::JOINED, String::from("[WIFI] STATUS JOINED MIND-Lab 02:4D:49:4E:44:01 CHANNEL 6 54 MB/S -48 DBM")));
    assert_eq!(headless::leave(&mut demo), (headless::JOINED, String::from("[WIFI] LEFT")));
    assert_eq!(headless::status(&mut demo).1, "[WIFI] STATUS OFF");

    let (code, line, _) = join_with(&mut Demo::new(), b"MIND-Lab", "wrong horse", false);
    assert_eq!(code, headless::REFUSED);
    assert!(line.starts_with("[WIFI] REFUSED MIND-Lab: the passphrase is wrong") && !line.contains("horse"), "{line}");
    // A network no scan hears: one scan, then the refusal.
    let mut demo = Demo::new();
    let (code, line, _) = join_with(&mut demo, b"Nowhere", DEMO_PASSPHRASE, false);
    assert_eq!((code, line.as_str()), (headless::REFUSED, "[WIFI] THE LAST SCAN DID NOT HEAR Nowhere: SCANNED, 10 ACCESS POINTS, NOT IT\n[WIFI] REFUSED Nowhere: no network of that name was heard"));
    assert_eq!(demo.scans, 1);
    // A station that does not remember networks is asked again without.
    let mut forgetful = Demo::new();
    forgetful.remembers = false;
    let (code, line, _) = join_with(&mut forgetful, b"MIND-Lab", DEMO_PASSPHRASE, true);
    assert_eq!(code, headless::JOINED);
    assert!(line.ends_with(", NOT REMEMBERED: THE STATION KEEPS NO NETWORKS"), "{line}");
    // No station at all.
    let (code, line, _) = join_with(&mut Absent(String::from("no radio")), b"MIND-Lab", DEMO_PASSPHRASE, false);
    assert_eq!((code, line.as_str()), (headless::NO_STATION, "[WIFI] NO STATION: no radio"));
}

// A station that starts joining and never ends.
struct Stuck;
impl Station for Stuck {
    fn scan(&mut self) -> Result<Vec<Network>, Refusal> { Ok(Vec::new()) }
    fn join(&mut self, _: &[u8], _: &str, _: bool) -> Result<(), Refusal> { Ok(()) }
    fn status(&mut self) -> Result<Link, Refusal> { Ok(Link::Joining { ssid: b"Slow".to_vec() }) }
    fn leave(&mut self) -> Result<(), Refusal> { Ok(()) }
    fn forget(&mut self, _: &[u8]) -> Result<bool, Refusal> { Ok(false) }
}

// The station as it is after a boot (550-APP-0098): it joins only what its last scan heard, and nothing has scanned.
// `late`: it takes the join and fails it in `status`, as the MacBook Pro's did; else it refuses the join.
struct Unscanned { demo: Demo, scanned: bool, late: bool, failed: Option<Vec<u8>> }
impl Station for Unscanned {
    fn scan(&mut self) -> Result<Vec<Network>, Refusal> { self.scanned = true; self.demo.scan() }
    fn join(&mut self, ssid: &[u8], passphrase: &str, remember: bool) -> Result<(), Refusal> {
        if self.scanned { return self.demo.join(ssid, passphrase, remember); }
        if !self.late { return Err(Refusal::NotFound); }
        self.failed = Some(ssid.to_vec());
        Ok(())
    }
    fn status(&mut self) -> Result<Link, Refusal> {
        match &self.failed { Some(ssid) if !self.scanned => Ok(Link::Failed { ssid: ssid.clone(), reason: Reason::NotFound }), _ => self.demo.status() }
    }
    fn leave(&mut self) -> Result<(), Refusal> { self.demo.leave() }
    fn forget(&mut self, ssid: &[u8]) -> Result<bool, Refusal> { self.demo.forget(ssid) }
}

#[test]
fn a_join_before_any_scan_scans_once_then_joins() {
    for late in [false, true] {
        let mut station = Unscanned { demo: Demo::new(), scanned: false, late, failed: None };
        let (code, line, _) = join_with(&mut station, b"Home", "", false);
        assert_eq!((code, line.as_str()), (headless::JOINED, "[WIFI] THE LAST SCAN DID NOT HEAR Home: SCANNED, 10 ACCESS POINTS, IT AMONG THEM\n[WIFI] JOINED Home 02:48:4F:4D:45:03 CHANNEL 1 5.5 MB/S"), "late: {late}");
        assert_eq!(station.demo.scans, 1, "late: {late}");
    }
}

#[test]
fn a_join_that_never_ends_gives_no_answer_after_a_minute() {
    let (code, line, waited) = join_with(&mut Stuck, b"Slow", "", false);
    assert_eq!((code, line.as_str()), (headless::NO_ANSWER, "[WIFI] NO ANSWER Slow IN 60 S"));
    assert_eq!(waited, headless::JOIN_MS);
}

#[test]
fn the_country_is_asked_for_applied_and_read_back_by_its_code() {
    // 550-APP-0082 against the stand-in of wifi.wit 1.1: the command lines.
    use headless::{parse, Command};
    assert_eq!(parse("country"), Ok(Some(Command::Country(None))));
    assert_eq!(parse("country uz"), Ok(Some(Command::Country(Some("UZ".into())))));
    assert_eq!(parse("countries Uzb"), Ok(Some(Command::Countries(Some("uzb".into())))));
    assert_eq!(parse("channels 00"), Ok(Some(Command::Channels(Some("00".into())))));
    assert_eq!(parse("channels"), Ok(Some(Command::Channels(None))));
    for wrong in ["country usa", "country u", "country u z", "channels uz de"] { assert_eq!(parse(wrong), Err(headless::USAGE), "{}", wrong); }
    // The names are the tool's: the catalogue has codes only.
    assert_eq!((countries::name("UZ"), countries::name("DE"), countries::name("00"), countries::name("ZZ")), (Some("Uzbekistan"), Some("Germany"), None, None));
    assert!(countries::NAMES.windows(2).all(|w| w[0].0 < w[1].0) && countries::NAMES.len() == 249);
    // None asked for: said so. Asked for and applied, read back, kept; the radio confirmed its limits.
    let mut demo = Demo::new();
    let (code, line) = headless::country(&mut demo, None);
    assert_eq!(code, headless::JOINED);
    assert!(line.starts_with("[WIFI] COUNTRY NONE ASKED: THE STATION PROBES NOWHERE") && line.ends_with("; CATALOGUE 0123456789abcdef"), "{line}");
    let (code, line) = headless::country(&mut demo, Some("UZ"));
    assert_eq!((code, line.as_str()), (headless::JOINED, "[WIFI] COUNTRY UZ (Uzbekistan) APPLIED, THE RADIO CONFIRMED ITS LIMITS, KEPT; CATALOGUE 0123456789abcdef"));
    assert_eq!(headless::country(&mut demo, None).1, line, "read back");
    // A radio that refuses the limits: failed, nothing applied; not a country of the catalogue: refused.
    let (code, line) = headless::country(&mut demo, Some("XX"));
    assert_eq!((code, line.as_str()), (headless::REFUSED, "[WIFI] COUNTRY XX ASKED, FAILED: THE RADIO REFUSED ITS LIMITS, NONE APPLIED, NOT KEPT ACROSS RESTARTS; CATALOGUE 0123456789abcdef"));
    assert_eq!(headless::country(&mut demo, Some("ZZ")), (headless::REFUSED, String::from("[WIFI] COUNTRY ZZ NOT SET: NOT A COUNTRY OF THE CATALOGUE")));
    assert_eq!(headless::country(&mut demo, Some("00")).0, headless::REFUSED, "the world domain is never chosen");
    // Joined, a change leaves the network first.
    demo.join(b"Home", "", false).unwrap();
    demo.status().unwrap();
    demo.status().unwrap();
    assert!(matches!(demo.status().unwrap(), Link::Joined { .. }));
    headless::country(&mut demo, Some("DE"));
    assert_eq!(demo.status().unwrap(), Link::Off);
}

#[test]
fn the_catalogue_and_a_countrys_channels_are_listed() {
    let mut demo = Demo::new();
    let (code, text) = headless::countries(&mut demo, None);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!((code, lines.len(), *lines.last().unwrap()), (headless::JOINED, 6, "[WIFI] 5 OF 5 COUNTRIES"));
    assert!(lines[3].starts_with("UZ Uzbekistan") && lines[3].ends_with("SEND 14 LISTEN  1"), "{}", lines[3]);
    assert!(lines[4].starts_with("XX ") && lines[4].ends_with("SEND  0 LISTEN  0: NO CHANNEL OF THIS RADIO"), "{}", lines[4]);
    // Found by its name or its code, in any case.
    let (_, found) = headless::countries(&mut demo, Some("uzb"));
    assert_eq!(found.lines().count(), 2);
    assert!(found.ends_with("[WIFI] 1 OF 5 COUNTRIES HOLD \"uzb\""), "{found}");
    assert_eq!(headless::countries(&mut demo, Some("jp")).1.lines().next().unwrap().get(..8), Some("JP Japan"));
    // The channels under a country, under the one applied, or under the world domain when none is.
    let (_, world) = headless::channels(&mut demo, None);
    assert!(world.ends_with("[WIFI] 16 CHANNELS UNDER 00 (the world domain)") && world.contains("CHANNEL  12 2.4 GHZ LISTEN ONLY 20 DBM"), "{world}");
    headless::country(&mut demo, Some("US"));
    let (_, us) = headless::channels(&mut demo, None);
    assert!(us.ends_with("UNDER US (United States)") && us.contains("CHANNEL  14 2.4 GHZ BARRED\n") && us.contains("CHANNEL  36 5 GHZ SEND 23 DBM"), "{us}");
    assert!(headless::channels(&mut demo, Some("JP")).1.contains("CHANNEL  52 5 GHZ NOT THIS RADIO 20 DBM"));
    assert_eq!(headless::channels(&mut demo, Some("ZZ")).1, "[WIFI] CHANNELS OF ZZ: NOT A COUNTRY OF THE CATALOGUE");
}

#[test]
fn a_station_without_a_catalogue_or_without_the_change_badge_says_so() {
    let mut old = Demo::new();
    old.catalogue = false;
    assert_eq!(headless::country(&mut old, None), (headless::JOINED, String::from("[WIFI] COUNTRY: NO CATALOGUE IN THIS STATION (IT RUNS AS WIFI.WIT 1.0 DID)")));
    assert_eq!(headless::country(&mut old, Some("UZ")).1, "[WIFI] COUNTRY UZ NOT SET: NO CATALOGUE IN THIS STATION (IT RUNS AS WIFI.WIT 1.0 DID)");
    assert_eq!(headless::countries(&mut old, None), (headless::REFUSED, String::from("[WIFI] COUNTRIES: NO CATALOGUE IN THIS STATION (IT RUNS AS WIFI.WIT 1.0 DID)")));
    assert_eq!(headless::channels(&mut old, None).0, headless::REFUSED);
    // A test station of 1.0's calls alone answers as a station without a catalogue does.
    assert_eq!(headless::countries(&mut Stuck, None).0, headless::REFUSED);
    // A read-only client.
    struct ReadOnly(Demo);
    impl Station for ReadOnly {
        fn scan(&mut self) -> Result<Vec<Network>, Refusal> { self.0.scan() }
        fn join(&mut self, _: &[u8], _: &str, _: bool) -> Result<(), Refusal> { Err(Refusal::Denied) }
        fn status(&mut self) -> Result<Link, Refusal> { self.0.status() }
        fn leave(&mut self) -> Result<(), Refusal> { Err(Refusal::Denied) }
        fn forget(&mut self, _: &[u8]) -> Result<bool, Refusal> { Err(Refusal::Denied) }
        fn set_country(&mut self, _: &str) -> Result<CountryStatus, Refusal> { Err(Refusal::Denied) }
    }
    assert_eq!(headless::country(&mut ReadOnly(Demo::new()), Some("UZ")), (headless::REFUSED, String::from("[WIFI] COUNTRY UZ NOT SET: WIFI'S CLIENT OF THE STATION ONLY READS: THE COUNTRY NEEDS THE CHANGE BADGE")));
    assert_eq!(headless::country(&mut Absent(String::from("no radio")), None), (headless::NO_STATION, String::from("[WIFI] NO STATION: no radio")));
}
