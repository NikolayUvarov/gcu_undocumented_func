//! CCMP-128 (12.5.3): an MPDU's body encrypted and authenticated under the TK with AES-CCM (M = 8, L = 2), the 802.11
//! header's fixed parts as additional data, and a 48-bit packet number that each receiver only lets rise.
use crate::Error;
use aes::Aes128;
use ccm::aead::generic_array::GenericArray;
use ccm::aead::{AeadInPlace, KeyInit};
use ccm::consts::{U13, U8};
use ccm::Ccm;

type Cipher = Ccm<Aes128, U8, U13>;

/// The CCMP header (PN0, PN1, reserved, key ID and ExtIV, PN2 to PN5) and the MIC after the body.
pub const HEADER: usize = 8;
pub const MIC: usize = 8;
pub const PN_MAX: u64 = (1 << 48) - 1;

/// The parts of an 802.11 MAC header CCMP needs.
struct Header { len: usize, management: bool, priority: u8 }

fn header(mac: &[u8]) -> Result<Header, Error> {
    let fc = *mac.first().ok_or(Error::Malformed)?;
    let flags = *mac.get(1).ok_or(Error::Malformed)?;
    let (kind, subtype) = ((fc >> 2) & 3, fc >> 4);
    let qos = kind == 2 && subtype & 8 != 0;
    let mut len = 24 + if kind == 2 && flags & 3 == 3 { 6 } else { 0 };
    let priority = if qos { mac.get(len).ok_or(Error::Malformed)? & 0x0F } else { 0 };
    if qos { len += 2 + if flags & 0x80 != 0 { 4 } else { 0 }; }
    if kind == 1 || kind == 3 || mac.len() < len { return Err(Error::Malformed); }
    Ok(Header { len, management: kind == 0, priority })
}

/// The additional authenticated data (12.5.3.3.3): FC with the subtype (data), Retry, Power Management and More Data bits
/// masked, Protected set and Order masked in QoS data; A1 to A3; the sequence control's fragment number; A4; QoS
/// control's TID. Returns its length.
fn aad(mac: &[u8], h: &Header, out: &mut [u8; 30]) -> usize {
    let qos = !h.management && mac[0] & 0x80 != 0;
    out[0] = if h.management { mac[0] } else { mac[0] & 0x8F };
    out[1] = (mac[1] & !0x38 & if qos { !0x80 } else { 0xFF }) | 0x40;
    out[2..20].copy_from_slice(&mac[4..22]);
    out[20] = mac[22] & 0x0F;
    out[21] = 0;
    let mut len = 22;
    if !h.management && mac[1] & 3 == 3 { out[22..28].copy_from_slice(&mac[24..30]); len = 28; }
    if qos { let at = if len == 28 { 30 } else { 24 }; out[len] = mac[at] & 0x0F; out[len + 1] = 0; len += 2; }
    len
}

/// The CCM nonce (12.5.3.3.4): flags (priority, management bit), A2, the PN from PN5 down to PN0.
fn nonce(mac: &[u8], h: &Header, pn: u64) -> [u8; 13] {
    let mut nonce = [0u8; 13];
    nonce[0] = h.priority | if h.management { 0x10 } else { 0 };
    nonce[1..7].copy_from_slice(&mac[10..16]);
    nonce[7..].copy_from_slice(&pn.to_be_bytes()[2..]);
    nonce
}

/// Protects an MPDU: `frame` holds the MAC header (Protected bit set by the caller or not) and then the plaintext body.
/// Writes header, CCMP header, ciphertext and MIC into `out`; returns its length. `pn` must never repeat for a TK.
pub fn encrypt(tk: &[u8; 16], key_id: u8, pn: u64, frame: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let h = header(frame)?;
    let total = frame.len() + HEADER + MIC;
    if pn == 0 || pn > PN_MAX || key_id > 3 { return Err(Error::Malformed); }
    if out.len() < total { return Err(Error::Space); }
    let out = &mut out[..total];
    out[..h.len].copy_from_slice(&frame[..h.len]);
    out[1] |= 0x40;
    let p = pn.to_le_bytes();
    out[h.len..h.len + HEADER].copy_from_slice(&[p[0], p[1], 0, 0x20 | key_id << 6, p[2], p[3], p[4], p[5]]);
    let body = h.len + HEADER;
    out[body..total - MIC].copy_from_slice(&frame[h.len..]);
    let mut additional = [0u8; 30];
    let a = aad(out, &h, &mut additional);
    let (head, rest) = out.split_at_mut(total - MIC);
    let tag = Cipher::new(GenericArray::from_slice(tk))
        .encrypt_in_place_detached(GenericArray::from_slice(&nonce(head, &h, pn)), &additional[..a], &mut head[body..])
        .map_err(|_| Error::Malformed)?;
    rest.copy_from_slice(&tag);
    Ok(total)
}

/// The key ID and PN of a protected MPDU, read before decrypting it (to pick the key and check the PN first).
pub fn peek(frame: &[u8]) -> Result<(u8, u64), Error> {
    let h = header(frame)?;
    let c = frame.get(h.len..h.len + HEADER).filter(|c| c[3] & 0x20 != 0).ok_or(Error::Malformed)?;
    Ok((c[3] >> 6, u64::from_le_bytes([c[0], c[1], c[4], c[5], c[6], c[7], 0, 0])))
}

/// Checks and decrypts a protected MPDU: writes the MAC header (Protected bit cleared) and the plaintext body into `out`.
/// Returns the plaintext frame's length and its PN, which the caller checks against its replay counter (`Replay`).
pub fn decrypt(tk: &[u8; 16], frame: &[u8], out: &mut [u8]) -> Result<(usize, u64), Error> {
    let h = header(frame)?;
    let (_, pn) = peek(frame)?;
    if frame.len() < h.len + HEADER + MIC { return Err(Error::Malformed); }
    let total = frame.len() - HEADER - MIC;
    if out.len() < total { return Err(Error::Space); }
    let out = &mut out[..total];
    out[..h.len].copy_from_slice(&frame[..h.len]);
    out[h.len..].copy_from_slice(&frame[h.len + HEADER..frame.len() - MIC]);
    let mut additional = [0u8; 30];
    let a = aad(frame, &h, &mut additional);
    Cipher::new(GenericArray::from_slice(tk))
        .decrypt_in_place_detached(GenericArray::from_slice(&nonce(frame, &h, pn)), &additional[..a], &mut out[h.len..],
                                   GenericArray::from_slice(&frame[frame.len() - MIC..]))
        .map_err(|_| Error::Mic)?;
    out[1] &= !0x40;
    Ok((total, pn))
}

/// A receiver's highest PN for each traffic identifier (0-15) and for management frames (16).
#[derive(Default, Clone)]
pub struct Replay { last: [u64; 17] }

impl Replay {
    /// A receive sequence counter to start from: the GTK's RSC from the handshake, for every identifier.
    pub fn starting(pn: u64) -> Self { Replay { last: [pn; 17] } }
    /// Takes `pn` for identifier `tid` if it is above every one taken before; a frame whose PN does not rise is dropped.
    pub fn accept(&mut self, tid: usize, pn: u64) -> Result<(), Error> {
        let last = self.last.get_mut(tid).ok_or(Error::Malformed)?;
        if pn <= *last { return Err(Error::Replay); }
        *last = pn;
        Ok(())
    }
}

/// The priority a frame's PN is counted under: its TID for QoS data, 16 for a management frame, else 0.
pub fn tid(frame: &[u8]) -> Result<usize, Error> {
    let h = header(frame)?;
    Ok(if h.management { 16 } else { h.priority as usize })
}
