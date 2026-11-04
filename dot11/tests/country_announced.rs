//! The country the access points announce (550-NET-0035, the maintainer's decision of 2026-11-04): each beacon's
//! Country element read, and the code most of them name chosen, none at a tie.
use dot11::frame::{self, Mac, BROADCAST};
use dot11::scan::{self, List};

// A beacon of `bssid` on channel 6, with a Country element of `country` (its whole body) when given.
fn beacon(bssid: Mac, country: Option<&[u8]>) -> Vec<u8> {
    let mut f = vec![frame::BEACON << 4, 0, 0, 0];
    f.extend_from_slice(&BROADCAST);
    f.extend_from_slice(&bssid);
    f.extend_from_slice(&bssid);
    f.extend_from_slice(&16u16.to_le_bytes());
    f.extend_from_slice(&[0; 8]);
    f.extend_from_slice(&100u16.to_le_bytes());
    f.extend_from_slice(&frame::CAPABILITY_ESS.to_le_bytes());
    f.extend_from_slice(&[0, 4]);
    f.extend_from_slice(b"MIND");
    f.extend_from_slice(&[3, 1, 6]);
    if let Some(body) = country {
        f.extend_from_slice(&[frame::COUNTRY, body.len() as u8]);
        f.extend_from_slice(body);
    }
    f
}

fn heard(list: &mut List, n: u8, country: Option<&[u8]>) {
    let f = beacon([0x02, 0, 0, 0, 0, n], country);
    let h = frame::header(&f).unwrap();
    list.insert(scan::describe(&f, &h, 6, 50, 0).unwrap());
}

#[test]
fn a_beacons_country_is_read() {
    let f = beacon([0x02, 0, 0, 0, 0, 1], Some(b"UZ \x01\x0d\x14"));
    let h = frame::header(&f).unwrap();
    assert_eq!(scan::describe(&f, &h, 6, 50, 0).unwrap().country, Some(*b"UZ"));
    // Indoor and outdoor marks, and no triplets.
    for body in [&b"USI"[..], b"USO", b"US\x04"] {
        let f = beacon([0x02, 0, 0, 0, 0, 1], Some(body));
        assert_eq!(scan::describe(&f, &frame::header(&f).unwrap(), 6, 50, 0).unwrap().country, Some(*b"US"));
    }
}

#[test]
fn a_country_element_that_names_no_country_is_none() {
    for body in [&b"uz "[..], b"U1 ", b"XX ", b"UZ", b"", b"\xff\xff "] {
        let f = beacon([0x02, 0, 0, 0, 0, 1], Some(body));
        assert_eq!(scan::describe(&f, &frame::header(&f).unwrap(), 6, 50, 0).unwrap().country, None, "{body:?}");
    }
    let f = beacon([0x02, 0, 0, 0, 0, 1], None);
    assert_eq!(scan::describe(&f, &frame::header(&f).unwrap(), 6, 50, 0).unwrap().country, None);
}

#[test]
fn the_code_most_access_points_name_is_chosen() {
    let mut list = List::default();
    assert_eq!(list.announced(), None);
    heard(&mut list, 1, None);
    assert_eq!(list.announced(), None, "no access point names a country");
    heard(&mut list, 2, Some(b"UZ "));
    assert_eq!(list.announced(), Some((*b"UZ", 1, 1)));
    heard(&mut list, 3, Some(b"UZ "));
    heard(&mut list, 4, Some(b"RU "));
    assert_eq!(list.announced(), Some((*b"UZ", 2, 3)));
    // The same access point heard again counts once.
    heard(&mut list, 4, Some(b"RU "));
    assert_eq!(list.announced(), Some((*b"UZ", 2, 3)));
}

#[test]
fn a_tie_chooses_nothing() {
    let mut list = List::default();
    heard(&mut list, 1, Some(b"UZ "));
    heard(&mut list, 2, Some(b"KZ "));
    assert_eq!(list.announced(), None, "a border: as many name either");
    heard(&mut list, 3, Some(b"KZ "));
    assert_eq!(list.announced(), Some((*b"KZ", 2, 3)));
    list.clear();
    assert_eq!(list.announced(), None);
}
