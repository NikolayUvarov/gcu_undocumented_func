//! What link.conf's words set for `link` (801-LNK-0044): the rate and the address alone; none, or no link.conf at all,
//! leaves the cable working at 460800 bit/s, then 115200, with no address.
#[path = "../link/src/conf.rs"] mod conf;
use conf::*;

fn tried(words: &str) -> Vec<u32> { let (rates, n) = rates(words); rates[..n].to_vec() }

#[test]
fn no_link_conf_serves_at_the_benchs_rate_then_the_uarts() {
    assert_eq!(tried(""), [460_800, 115_200], "init passes no words without link.conf");
    assert_eq!(tried("run dev"), [460_800, 115_200], "the old words set nothing");
    assert_eq!(tried("uart"), [460_800, 115_200], "a UART that keeps 115200 refuses 460800 and gets the next");
}

#[test]
fn a_named_rate_comes_first_and_a_refused_one_falls_back() {
    assert_eq!(tried("baud=921600 dev"), [921_600, 460_800, 115_200]);
    assert_eq!(tried("baud=115200 run"), [115_200, 460_800], "each rate once");
    assert_eq!(tried("baud=460800"), [460_800, 115_200]);
    assert_eq!(tried("baud=fast address=2"), [460_800, 115_200], "a rate that is no number is left out");
    assert_eq!(tried("baud=0"), [460_800, 115_200]);
    assert_eq!(tried("baud=9600 baud=19200"), [9_600, 460_800, 115_200], "the first word counts");
}

#[test]
fn the_address_is_the_only_other_word() {
    assert_eq!(address(""), Ok(0));
    assert_eq!(address("baud=460800 dev run"), Ok(0));
    assert_eq!(address("address=1"), Ok(1));
    assert_eq!(address("uart address=247"), Ok(247));
    assert_eq!(address("address=0"), Err("0"));
    assert_eq!(address("address=248"), Err("248"));
    assert_eq!(address("address=x"), Err("x"));
}
