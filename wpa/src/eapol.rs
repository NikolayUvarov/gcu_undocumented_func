//! EAPOL-Key frames (IEEE 802.1X-2020 11.3, IEEE 802.11-2020 12.7.2) of descriptor type 2 (RSN), version 2 (HMAC-SHA1-128
//! MICs, AES key wrap): read, written, MIC checked and set, key data unwrapped and its elements found.
use crate::Error;
use aes_kw::KekAes128;
use hmac::{Hmac, Mac};
use sha1::Sha1;

/// Key information bits (12.7.2).
pub const VERSION: u16 = 0x0007;
pub const VERSION_SHA1_AES: u16 = 2;
pub const PAIRWISE: u16 = 1 << 3;
pub const INSTALL: u16 = 1 << 6;
pub const ACK: u16 = 1 << 7;
pub const MIC: u16 = 1 << 8;
pub const SECURE: u16 = 1 << 9;
pub const ERROR: u16 = 1 << 10;
pub const REQUEST: u16 = 1 << 11;
pub const ENCRYPTED: u16 = 1 << 12;

/// The EAPOL header (version, packet type 3, body length) and the fixed fields up to the key data.
pub const HEADER: usize = 4;
pub const FIXED: usize = HEADER + 95;
const DESCRIPTOR_RSN: u8 = 2;
const TYPE_KEY: u8 = 3;
const MIC_AT: usize = HEADER + 77;
/// The largest key data the station reads or writes: the GTK KDE, an RSN element and padding fit easily.
pub const KEY_DATA_MAX: usize = 256;

/// An EAPOL-Key frame's fields; `data` is the key data as sent (wrapped when `ENCRYPTED` is set).
#[derive(Clone, Copy, Debug)]
pub struct Key<'a> {
    pub version: u8,
    pub info: u16,
    pub key_length: u16,
    pub replay: u64,
    pub nonce: [u8; 32],
    /// The receive sequence counter of the GTK, its PN in order (PN0 first).
    pub rsc: [u8; 8],
    pub mic: [u8; 16],
    pub data: &'a [u8],
}

/// Reads an EAPOL-Key frame; bytes after the EAPOL body (Ethernet padding) are ignored.
pub fn parse(frame: &[u8]) -> Result<Key<'_>, Error> {
    if frame.len() < FIXED || frame[1] != TYPE_KEY || frame[HEADER] != DESCRIPTOR_RSN { return Err(Error::Malformed); }
    let body = u16::from_be_bytes([frame[2], frame[3]]) as usize;
    let frame = frame.get(..HEADER + body).filter(|_| HEADER + body >= FIXED).ok_or(Error::Malformed)?;
    let length = u16::from_be_bytes([frame[FIXED - 2], frame[FIXED - 1]]) as usize;
    let data = frame.get(FIXED..FIXED + length).ok_or(Error::Malformed)?;
    let field = |at: usize, n: usize| &frame[HEADER + at..HEADER + at + n];
    let mut key = Key { version: frame[0], info: u16::from_be_bytes([frame[5], frame[6]]), key_length: u16::from_be_bytes([frame[7], frame[8]]),
                        replay: u64::from_be_bytes(field(5, 8).try_into().unwrap()), nonce: [0; 32], rsc: [0; 8], mic: [0; 16], data };
    key.nonce.copy_from_slice(field(13, 32));
    key.rsc.copy_from_slice(field(61, 8));
    key.mic.copy_from_slice(field(77, 16));
    if key.info & VERSION != VERSION_SHA1_AES { return Err(Error::Malformed); }
    Ok(key)
}

/// Writes an EAPOL-Key frame with a zero MIC into `out`; returns its length. `version` is the EAPOL protocol version.
pub fn write(out: &mut [u8], version: u8, info: u16, replay: u64, nonce: &[u8; 32], data: &[u8]) -> Result<usize, Error> {
    let len = FIXED + data.len();
    if out.len() < len || data.len() > KEY_DATA_MAX { return Err(Error::Space); }
    let out = &mut out[..len];
    out.fill(0);
    out[0] = version;
    out[1] = TYPE_KEY;
    out[2..4].copy_from_slice(&((len - HEADER) as u16).to_be_bytes());
    out[HEADER] = DESCRIPTOR_RSN;
    out[5..7].copy_from_slice(&info.to_be_bytes());
    out[9..17].copy_from_slice(&replay.to_be_bytes());
    out[17..49].copy_from_slice(nonce);
    out[FIXED - 2..FIXED].copy_from_slice(&(data.len() as u16).to_be_bytes());
    out[FIXED..].copy_from_slice(data);
    Ok(len)
}

