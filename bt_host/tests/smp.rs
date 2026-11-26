//! LE security (700-BLT-0009): the Security Manager's functions against the sample data of Core 5.3 Vol 3 Part H
//! (2.2.3, 2.2.4 and Appendix D), the P-256 Diffie-Hellman key both ways, and the pairing exchanges against a
//! simulated peripheral. Evidence for these computations and exchanges only (MC-12.1).
mod common;
use bt_host::smp_crypto::*;

fn h<const N: usize>(text: &str) -> [u8; N] {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(t.len(), 2 * N, "{}", text);
    core::array::from_fn(|i| u8::from_str_radix(&t[2 * i..2 * i + 2], 16).unwrap())
}

#[test]
fn aes_cmac_against_rfc4493() {
    let k: Key = h("2b7e1516 28aed2a6 abf71588 09cf4f3c");
    assert_eq!(e(&k, &[0; 16]), h::<16>("7df76b0c 1ab899b3 3e42f047 b91b546f"));
    assert_eq!(cmac(&k, &[]), h::<16>("bb1d6929 e9593728 7fa37d12 9b756746"));
    assert_eq!(cmac(&k, &[&h::<16>("6bc1bee2 2e409f96 e93d7e11 7393172a")]), h::<16>("070a16b4 6b4d4144 f79bdd9d d04a287c"));
    let m: [u8; 40] = h("6bc1bee2 2e409f96 e93d7e11 7393172a ae2d8a57 1e03ac9c 9eb76fac 45af8e51 30c81c46 a35ce411");
    assert_eq!(cmac(&k, &[&m[..17], &m[17..]]), h::<16>("dfa66747 de9ae630 30ca3261 1497c827"));
    let m: [u8; 64] = h("6bc1bee2 2e409f96 e93d7e11 7393172a ae2d8a57 1e03ac9c 9eb76fac 45af8e51 30c81c46 a35ce411 e5fbc119 1a0a52ef f69f2445 df4f9b17 ad2b417b e66c3710");
    assert_eq!(cmac(&k, &[&m]), h::<16>("51f0bebf 7e3b9d92 fc497417 79363cfe"));
}

#[test]
fn legacy_c1_and_s1() {
    // 2.2.3: p1 = 05000800000302 070710000001 01 00 01 (pres, preq, rat 0, iat 1), p2 from A1A2A3A4A5A6 and B1B2B3B4B5B6.
    let c = c1(&[0; 16], &h("5783D52156AD6F0E6388274EC6702EE0"), &h("07071000000101"), &h("05000800000302"), 1, &h("A1A2A3A4A5A6"), 0, &h("B1B2B3B4B5B6"));
    assert_eq!(c, h::<16>("1E1E3FEF878988EAD2A74DC5BEF13B86"));
    assert_eq!(s1(&[0; 16], &h("000F0E0D0C0B0A091122334455667788"), &h("010203040506070899AABBCCDDEEFF00")), h::<16>("9a1fe1f0e8b0f49b5b4216ae796da062"));
}

#[test]
fn secure_connections_f4_f5_f6_g2_and_ah() {
    let u: [u8; 32] = h("20b003d2 f297be2c 5e2c83a7 e9f9a5b9 eff49111 acf4fddb cc030148 0e359de6");
    let v: [u8; 32] = h("55188b3d 32f6bb9a 900afcfb eed4e72a 59cb9ac2 f19d7cfb 6b4fdd49 f47fc5fd");
    let x: Key = h("d5cb8454 d177733e ffffb2ec 712baeab");
    let y: Key = h("a6e8e7cc 25a75f6e 216583f7 ff3dc4cf");
    assert_eq!(f4(&u, &v, &x, 0), h::<16>("f2c916f1 07a9bd1c f1eda1be a974872d"));
    let w: [u8; 32] = h("ec0234a3 57c8ad05 341010a6 0a397d9b 99796b13 b4f866f1 868d34f3 73bfa698");
    let (a1, a2): ([u8; 7], [u8; 7]) = (h("00561237 37bfce"), h("00a71370 2dcfc1"));
    let (mackey, ltk) = f5(&w, &x, &y, &a1, &a2);
    assert_eq!(mackey, h::<16>("2965f176 a1084a02 fd3f6a20 ce636e20"));
    assert_eq!(ltk, h::<16>("69867911 69d7cd23 980522b5 94750a38"));
    assert_eq!(f6(&mackey, &x, &y, &h("12a3343b b453bb54 08da42d2 0c2d0fc8"), &h("010102"), &a1, &a2), h::<16>("e3c47398 9cd0e8c5 d26c0b09 da958f61"));
    assert_eq!(g2(&u, &v, &x, &y), 0x2f9ed5ba);
    assert_eq!(ah(&h("ec0234a3 57c8ad05 341010a6 0a397d9b"), h("708194")), h::<3>("0dfbaa"));
    assert_eq!(rev(h::<3>("0dfbaa")), [0xaa, 0xfb, 0x0d]);
}

#[test]
fn both_sides_reach_one_dhkey() {
    let (a, ax, ay) = keypair(&[0x11; 32]).unwrap();
    let (b, bx, by) = keypair(&[0x22; 32]).unwrap();
    assert_eq!(dhkey(&a, &bx, &by), dhkey(&b, &ax, &ay));
    assert!(dhkey(&a, &bx, &by).is_some());
    // A point off the curve, and zero as a secret, are refused.
    let mut off = by;
    off[31] ^= 1;
    assert!(dhkey(&a, &bx, &off).is_none());
    assert!(keypair(&[0; 32]).is_none());
}
