//! The access point's side of the 4-way and group key handshakes (12.7.6, 12.7.7), for test access points: the host
//! tests of the station and a stand-in radio in QEMU. It trusts its caller's ANonce and GTK and keeps one station.
use crate::eapol::{self, Gtk, ACK, ENCRYPTED, INSTALL, MIC, PAIRWISE, SECURE, VERSION_SHA1_AES};
use crate::keys::{self, Ptk};
use crate::Error;
use aes_kw::KekAes128;

/// The longest frame it sends: the fixed fields and wrapped key data with an RSN element and a GTK KDE.
pub const FRAME_MAX: usize = eapol::FIXED + 160;

pub struct Authenticator {
    pmk: [u8; 32],
    aa: [u8; 6],
    spa: [u8; 6],
    rsne: ([u8; 64], usize),
    anonce: [u8; 32],
    replay: u64,
    ptk: Option<Ptk>,
}

impl Authenticator {
    /// For station `spa` joining `aa`, whose beacons carry `rsne`.
    pub fn new(pmk: [u8; 32], aa: [u8; 6], spa: [u8; 6], rsne: &[u8], anonce: [u8; 32]) -> Result<Self, Error> {
        if rsne.len() > 64 { return Err(Error::Malformed); }
        let mut copy = [0u8; 64];
        copy[..rsne.len()].copy_from_slice(rsne);
        Ok(Authenticator { pmk, aa, spa, rsne: (copy, rsne.len()), anonce, replay: 0, ptk: None })
    }

    /// The pairwise keys once message 2 was good.
    pub fn ptk(&self) -> Option<&Ptk> { self.ptk.as_ref() }

    /// Message 1: the ANonce.
    pub fn message_1(&mut self, out: &mut [u8; FRAME_MAX]) -> Result<usize, Error> {
        self.replay += 1;
        let len = eapol::write(out, 2, VERSION_SHA1_AES | PAIRWISE | ACK, self.replay, &self.anonce, &[])?;
        out[7..9].copy_from_slice(&16u16.to_be_bytes());
        Ok(len)
    }

    /// Takes message 2: its MIC under the PTK of its SNonce.
    pub fn message_2(&mut self, frame: &[u8]) -> Result<(), Error> {
        let key = eapol::parse(frame)?;
        if key.info != VERSION_SHA1_AES | PAIRWISE | MIC || key.replay != self.replay { return Err(Error::Unexpected); }
        let ptk = keys::ptk(&self.pmk, &self.aa, &self.spa, &self.anonce, &key.nonce);
        if !eapol::verify(&ptk.kck, frame, &key) { return Err(Error::Mic); }
        self.ptk = Some(ptk);
        Ok(())
    }

    fn signed(&mut self, out: &mut [u8; FRAME_MAX], info: u16, nonce: &[u8; 32], plain: &[u8], rsc: [u8; 8]) -> Result<usize, Error> {
        let ptk = self.ptk.clone().ok_or(Error::Unexpected)?;
        let mut padded = [0u8; 120];
        let mut len = plain.len();
        padded.get_mut(..len).ok_or(Error::Space)?.copy_from_slice(plain);
        if len % 8 != 0 || len < 16 { padded[len] = 0xDD; len += 1; while len % 8 != 0 || len < 16 { len += 1; } }
        let mut wrapped = [0u8; 128];
        KekAes128::from(ptk.kek).wrap(&padded[..len], &mut wrapped[..len + 8]).map_err(|_| Error::KeyData)?;
        self.replay += 1;
        let total = eapol::write(out, 2, info, self.replay, nonce, &wrapped[..len + 8])?;
        out[eapol::HEADER + 61..eapol::HEADER + 69].copy_from_slice(&rsc);
        eapol::sign(&ptk.kck, &mut out[..total])?;
        Ok(total)
    }

    /// Message 3: the RSN element and the GTK, wrapped under the KEK.
    pub fn message_3(&mut self, gtk: &Gtk, rsc: [u8; 8], out: &mut [u8; FRAME_MAX]) -> Result<usize, Error> {
        let mut plain = [0u8; 96];
        let n = self.rsne.1;
        plain[..n].copy_from_slice(&self.rsne.0[..n]);
        plain[n..n + 8].copy_from_slice(&[0xDD, 22, 0x00, 0x0F, 0xAC, 1, gtk.id & 3, 0]);
        plain[n + 8..n + 24].copy_from_slice(&gtk.key);
        let anonce = self.anonce;
        self.signed(out, VERSION_SHA1_AES | PAIRWISE | ACK | MIC | INSTALL | SECURE | ENCRYPTED, &anonce, &plain[..n + 24], rsc)
    }

    /// Takes message 4 or group message 2: a MIC under the PTK and the replay counter just sent.
    pub fn confirmation(&mut self, frame: &[u8]) -> Result<(), Error> {
        let key = eapol::parse(frame)?;
        let ptk = self.ptk.as_ref().ok_or(Error::Unexpected)?;
        if key.replay != self.replay || key.info & (MIC | SECURE) != MIC | SECURE || key.info & ACK != 0 { return Err(Error::Unexpected); }
        if !eapol::verify(&ptk.kck, frame, &key) { return Err(Error::Mic); }
        Ok(())
    }

    /// Group message 1: a new GTK.
    pub fn group(&mut self, gtk: &Gtk, rsc: [u8; 8], out: &mut [u8; FRAME_MAX]) -> Result<usize, Error> {
        let mut plain = [0u8; 24];
        plain[..8].copy_from_slice(&[0xDD, 22, 0x00, 0x0F, 0xAC, 1, gtk.id & 3, 0]);
        plain[8..].copy_from_slice(&gtk.key);
        self.signed(out, VERSION_SHA1_AES | ACK | MIC | SECURE | ENCRYPTED, &[0; 32], &plain, rsc)
    }
}
