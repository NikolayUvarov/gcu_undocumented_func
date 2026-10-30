// Host tests of bcm_wifi's levels with the external LNA held off (550-DRV-0045): the production logic on raw power bytes.
#[path = "../bcm_wifi/src/level.rs"]
mod level;

use level::{level, levels, LNA_GAIN};

#[test]
fn a_level_read_with_the_lna_held_off_gets_its_gain_back() {
    assert_eq!(level((-60i8) as u8, LNA_GAIN), -42);
    assert_eq!(level((-127i8) as u8, LNA_GAIN), -109);
}

#[test]
fn no_level_stays_no_level() {
    assert_eq!(level(0, LNA_GAIN), 0);
    assert_eq!(level(0x80, LNA_GAIN), -128);
}

#[test]
fn a_strong_level_stays_below_zero() {
    assert_eq!(level((-5i8) as u8, LNA_GAIN), -1);
}

#[test]
fn without_the_hold_a_level_is_as_read() {
    assert_eq!(levels([(-60i8) as u8, 0, 0x80], 0), [-60, 0, -128]);
}

#[test]
fn every_core_is_corrected() {
    assert_eq!(levels([(-60i8) as u8, (-70i8) as u8, 0x80], LNA_GAIN), [-42, -52, -128]);
}
