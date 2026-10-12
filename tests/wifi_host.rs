//! Host tests of `wifi` (550-APP-0033): the rows a scan makes, the passphrase's rules, and the screen's keys against
//! the stand-in station (wifi/src/station.rs), as the QEMU test runs it with `--demo`.
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
    let names: Vec<&str> = rows.iter().map(|r| r.ssid.as_str()).collect();
    // MIND-Lab's two access points are one row with the stronger's signal; the hidden network last.
    assert_eq!(names, ["MIND-Lab", "Home-WPA3", "Cafe guests", "Office", "Old router", "Far away", ""]);
    assert_eq!((rows[0].signal, rows[0].channel, rows[0].points), (-48, 6, 2));
    assert!(rows[1].remembered && !rows[0].remembered);
    assert_eq!([-48, -60, -71, -80, -88, -92].map(bars), [4, 3, 2, 1, 0, 0]);
}

#[test]
fn the_passphrase_follows_the_protection() {
    assert!(check(Security::Wpa2, "short").is_err());
    assert!(check(Security::Wpa2, "correct horse").is_ok());
    assert!(check(Security::Wpa2, &"a".repeat(63)).is_ok());
    assert!(check(Security::Wpa2, &"a".repeat(64)).is_ok(), "64 hexadecimal digits: the PSK itself");
    assert!(check(Security::Wpa2, &"g".repeat(64)).is_err());
    assert!(check(Security::Wpa2, "пароль-пароль").is_err(), "printable ASCII only");
    assert!(check(Security::Wpa3, "x").is_ok());
    assert!(check(Security::Wpa3, &"x".repeat(129)).is_err());
    assert!(check(Security::Open, "").is_ok());
    assert!(check(Security::Wep, "anything").is_err() && check(Security::Enterprise, "anything").is_err());
}

#[test]
fn a_protected_network_is_joined_with_its_passphrase_unseen() {
    let mut demo = Demo::new();
    let mut w = Wifi::new(true);
    w.refresh(&mut demo);
    assert!(screen(&w)[0].contains("DEMO"));
    assert!(w.status().starts_with("MODE=LIST ROWS=7 AT=MIND-Lab LINK=OFF"), "{}", w.status());
    // Enter on MIND-Lab asks for its passphrase; what is typed shows as dots, never as text, nor in the status line.
    w.key(key(abi::KEY_ENTER), &mut demo);
    assert!(w.status().starts_with("MODE=PASSPHRASE MIND-Lab REMEMBER=true"), "{}", w.status());
    typed(&mut w, &mut demo, "short");
    w.key(key(abi::KEY_ENTER), &mut demo);
    assert!(screen(&w).iter().any(|row| row.contains("8 to 63 characters")), "too short: said, still asking");
    for _ in 0..5 { w.key(key(abi::KEY_BACKSPACE), &mut demo); }
    typed(&mut w, &mut demo, DEMO_PASSPHRASE);
    let shown = screen(&w);
    assert!(shown.iter().any(|row| row.contains(&"•".repeat(DEMO_PASSPHRASE.len()))), "dots");
    assert!(!shown.iter().any(|row| row.contains(DEMO_PASSPHRASE)) && !w.status().contains("horse"));
    w.key(key(abi::KEY_ENTER), &mut demo);
    assert!(w.status().contains("LINK=JOINING MIND-Lab"), "{}", w.status());
    w.poll(&mut demo);
    assert!(w.status().contains("LINK=JOINED MIND-Lab 10.0.0.42"), "{}", w.status());
    assert!(w.notice.contains("Joined MIND-Lab: 10.0.0.42"));
    // Remembered now: the row says so; forgetting it leaves the network.
    assert!(w.rows[0].remembered);
    w.key(char_key('f'), &mut demo);
    assert!(w.notice.contains("MIND-Lab forgotten") && !w.rows[0].remembered && w.status().contains("LINK=OFF"), "{} / {}", w.notice, w.status());
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
    assert!(w.status().contains("LINK=JOINED Home-WPA3"), "{}", w.status());
    w.key(key(abi::KEY_DOWN), &mut demo);
    w.key(key(abi::KEY_ENTER), &mut demo);
    w.poll(&mut demo);
    assert!(w.status().contains("LINK=JOINED Cafe guests 192.168.4.23"), "{}", w.status());
    // Enterprise, WEP and a hidden network say why they are not joined.
    for (downs, said) in [(1, "802.1X"), (1, "WEP is broken"), (2, "hidden network")] {
        for _ in 0..downs { w.key(key(abi::KEY_DOWN), &mut demo); }
        w.key(key(abi::KEY_ENTER), &mut demo);
        assert!(w.notice.contains(said), "{}", w.notice);
        assert!(w.status().starts_with("MODE=LIST"));
    }
    // d leaves; Esc in the passphrase goes back to the list; q ends.
    w.key(char_key('d'), &mut demo);
    assert!(w.status().contains("LINK=OFF"));
    w.key(key(abi::KEY_HOME), &mut demo);
    for _ in 0..7 { w.key(key(abi::KEY_UP), &mut demo); }
    w.key(key(abi::KEY_ENTER), &mut demo);
    typed(&mut w, &mut demo, "abc");
    assert!(matches!(w.key(key(abi::KEY_ESC), &mut demo), Flow::Continue) && w.status().starts_with("MODE=LIST"));
    assert!(matches!(w.key(char_key('q'), &mut demo), Flow::Quit));
}

#[test]
fn without_a_station_the_screen_says_why() {
    let mut absent = Absent(String::from("no station here"));
    let mut w = Wifi::new(false);
    w.refresh(&mut absent);
    assert_eq!(w.problem.as_deref(), Some("no station here"));
    let shown = screen(&w);
    assert!(shown.iter().any(|row| row.contains("No Wi-Fi station")) && shown.iter().any(|row| row.contains("wifi --demo")));
    assert!(matches!(w.key(key(abi::KEY_ENTER), &mut absent), Flow::Quit));
}
