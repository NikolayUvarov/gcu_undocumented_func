#![allow(dead_code)]
#[path = "../regdb/src/lib.rs"] mod regulatory;
#[path="../rtw89_wifi/src/band.rs"] mod band;
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

// 213-DRV-0087: the 5 GHz channels at 20 MHz under the radio's own catalogue.
fn five(eirp: u8) -> impl Iterator<Item = (u8, u8, bool)> { band::CHANNELS_5G.into_iter().map(move |ch| (ch, eirp, false)) }
fn listening(limits: &Limits) -> Vec<u8> { limits.channels().filter(|&ch| limits.applied(ch).unwrap().listen).collect() }
fn sending(limits: &Limits) -> Vec<u8> { limits.channels().filter(|&ch| !limits.applied(ch).unwrap().listen).collect() }

#[test]
fn radar_channels_are_listen_only_and_outside_channels_left_out() {
    let uz = Limits::accept("UZ", 1, five(30)).unwrap();
    assert_eq!(sending(&uz), [36, 40, 44, 48]);
    assert_eq!(listening(&uz), [52, 56, 60, 64], "DFS keeps them listen-only");
    assert_eq!(uz.applied(36).unwrap().eirp_half_dbm, 40);
    assert_eq!(uz.applied(100), None, "UZ's rules stop at 5330 MHz");
    let us = Limits::accept("US", 1, five(30)).unwrap();
    assert_eq!(sending(&us), [36, 40, 44, 48, 149, 153, 157, 161, 165]);
    assert_eq!(listening(&us), [52, 56, 60, 64, 100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144]);
    assert_eq!((us.applied(36).unwrap().eirp_half_dbm, us.applied(100).unwrap().eirp_half_dbm), (46, 48));
    assert_eq!(us.applied(169), None, "169 is held by no single rule");
}

#[test]
fn the_world_only_listens_on_5_ghz() {
    let world = Limits::accept("00", 1, five(20)).unwrap();
    assert!(sending(&world).is_empty());
    assert_eq!(listening(&world), band::CHANNELS_5G[..25].to_vec(), "NO-IR and DFS everywhere, up to 165");
    let all = Limits::world(1).unwrap();
    assert_eq!(all.channels().count(), 13 + 25);
}

#[test]
fn wmm_and_environment_rules_leave_5_ghz_channels_out() {
    let ru = Limits::accept("RU", 1, five(20)).unwrap();
    assert_eq!(ru.channels().count(), 0, "NO-OUTDOOR everywhere on 5 GHz");
    let de = Limits::accept("DE", 1, five(30)).unwrap();
    assert_eq!(sending(&de), [149, 153, 157, 161, 165, 169, 173], "the ETSI WMM rule leaves 36 to 140 out");
    assert_eq!(de.applied(149).unwrap().eirp_half_dbm, 27, "25 mW floors to 27 half-dBm");
    // A channel the rules leave out may still not be named twice.
    assert_eq!(Limits::accept("RU", 1, [(36, 20, false), (36, 20, false)]).unwrap_err(), Refusal::Invalid);
    for channel in [14, 34, 38, 178, 182] { assert_eq!(Limits::accept("US", 1, [(channel, 20, false)]).unwrap_err(), Refusal::Invalid, "{channel}"); }
}
