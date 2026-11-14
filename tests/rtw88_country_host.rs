//! rtw88_wifi's country limits (wlanhw.wit 1.3): the world's 00 taken, and an unknown country told apart (801-DRV-0071).
#[path = "../rtw88_wifi/src/country.rs"]
mod country;

use country::Country;

#[test]
fn the_world_code_is_taken_and_leaves_the_country_unknown() {
    let world = Country::from("00", (1..=13).map(|ch| (ch, ch >= 12))).unwrap();
    assert!(Country::allows(Some(world), 13) && !Country::may_send(Some(world), 12, &[0x40]));
    assert!(Country::may_send(Some(world), 12, &[0x08]), "data to a network it has heard");
    assert!(Country::unknown(Some(world)), "00: the power capped");
    assert!(Country::unknown(None), "no country yet: the power capped");
    let uz = Country::from("UZ", (1..=13).map(|ch| (ch, false))).unwrap();
    assert!(!Country::unknown(Some(uz)), "a known country: not capped");
}

#[test]
fn malformed_codes_and_limits_are_refused() {
    for code in ["", "0", "000", "uz", "U1", "0A", "ZZZ"] { assert_eq!(Country::from(code, [(1, false)]), None, "{code}"); }
    assert_eq!(Country::from("UZ", [(1, false), (1, true)]), None, "a channel twice");
    assert_eq!(Country::from("UZ", [(14, false)]), None, "not the radio's channel");
    let us = Country::from("US", (1..=11).map(|ch| (ch, false))).unwrap();
    assert!(Country::allows(Some(us), 11) && !Country::allows(Some(us), 12) && Country::allows(None, 13));
}
