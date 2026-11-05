#![allow(dead_code)]
#[path = "../regdb/src/lib.rs"] mod regulatory;
#[path = "../rtw89_wifi/src/country.rs"] mod country;
use country::{Applied, Limits, Refusal};

fn all(eirp: u8) -> impl Iterator<Item = (u8, u8, bool)> { (1..=13).map(move |ch| (ch, eirp, false)) }

#[test]
fn uzbekistan_keeps_every_channel_under_its_twenty_dbm() {
    let uz = Limits::accept("UZ", 1, all(30)).unwrap();
    assert_eq!(uz.channels().collect::<Vec<_>>(), (1..=13).collect::<Vec<_>>());
    assert_eq!(uz.applied(1), Some(Applied { channel: 1, eirp_half_dbm: 40, listen: false }), "the catalogue caps 30 to 20 dBm");
    let low = Limits::accept("UZ", 2, [(6, 10, false)]).unwrap();
    assert_eq!(low.applied(6).unwrap().eirp_half_dbm, 20, "the station's lower limit holds");
    assert_eq!(low.channels().count(), 1);
}

#[test]
fn the_world_listens_only_on_twelve_and_thirteen() {
    let world = Limits::accept("00", 1, all(20)).unwrap();
    assert_eq!(world.code(), *b"00");
    for ch in 1..=11 { assert!(!world.applied(ch).unwrap().listen, "{ch}"); }
    for ch in [12, 13] { assert!(world.applied(ch).unwrap().listen, "{ch}: NO-IR stays listen-only"); }
}

#[test]
fn a_channel_outside_the_countrys_rules_is_left_out() {
    let us = Limits::accept("US", 1, all(30)).unwrap();
    assert_eq!(us.channels().max(), Some(11));
    assert_eq!(us.applied(12), None);
    assert_eq!(us.applied(1).unwrap().eirp_half_dbm, 60);
    let listen = Limits::accept("DE", 1, [(1, 20, true)]).unwrap();
    assert!(listen.applied(1).unwrap().listen, "the station's listen-only is kept");
}

#[test]
fn malformed_requests_are_invalid_and_unknown_codes_failed() {
    for code in ["", "U", "UZB", "uz", "U1", "0A"] { assert_eq!(Limits::accept(code, 1, all(20)).unwrap_err(), Refusal::Invalid, "{code}"); }
    assert_eq!(Limits::accept("UZ", 1, [(1, 20, false), (1, 20, false)]).unwrap_err(), Refusal::Invalid);
    assert_eq!(Limits::accept("UZ", 1, [(14, 20, false)]).unwrap_err(), Refusal::Invalid);
    assert_eq!(Limits::accept("UZ", 1, [(0, 20, false)]).unwrap_err(), Refusal::Invalid);
    assert_eq!(Limits::accept("ZZ", 1, all(20)).unwrap_err(), Refusal::Failed);
    assert_eq!(Limits::accept("UZ", 0, all(20)).unwrap_err(), Refusal::Failed, "version zero is no setting");
}
