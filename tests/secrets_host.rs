//! Host tests of a program's secrets as keystore keeps them before sealing (keystore/src/secret_file.rs, 000-NET-0045):
//! the record read back as written, a name or value refused, the limits of count and bytes, and records the decoder
//! refuses (cut short, a name twice, bytes after the end).
extern crate alloc;
#[path = "../keystore/src/secret_file.rs"]
mod secret_file;

use secret_file::{program_ok, Refused, Secrets, BYTES_MAX, NAME_MAX, PROGRAM_MAX, SECRETS, VALUE_MAX};

#[test]
fn a_record_reads_back_as_written() {
    let mut secrets = Secrets::default();
    secrets.put(b"bot-token", b"123456:ABC-DEF").unwrap();
    secrets.put(b"api.key", &[0u8, 1, 2, 255]).unwrap();
    secrets.put(b"bot-token", b"replaced").unwrap(); // in place: the order stays
    let plain = secrets.encode();
    assert_eq!(plain, [&[2u8, 9][..], b"bot-token", &[8, 0], b"replaced", &[7], b"api.key", &[4, 0], &[0, 1, 2, 255]].concat());
    let back = Secrets::decode(&plain).unwrap();
    assert_eq!(back.names().collect::<Vec<_>>(), [&b"bot-token"[..], b"api.key"]);
    assert_eq!(back.get(b"bot-token"), Some(&b"replaced"[..]));
    assert_eq!(back.get(b"api.key"), Some(&[0u8, 1, 2, 255][..]));
    assert_eq!(back.get(b"other"), None);
    let mut back = back;
    assert!(back.delete(b"bot-token") && !back.delete(b"bot-token"));
    assert_eq!(Secrets::decode(&back.encode()).unwrap().names().count(), 1);
    assert_eq!(Secrets::default().encode(), [0]);
    assert!(Secrets::default().is_empty() && !back.is_empty());
}

#[test]
fn names_and_values_refused() {
    let mut secrets = Secrets::default();
    for name in [&b""[..], b"a b", b"a/b", b"a:b", "имя".as_bytes(), &[b'x'; NAME_MAX + 1]] {
        assert_eq!(secrets.put(name, b"v"), Err(Refused::Name), "{:?}", name);
    }
    assert_eq!(secrets.put(b"n", b""), Err(Refused::Value));
    assert_eq!(secrets.put(b"n", &vec![7; VALUE_MAX + 1]), Err(Refused::Value));
    secrets.put(&[b'x'; NAME_MAX], &vec![7; VALUE_MAX]).unwrap();
    assert!(secrets.names().count() == 1);
    for program in ["netcheck", "tgtxt", "a.b_c-1", &"p".repeat(PROGRAM_MAX)] { assert!(program_ok(program.as_bytes()), "{}", program); }
    for program in ["", ".", "..", "Agent", "a/b", "a b", "a:b", &"p".repeat(PROGRAM_MAX + 1)] { assert!(!program_ok(program.as_bytes()), "{}", program); }
}

#[test]
fn the_limits_of_one_program() {
    let mut secrets = Secrets::default();
    for i in 0..SECRETS { secrets.put(format!("s{}", i).as_bytes(), b"v").unwrap(); }
    assert_eq!(secrets.put(b"one-more", b"v"), Err(Refused::Full));
    secrets.put(b"s0", b"replaced").unwrap(); // a replacement is no new secret
    // The bytes of names and values together: four values of 16 KiB do not fit beside the names.
    let mut secrets = Secrets::default();
    for i in 0..3 { secrets.put(format!("k{}", i).as_bytes(), &vec![1; VALUE_MAX]).unwrap(); }
    assert_eq!(secrets.put(b"k3", &vec![1; VALUE_MAX]), Err(Refused::Full));
    let left = BYTES_MAX - 3 * (2 + VALUE_MAX) - 2;
    secrets.put(b"k3", &vec![1; left]).unwrap();
    assert_eq!(secrets.put(b"k3", &vec![1; left + 1]), Err(Refused::Full), "a replacement counts its new length");
    let plain = secrets.encode();
    assert!(plain.len() <= secret_file::PLAIN_MAX);
    assert_eq!(Secrets::decode(&plain).unwrap().get(b"k3").map(<[u8]>::len), Some(left));
}

#[test]
fn records_the_decoder_refuses() {
    let mut secrets = Secrets::default();
    secrets.put(b"a", b"one").unwrap();
    secrets.put(b"b", b"two").unwrap();
    let plain = secrets.encode();
    for cut in 0..plain.len() { assert!(Secrets::decode(&plain[..cut]).is_err(), "cut at {}", cut); }
    assert_eq!(Secrets::decode(&[plain.as_slice(), &[0]].concat()).err(), Some(Refused::Damaged), "a byte after the end");
    let twice = [&[2u8, 1][..], b"a", &[1, 0], b"x", &[1], b"a", &[1, 0], b"y"].concat();
    assert_eq!(Secrets::decode(&twice).err(), Some(Refused::Damaged), "a name twice");
    let empty = [&[1u8, 1][..], b"a", &[0, 0]].concat();
    assert_eq!(Secrets::decode(&empty).err(), Some(Refused::Damaged), "an empty value");
    let bad_name = [&[1u8, 1][..], b" ", &[1, 0], b"x"].concat();
    assert_eq!(Secrets::decode(&bad_name).err(), Some(Refused::Damaged), "a name refused");
}