/// The MIC of a frame: HMAC-SHA1-128 under the KCK over the whole EAPOL frame with its MIC field zero.
pub fn mic(kck: &[u8; 16], frame: &[u8]) -> Result<[u8; 16], Error> {
    let body = frame.get(2..4).map(|b| u16::from_be_bytes([b[0], b[1]]) as usize).ok_or(Error::Malformed)?;
    let frame = frame.get(..HEADER + body).filter(|f| f.len() >= FIXED).ok_or(Error::Malformed)?;
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(kck).expect("HMAC takes a key of any length");
    mac.update(&frame[..MIC_AT]);
    mac.update(&[0; 16]);
    mac.update(&frame[MIC_AT + 16..]);
    let mut out = [0u8; 16];
    out.copy_from_slice(&mac.finalize().into_bytes()[..16]);
    Ok(out)
}

/// Sets the MIC of a frame `write` made.
pub fn sign(kck: &[u8; 16], frame: &mut [u8]) -> Result<(), Error> {
    let mic = mic(kck, frame)?;
    frame[MIC_AT..MIC_AT + 16].copy_from_slice(&mic);
    Ok(())
}

/// Whether a received frame's MIC is the one the KCK gives, compared in constant time.
pub fn verify(kck: &[u8; 16], frame: &[u8], key: &Key) -> bool {
    mic(kck, frame).is_ok_and(|expected| expected.iter().zip(key.mic.iter()).fold(0u8, |d, (a, b)| d | (a ^ b)) == 0)
}

/// Unwraps encrypted key data with the KEK (AES key wrap, RFC 3394) into `out`; returns the plaintext's length.
pub fn unwrap<'o>(kek: &[u8; 16], data: &[u8], out: &'o mut [u8; KEY_DATA_MAX]) -> Result<&'o [u8], Error> {
    if data.len() < 24 || data.len() % 8 != 0 || data.len() - 8 > KEY_DATA_MAX { return Err(Error::KeyData); }
    let plain = &mut out[..data.len() - 8];
    KekAes128::from(*kek).unwrap(data, plain).map_err(|_| Error::KeyData)?;
    Ok(plain)
}

/// The group key a GTK KDE carries (12.7.2, table 12-10).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Gtk { pub id: u8, pub key: [u8; 16] }

/// What the key data of message 3 or of a group message carries: the RSN element and the GTK, where present.
#[derive(Default, Debug)]
pub struct Elements<'a> { pub rsne: Option<&'a [u8]>, pub gtk: Option<Gtk> }

/// Finds the elements and KDEs of (unwrapped) key data; padding (0xDD, then zeros) ends it.
pub fn elements(data: &[u8]) -> Result<Elements<'_>, Error> {
    let mut found = Elements::default();
    let mut at = 0;
    while at < data.len() {
        let id = data[at];
        if id == 0xDD && data[at + 1..].iter().all(|&b| b == 0) { break; }
        let len = *data.get(at + 1).ok_or(Error::KeyData)? as usize;
        let body = data.get(at + 2..at + 2 + len).ok_or(Error::KeyData)?;
        match id {
            0x30 if found.rsne.is_none() => found.rsne = Some(&data[at..at + 2 + len]),
            // The GTK KDE: OUI 00-0F-AC, data type 1, then the key ID (bits 0-1), a reserved byte and the key.
            0xDD if body.len() >= 4 && body[..4] == [0x00, 0x0F, 0xAC, 1] => {
                let gtk = body.get(6..).filter(|k| k.len() == 16).ok_or(Error::KeyData)?;
                let mut key = [0u8; 16];
                key.copy_from_slice(gtk);
                found.gtk = Some(Gtk { id: body[4] & 3, key });
            }
            _ => {}
        }
        at += 2 + len;
    }
    Ok(found)
}
