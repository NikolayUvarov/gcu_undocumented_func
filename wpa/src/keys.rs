//! Keys of a WPA2-PSK association: the PMK from the passphrase (J.4), the PRF (12.7.1.2) and the PTK it expands to.
use hmac::{Hmac, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

/// The PMK of a passphrase (8 to 63 printable ASCII characters) for an SSID of up to 32 bytes: PBKDF2-HMAC-SHA1 over
/// 4096 rounds (J.4.1). None for a passphrase or SSID outside those bounds.
pub fn pmk(passphrase: &[u8], ssid: &[u8]) -> Option<[u8; 32]> {
    if !(8..=63).contains(&passphrase.len()) || passphrase.iter().any(|&c| !(0x20..=0x7E).contains(&c)) || ssid.is_empty() || ssid.len() > 32 {
        return None;
    }
    let mut out = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha1>(passphrase, ssid, 4096, &mut out);
    Some(out)
}

/// A PSK given as 64 hexadecimal digits instead of a passphrase.
pub fn psk_hex(text: &[u8]) -> Option<[u8; 32]> {
    if text.len() != 64 { return None; }
    let digit = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = [0u8; 32];
    for (byte, pair) in out.iter_mut().zip(text.chunks(2)) { *byte = digit(pair[0])? << 4 | digit(pair[1])?; }
    Some(out)
}

/// PRF-n of 12.7.1.2: HMAC-SHA1(K, A || 0 || B || i) for i = 0, 1, … until `out` is full.
pub fn prf(key: &[u8], label: &[u8], data: &[u8], out: &mut [u8]) {
    for (i, chunk) in out.chunks_mut(20).enumerate() {
        let mut mac = <HmacSha1 as Mac>::new_from_slice(key).expect("HMAC takes a key of any length");
        mac.update(label);
        mac.update(&[0]);
        mac.update(data);
        mac.update(&[i as u8]);
        chunk.copy_from_slice(&mac.finalize().into_bytes()[..chunk.len()]);
    }
}

/// The pairwise keys of CCMP-128 with AKM 2 (PSK): the EAPOL-Key confirmation and encryption keys, and the temporal key.
#[derive(Clone, PartialEq, Eq)]
pub struct Ptk { pub kck: [u8; 16], pub kek: [u8; 16], pub tk: [u8; 16] }

/// PTK = PRF-384(PMK, "Pairwise key expansion", Min(AA, SPA) || Max(AA, SPA) || Min(ANonce, SNonce) || Max(ANonce, SNonce)).
pub fn ptk(pmk: &[u8; 32], aa: &[u8; 6], spa: &[u8; 6], anonce: &[u8; 32], snonce: &[u8; 32]) -> Ptk {
    let mut data = [0u8; 76];
    let (low, high) = if aa < spa { (aa, spa) } else { (spa, aa) };
    data[..6].copy_from_slice(low);
    data[6..12].copy_from_slice(high);
    let (low, high) = if anonce < snonce { (anonce, snonce) } else { (snonce, anonce) };
    data[12..44].copy_from_slice(low);
    data[44..].copy_from_slice(high);
    let mut out = [0u8; 48];
    prf(pmk, b"Pairwise key expansion", &data, &mut out);
    let mut ptk = Ptk { kck: [0; 16], kek: [0; 16], tk: [0; 16] };
    ptk.kck.copy_from_slice(&out[..16]);
    ptk.kek.copy_from_slice(&out[16..32]);
    ptk.tk.copy_from_slice(&out[32..]);
    ptk
}
