//! Host tests of libmind/src/sha256.rs and libmind/src/cid.rs (issue 300-STO-0001; MC-4.2, 4.13): SHA-256 against the
//! FIPS 180-2 examples and digests from Python's hashlib, content identifiers against CIDv1 computed by the reference
//! Python library `multiformats` 0.3.1, and every unsupported or non-canonical form refused.
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[path = "../libmind/src/cid.rs"]
mod cid;

use cid::{Algorithm, Cid, Codec, Error, BYTES, TEXT};
use sha256::{digest, Sha256};

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn unhex(text: &str) -> Vec<u8> { (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect() }
fn pattern(len: usize) -> Vec<u8> { (0..len).map(|i| (i % 251) as u8).collect() }

#[test]
fn sha256_fips_examples() {
    assert_eq!(hex(&digest(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(hex(&digest(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(hex(&digest(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    assert_eq!(hex(&digest(b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu")),
        "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1");
    assert_eq!(hex(&digest(&vec![b'a'; 1_000_000])), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
}

#[test]
fn sha256_lengths_around_the_padding() {
    // The padding takes one block or two depending on the length mod 64: every case, from hashlib.
    for (len, expected) in [
        (1, "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d"),
        (55, "463eb28e72f82e0a96c0a4cc53690c571281131f672aa229e0d45ae59b598b59"),
        (56, "da2ae4d6b36748f2a318f23e7ab1dfdf45acdc9d049bd80e59de82a60895f562"),
        (57, "2fe741af801cc238602ac0ec6a7b0c3a8a87c7fc7d7f02a3fe03d1c12eac4d8f"),
        (63, "29af2686fd53374a36b0846694cc342177e428d1647515f078784d69cdb9e488"),
        (64, "fdeab9acf3710362bd2658cdc9a29e8f9c757fcf9811603a8c447cd1d9151108"),
        (65, "4bfd2c8b6f1eec7a2afeb48b934ee4b2694182027e6d0fc075074f2fabb31781"),
        (119, "da18797ed7c3a777f0847f429724a2d8cd5138e6ed2895c3fa1a6d39d18f7ec6"),
        (120, "f52b23db1fbb6ded89ef42a23ce0c8922c45f25c50b568a93bf1c075420bbb7c"),
        (127, "92ca0fa6651ee2f97b884b7246a562fa71250fedefe5ebf270d31c546bfea976"),
        (128, "471fb943aa23c511f6f72f8d1652d9c880cfa392ad80503120547703e56a2be5"),
        (129, "5099c6a56203f9687f7d33f4bfdf576d31dc91f6b695ecea38b2770c87631135"),
        (1000, "4e4c294b331f7a2099a379bec34b9f9fc03dc46ab465d998f4d683da53487e6d"),
    ] {
        assert_eq!(hex(&digest(&pattern(len))), expected, "length {len}");
    }
}

#[test]
fn sha256_in_pieces_equals_one_call() {
    let data = pattern(300);
    let whole = digest(&data);
    for split in 0..=data.len() {
        let mut hash = Sha256::new();
        hash.update(&data[..split]);
        hash.update(&data[split..]);
        assert_eq!(hash.finish(), whole, "split at {split}");
    }
    let mut hash = Sha256::new();
    for byte in &data { hash.update(core::slice::from_ref(byte)); }
    assert_eq!(hash.finish(), whole);
}

// (content, text form, binary form in hex) from multiformats: CID("base32", 1, "raw", multihash.digest(data, "sha2-256")).
fn vectors() -> Vec<(Vec<u8>, &'static str, &'static str)> {
    vec![
        (b"".to_vec(), "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku",
            "01551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
        (b"hello world".to_vec(), "bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e",
            "01551220b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"),
        (b"abc".to_vec(), "bafkreif2pall7dybz7vecqka3zo24irdwabwdi4wc55jznaq75q7eaavvu",
            "01551220ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
        ((0..4096).map(|i| i as u8).collect(), "bafkreigi6xidihku3fi2ogytnzxcv7frjui63beju6xbe2up5yg7n3hrsm",
            "01551220c8f5d0341d54d951a71b136e6e2afcb14d11ed8489a7ae126a8fee0df6ecf193"),
    ]
}

#[test]
fn identifiers_match_multiformats() {
    for (data, text, binary) in vectors() {
        let cid = Cid::raw(&data);
        assert_eq!((cid.codec(), cid.algorithm()), (Codec::Raw, Algorithm::Sha2_256));
        assert_eq!(hex(&cid.to_bytes()), binary);
        assert_eq!(std::str::from_utf8(&cid.to_text()).unwrap(), text);
        assert_eq!(cid.to_string(), text);
        assert_eq!(Cid::from_bytes(&unhex(binary)), Ok(cid));
        assert_eq!(Cid::from_text(text.as_bytes()), Ok(cid));
        assert!(cid.matches(&data));
    }
    assert_eq!((BYTES, TEXT), (36, 59));
}

#[test]
fn a_changed_byte_is_another_identifier() {
    let data = pattern(4096);
    let cid = Cid::raw(&data);
    for at in [0, 1, 63, 64, 2048, 4095] {
        let mut changed = data.clone();
        changed[at] ^= 1;
        assert!(!cid.matches(&changed), "byte {at}");
        assert_ne!(Cid::raw(&changed), cid);
    }
    assert!(!cid.matches(&data[..4095]));
}

#[test]
fn read_takes_one_identifier_from_a_longer_record() {
    let cid = Cid::raw(b"abc");
    let mut record = cid.to_bytes().to_vec();
    record.extend_from_slice(b"next field");
    assert_eq!(Cid::read(&record), Ok((cid, BYTES)));
    assert_eq!(Cid::from_bytes(&record), Err(Error::Trailing));
}

#[test]
fn unsupported_binary_forms_are_refused() {
    let good = Cid::raw(b"abc").to_bytes();
    let with = |header: &[u8], digest: &[u8]| [header, digest].concat();
    let digest = &good[4..];
    // CIDv0 is a bare multihash: its first byte, the hash code 0x12, reads as version 18.
    assert_eq!(Cid::from_bytes(&with(&[0x12, 0x20], digest)), Err(Error::Version));
    assert_eq!(Cid::from_bytes(&with(&[0x02, 0x55, 0x12, 0x20], digest)), Err(Error::Version));
    assert_eq!(Cid::from_bytes(&with(&[0x00, 0x55, 0x12, 0x20], digest)), Err(Error::Version));
    // dag-pb, dag-cbor: types not supported yet.
    assert_eq!(Cid::from_bytes(&with(&[0x01, 0x70, 0x12, 0x20], digest)), Err(Error::Codec));
    assert_eq!(Cid::from_bytes(&with(&[0x01, 0x71, 0x12, 0x20], digest)), Err(Error::Codec));
    // sha2-512, sha3-256, identity: algorithms not supported, whatever the digest.
    assert_eq!(Cid::from_bytes(&with(&[0x01, 0x55, 0x13, 0x40], &[digest, digest].concat())), Err(Error::Algorithm));
    assert_eq!(Cid::from_bytes(&with(&[0x01, 0x55, 0x16, 0x20], digest)), Err(Error::Algorithm));
    assert_eq!(Cid::from_bytes(&with(&[0x01, 0x55, 0x00, 0x03], b"abc")), Err(Error::Algorithm));
    // A sha2-256 digest of another length, shortened or not.
    assert_eq!(Cid::from_bytes(&with(&[0x01, 0x55, 0x12, 0x1f], &digest[..31])), Err(Error::Length));
    assert_eq!(Cid::from_bytes(&with(&[0x01, 0x55, 0x12, 0x21], &[digest, &[0]].concat())), Err(Error::Length));
    // The same numbers written longer than needed: not one encoding per identifier.
    assert_eq!(Cid::from_bytes(&with(&[0x81, 0x00, 0x55, 0x12, 0x20], digest)), Err(Error::Varint));
    assert_eq!(Cid::from_bytes(&with(&[0x01, 0xd5, 0x00, 0x12, 0x20], digest)), Err(Error::Varint));
    assert_eq!(Cid::from_bytes(&[0xff; 12]), Err(Error::Varint));
    // Cut anywhere, or with a byte more.
    for len in 0..BYTES { assert_eq!(Cid::from_bytes(&good[..len]), Err(Error::Truncated), "length {len}"); }
    assert_eq!(Cid::from_bytes(&[&good[..], &[0]].concat()), Err(Error::Trailing));
    assert_eq!(Cid::from_bytes(&good), Ok(Cid::raw(b"abc")));
}

#[test]
fn only_the_canonical_text_form_is_accepted() {
    let cid = Cid::raw(b"hello world");
    let text = cid.to_text();
    let changed = |at: usize, c: u8| { let mut t = text; t[at] = c; t };
    // base32upper (B), base58btc (z), base16 (f) and no prefix: other multibase encodings are not read.
    assert_eq!(Cid::from_text(&changed(0, b'B')), Err(Error::Text));
    assert_eq!(Cid::from_text(&text.to_ascii_uppercase()), Err(Error::Text));
    assert_eq!(Cid::from_text(&changed(0, b'z')), Err(Error::Text));
    assert_eq!(Cid::from_text(format!("f{}", hex(&cid.to_bytes())).as_bytes()), Err(Error::Text));
    assert_eq!(Cid::from_text(&text[1..]), Err(Error::Text));
    assert_eq!(Cid::from_text(b""), Err(Error::Text));
    // Characters outside the alphabet and padding.
    assert_eq!(Cid::from_text(&changed(10, b'1')), Err(Error::Text));
    assert_eq!(Cid::from_text(&changed(10, b'A')), Err(Error::Text));
    assert_eq!(Cid::from_text(&[&text[..], b"="].concat()), Err(Error::Text));
    assert_eq!(Cid::from_text(&[&text[..], b"======"].concat()), Err(Error::Text));
    // The last character carries 3 bits of the digest and 2 that must be zero.
    let last = text[TEXT - 1];
    let value = b"abcdefghijklmnopqrstuvwxyz234567".iter().position(|&d| d == last).unwrap();
    assert_eq!(value & 3, 0);
    for stray in 1..4 {
        let c = b"abcdefghijklmnopqrstuvwxyz234567"[value | stray];
        assert_eq!(Cid::from_text(&changed(TEXT - 1, c)), Err(Error::Text));
    }
    // Characters less or more: 57 digits leave 5 bits (no base32 text), 56 make 35 bytes.
    assert_eq!(Cid::from_text(&text[..TEXT - 1]), Err(Error::Text));
    assert_eq!(Cid::from_text(&text[..TEXT - 2]), Err(Error::Truncated));
    assert_eq!(Cid::from_text(&[&text[..], b"a"].concat()), Err(Error::Text));
    assert_eq!(Cid::from_text(&[&text[..], b"aa"].concat()), Err(Error::Trailing));
    assert_eq!(Cid::from_text(&text), Ok(cid));
}

#[test]
fn ordering_follows_the_binary_form() {
    // Sorted indexes of identifiers (the block store's) may sort by either.
    let mut cids: Vec<Cid> = (0..64u8).map(|i| Cid::raw(&[i])).collect();
    let mut binary: Vec<[u8; BYTES]> = cids.iter().map(|c| c.to_bytes()).collect();
    cids.sort();
    binary.sort();
    assert_eq!(cids.iter().map(|c| c.to_bytes()).collect::<Vec<_>>(), binary);
}
