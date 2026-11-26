//! The Security Manager's cryptographic functions (Core 5.3 Vol 3 Part H 2.2): e (AES-128), c1 and s1 for legacy
//! pairing, f4, f5, f6 and g2 on AES-CMAC (RFC 4493) for LE Secure Connections, ah for resolvable addresses, and the
//! P-256 key pair and Diffie-Hellman key. Values are as the specification writes them, most significant octet first;
//! SMP carries them least significant first (`rev`).
use aes::cipher::{BlockEncrypt, KeyInit};
use aes::Aes128;
use cmac::{Cmac, Mac};

pub type Key = [u8; 16];

/// A value turned between the specification's order and the air's.
pub fn rev<const N: usize>(v: [u8; N]) -> [u8; N] { let mut r = v; r.reverse(); r }

/// The security function e: AES-128 of `block` under `key`.
pub fn e(key: &Key, block: &Key) -> Key {
    let cipher = Aes128::new(key.into());
    let mut b = (*block).into();
    cipher.encrypt_block(&mut b);
    b.into()
}

/// AES-CMAC of the parts' concatenation under `key`.
pub fn cmac(key: &Key, parts: &[&[u8]]) -> Key {
    let mut mac = <Cmac<Aes128> as Mac>::new_from_slice(key).expect("a 16-byte key");
    for p in parts { mac.update(p); }
    mac.finalize().into_bytes().into()
}

fn xor(a: &Key, b: &Key) -> Key { core::array::from_fn(|i| a[i] ^ b[i]) }

/// c1, the legacy confirm value: `preq` and `pres` the 7-octet pairing request and response as the specification writes
/// them (their code the least significant octet), the address types and addresses of initiator and responder.
pub fn c1(k: &Key, r: &Key, preq: &[u8; 7], pres: &[u8; 7], iat: u8, ia: &[u8; 6], rat: u8, ra: &[u8; 6]) -> Key {
    let mut p1 = [0u8; 16];
    p1[..7].copy_from_slice(pres);
    p1[7..14].copy_from_slice(preq);
    (p1[14], p1[15]) = (rat & 1, iat & 1);
    let mut p2 = [0u8; 16];
    p2[4..10].copy_from_slice(ia);
    p2[10..].copy_from_slice(ra);
    e(k, &xor(&e(k, &xor(r, &p1)), &p2))
}

/// s1, the legacy short term key: the low halves of r1 then r2.
pub fn s1(k: &Key, r1: &Key, r2: &Key) -> Key {
    let mut r = [0u8; 16];
    r[..8].copy_from_slice(&r1[8..]);
    r[8..].copy_from_slice(&r2[8..]);
    e(k, &r)
}

/// f4, LE Secure Connections' confirm value.
pub fn f4(u: &[u8; 32], v: &[u8; 32], x: &Key, z: u8) -> Key { cmac(x, &[u, v, &[z]]) }

/// The salt of f5's T.
pub const SALT: Key = [0x6C, 0x88, 0x83, 0x91, 0xAA, 0xF5, 0xA5, 0x38, 0x60, 0x37, 0x0B, 0xDB, 0x5A, 0x60, 0x83, 0xBE];

/// f5: the MacKey and the LTK from the DHKey `w`, the nonces and the 56-bit addresses (type then address).
pub fn f5(w: &[u8; 32], n1: &Key, n2: &Key, a1: &[u8; 7], a2: &[u8; 7]) -> (Key, Key) {
    let t = cmac(&SALT, &[w]);
    let one = |counter: u8| cmac(&t, &[&[counter], b"btle", n1, n2, a1, a2, &[0x01, 0x00]]);
    (one(0), one(1))
}

/// f6, the DHKey check value.
pub fn f6(w: &Key, n1: &Key, n2: &Key, r: &Key, io: &[u8; 3], a1: &[u8; 7], a2: &[u8; 7]) -> Key { cmac(w, &[n1, n2, r, io, a1, a2]) }

/// g2, the numeric comparison's value (of which six decimal digits are shown).
pub fn g2(u: &[u8; 32], v: &[u8; 32], x: &Key, y: &Key) -> u32 {
    let m = cmac(x, &[u, v, y]);
    u32::from_be_bytes([m[12], m[13], m[14], m[15]])
}

/// ah, the random address hash: the low 24 bits of e(irk, padding || r).
pub fn ah(irk: &Key, r: [u8; 3]) -> [u8; 3] {
    let mut p = [0u8; 16];
    p[13..].copy_from_slice(&r);
    let h = e(irk, &p);
    [h[13], h[14], h[15]]
}

/// A P-256 key pair from 32 random octets: the secret, and the public key's X and Y, most significant first. None when
/// the octets are not a valid secret (zero or past the order: draw again).
pub fn keypair(random: &[u8; 32]) -> Option<(p256::SecretKey, [u8; 32], [u8; 32])> {
    use p256::elliptic_curve::sec1::ToEncodedPoint;
    let secret = p256::SecretKey::from_slice(random).ok()?;
    let point = secret.public_key().to_encoded_point(false);
    let (x, y) = (point.x()?, point.y()?);
    Some((secret, (*x).into(), (*y).into()))
}

/// The Diffie-Hellman key with a peer's public key (X and Y, most significant first); None when the point is not on
/// the curve (the pairing then fails: Core Vol 3 Part H 2.3.5.6.1).
pub fn dhkey(secret: &p256::SecretKey, x: &[u8; 32], y: &[u8; 32]) -> Option<[u8; 32]> {
    use p256::elliptic_curve::sec1::FromEncodedPoint;
    let point = p256::EncodedPoint::from_affine_coordinates(x.into(), y.into(), false);
    let public: Option<p256::PublicKey> = p256::PublicKey::from_encoded_point(&point).into();
    let shared = p256::ecdh::diffie_hellman(secret.to_nonzero_scalar(), public?.as_affine());
    Some((*shared.raw_secret_bytes()).into())
}
