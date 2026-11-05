//! The station's country from the catalogue (550-NET-0035, stage B; the maintainer's decision of 2026-11-04): chosen,
//! else announced, else `00`, and what each lets the station do on the radio's channels.
use dot11::catalogue::{decide, entry, listening, moves, plan, Source};
use dot11::station::Allowed;

const RADIO: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];

#[test]
fn a_chosen_country_comes_first() {
    assert_eq!(decide(Some(*b"UZ"), Some((*b"RU", 3, 3))), (*b"UZ", Source::Chosen));
    assert_eq!(decide(Some(*b"US"), None), (*b"US", Source::Chosen));
}

#[test]
fn with_none_chosen_the_announced_country_else_00() {
    assert_eq!(decide(None, Some((*b"UZ", 2, 3))), (*b"UZ", Source::Detected { announced: 2, heard: 3 }));
    assert_eq!(decide(None, None), (*b"00", Source::World));
    // A code the catalogue does not hold is not followed, chosen or announced.
    assert_eq!(decide(Some(*b"QQ"), None), (*b"00", Source::World));
    assert_eq!(decide(Some(*b"QQ"), Some((*b"KZ", 1, 1))), (*b"KZ", Source::Detected { announced: 1, heard: 1 }));
    assert_eq!(decide(None, Some((*b"QQ", 4, 4))), (*b"00", Source::World));
    // 00 is never a chosen or an announced country.
    assert_eq!(decide(Some(*b"00"), Some((*b"00", 1, 1))), (*b"00", Source::World));
}

#[test]
fn every_catalogued_country_can_be_run_under() {
    let mut n = 0;
    for country in regdb::countries().iter().filter(|c| !c.is_world()) {
        let code = country.alpha2();
        assert_eq!(decide(Some(code), None), (code, Source::Chosen), "{}", String::from_utf8_lossy(&code));
        assert!(entry(code).is_some());
        let p = plan(country, &RADIO);
        assert_eq!(p.allowed().len(), RADIO.len());
        n += 1;
    }
    assert!(n > 150, "{n} countries");
}

#[test]
fn zero_zero_lets_the_station_send_on_1_to_11_and_listen_on_12_and_13() {
    let p = plan(entry(*b"00").unwrap(), &RADIO);
    for &(channel, allowed) in p.allowed() {
        let want = if channel <= 11 { Allowed::Send } else { Allowed::Listen };
        assert_eq!(allowed, want, "channel {channel}");
    }
    assert_eq!(p.counts(), (11, 2));
    // The radio's limits: every channel the station may use, 12 and 13 to listen only.
    assert_eq!(p.limits().len(), 13);
    assert!(p.limits().iter().all(|&(c, eirp, listen)| eirp > 0 && listen == (c > 11)));
}

#[test]
fn uz_us_and_ru_each_by_their_own_rules() {
    // UZ and RU send on 1 to 13; the US on 1 to 11, with 12 and 13 outside its rules.
    let uz = plan(entry(*b"UZ").unwrap(), &RADIO);
    assert_eq!(uz.counts(), (13, 0));
    let ru = plan(entry(*b"RU").unwrap(), &RADIO);
    assert_eq!(ru.counts(), (13, 0));
    let us = plan(entry(*b"US").unwrap(), &RADIO);
    let (send, _) = us.counts();
    assert_eq!(send, 11);
    assert!(us.allowed().iter().all(|&(c, a)| c <= 11 || a != Allowed::Send));
}

#[test]
fn the_first_scan_only_listens() {
    let p = listening(&RADIO);
    assert!(p.allowed().iter().all(|&(_, a)| a == Allowed::Listen));
    assert_eq!(p.counts(), (0, 13));
}

#[test]
fn an_announcement_moves_only_a_country_not_chosen_and_never_back_to_00() {
    let chosen = (*b"UZ", Source::Chosen);
    assert_eq!(moves(chosen, Some((*b"KZ", 3, 3))), None);
    let world = (*b"00", Source::World);
    assert_eq!(moves(world, Some((*b"UZ", 1, 1))), Some((*b"UZ", Source::Detected { announced: 1, heard: 1 })));
    assert_eq!(moves(world, None), None);
    let detected = (*b"UZ", Source::Detected { announced: 2, heard: 2 });
    assert_eq!(moves(detected, Some((*b"UZ", 3, 3))), None, "the same country is not applied again");
    assert_eq!(moves(detected, None), None, "an access point that stops announcing does not send it back to 00");
    assert_eq!(moves(detected, Some((*b"KZ", 2, 3))), Some((*b"KZ", Source::Detected { announced: 2, heard: 3 })));
}
