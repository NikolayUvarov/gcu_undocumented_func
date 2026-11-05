// Host tests of bcm_wifi's country limits (wlanhw.wit 1.3, 550-NET-0035): the production logic on plain limits.
#[path = "../bcm_wifi/src/country.rs"]
mod country;

use country::Country;

#[test]
fn the_world_domain_is_taken_with_its_listen_only_channels() {
    let world = Country::from("00", (1..=13).map(|ch| (ch, ch >= 12))).unwrap();
    assert!(Country::allows(Some(world), 1) && Country::allows(Some(world), 13));
    assert!(Country::may_send(Some(world), 6, &[0x40]), "a probe request where sending is allowed");
    assert!(!Country::may_send(Some(world), 12, &[0x40]), "no probe request where it may only listen");
    assert!(Country::may_send(Some(world), 12, &[0x08]), "data to a network it has heard");
}

#[test]
fn a_country_keeps_to_its_channels() {
    let us = Country::from("US", (1..=11).map(|ch| (ch, false))).unwrap();
    assert!(Country::allows(Some(us), 11) && !Country::allows(Some(us), 12));
    assert!(!Country::may_send(Some(us), 13, &[0x08]));
    assert!(Country::allows(None, 13), "before any limits, 1.1's channels");
}

#[test]
fn malformed_codes_and_limits_are_refused() {
    for code in ["", "0", "000", "us", "U1", "0A", "ZZZ"] { assert_eq!(Country::from(code, [(1, false)]), None, "{code}"); }
    assert_eq!(Country::from("UZ", [(1, false), (1, true)]), None, "a channel twice");
    assert_eq!(Country::from("UZ", [(14, false)]), None, "not the radio's channel");
    assert_eq!(Country::from("UZ", [(0, false)]), None);
}
