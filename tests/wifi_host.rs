//! Host tests of `wifi` (550-APP-0033): the rows a scan makes, the passphrase's rules, SSIDs as bytes, and the screen's
//! keys against the stand-in of idl/wifi.wit 1.0 (wifi/src/station.rs), as the QEMU test runs it with `--demo`.
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
