//! The supplicant's side of the 4-way handshake and of the group key handshake (12.7.6, 12.7.7) for WPA2-PSK with
//! CCMP-128. Keys are handed out once: a repeated message 3 or group message is answered without installing its key
//! again, so a replayed handshake cannot reset the receive and transmit counters (key reinstallation).
use crate::eapol::{self, Gtk, Key, ACK, ENCRYPTED, INSTALL, KEY_DATA_MAX, MIC, PAIRWISE, SECURE, VERSION_SHA1_AES};
use crate::keys::{self, Ptk};
use crate::Error;

/// The longest RSN element kept (an element is at most 257 bytes; a PSK network's is about 20).
pub const RSNE_MAX: usize = 64;
/// The longest answer: the fixed fields and the station's RSN element.
pub const REPLY_MAX: usize = eapol::FIXED + RSNE_MAX;

/// What one received EAPOL-Key frame gave: an answer to send (EAPOL frame), and keys to install now.
#[derive(Debug, Default)]
pub struct Step { pub reply: usize, pub pairwise: Option<[u8; 16]>, pub group: Option<(Gtk, [u8; 8])> }

pub struct Supplicant {
    pmk: [u8; 32],
    own: [u8; 6],
    ap: [u8; 6],
    /// The RSN element the station sent in its association request, and the one the access point advertised.
    own_rsne: ([u8; RSNE_MAX], usize),
    ap_rsne: ([u8; RSNE_MAX], usize),
    /// The highest replay counter of a frame whose MIC was good (or of the first message 1).
    replay: Option<u64>,
    anonce: Option<[u8; 32]>,
    snonce: [u8; 32],
    /// The keys of the last message 1, and those message 3 confirmed and installed.
    tptk: Option<Ptk>,
    ptk: Option<Ptk>,
    installed_gtk: Option<Gtk>,
}

fn kept(element: &[u8]) -> Result<([u8; RSNE_MAX], usize), Error> {
    if element.len() < 2 || element[0] != 0x30 || element.len() != 2 + element[1] as usize || element.len() > RSNE_MAX { return Err(Error::Malformed); }
    let mut copy = [0u8; RSNE_MAX];
    copy[..element.len()].copy_from_slice(element);
    Ok((copy, element.len()))
}

impl Supplicant {
    /// A handshake with access point `ap`: `own_rsne` is the RSN element of the association request, `ap_rsne` the one of
    /// its beacon or probe response.
    pub fn new(pmk: [u8; 32], own: [u8; 6], ap: [u8; 6], own_rsne: &[u8], ap_rsne: &[u8]) -> Result<Self, Error> {
        Ok(Supplicant { pmk, own, ap, own_rsne: kept(own_rsne)?, ap_rsne: kept(ap_rsne)?, replay: None, anonce: None, snonce: [0; 32],
                        tptk: None, ptk: None, installed_gtk: None })
    }

    /// The pairwise keys once message 3 was good.
    pub fn ptk(&self) -> Option<&Ptk> { self.ptk.as_ref() }

    /// Takes one EAPOL-Key frame from the access point; `random` is a fresh 32-byte value, used as the SNonce when the
    /// frame is a new message 1. The answer, if any, is in `out[..step.reply]`.
    pub fn receive(&mut self, frame: &[u8], random: &[u8; 32], out: &mut [u8; REPLY_MAX]) -> Result<Step, Error> {
        let key = eapol::parse(frame)?;
        if key.info & ACK == 0 || key.info & (eapol::REQUEST | eapol::ERROR) != 0 { return Err(Error::Unexpected); }
        if self.replay.is_some_and(|last| key.replay <= last) { return Err(Error::Replay); }
        match (key.info & PAIRWISE != 0, key.info & MIC != 0) {
            (true, false) => self.message_1(&key, random, out),
            (true, true) => self.message_3(frame, &key, out),
            (false, true) => self.group(frame, &key, out),
            (false, false) => Err(Error::Unexpected),
        }
    }

    // Message 1: the ANonce; answered with the SNonce and the station's RSN element, under a MIC of the new PTK.
    fn message_1(&mut self, key: &Key, random: &[u8; 32], out: &mut [u8; REPLY_MAX]) -> Result<Step, Error> {
        if key.info & (INSTALL | ENCRYPTED) != 0 { return Err(Error::Malformed); }
        // A repeated message 1 with the same ANonce keeps the SNonce, so a late message 3 for it still verifies.
        if self.anonce != Some(key.nonce) { self.snonce = *random; }
        self.anonce = Some(key.nonce);
        let ptk = keys::ptk(&self.pmk, &self.ap, &self.own, &key.nonce, &self.snonce);
        let len = eapol::write(out, key.version, VERSION_SHA1_AES | PAIRWISE | MIC, key.replay, &self.snonce, &self.own_rsne.0[..self.own_rsne.1])?;
        eapol::sign(&ptk.kck, &mut out[..len])?;
        // Not the replay counter's floor yet: message 1 has no MIC, so it may be forged (12.7.6.2).
        self.tptk = Some(ptk);
        Ok(Step { reply: len, ..Step::default() })
    }

    // Message 3: the same ANonce under a good MIC, the advertised RSN element and the GTK; answered with message 4.
    fn message_3(&mut self, frame: &[u8], key: &Key, out: &mut [u8; REPLY_MAX]) -> Result<Step, Error> {
        let required = INSTALL | SECURE | ENCRYPTED;
        if key.info & required != required { return Err(Error::Malformed); }
        if self.anonce != Some(key.nonce) { return Err(Error::Unexpected); }
        let ptk = self.tptk.clone().ok_or(Error::Unexpected)?;
        if !eapol::verify(&ptk.kck, frame, key) { return Err(Error::Mic); }
        self.replay = Some(key.replay);
        let mut plain = [0u8; KEY_DATA_MAX];
        let found = eapol::elements(eapol::unwrap(&ptk.kek, key.data, &mut plain)?)?;
        if found.rsne != Some(&self.ap_rsne.0[..self.ap_rsne.1]) { return Err(Error::Downgrade); }
        let gtk = found.gtk.ok_or(Error::KeyData)?;
        let len = eapol::write(out, key.version, VERSION_SHA1_AES | PAIRWISE | MIC | SECURE, key.replay, &[0; 32], &[])?;
        eapol::sign(&ptk.kck, &mut out[..len])?;
        let pairwise = (self.ptk.as_ref().map(|p| p.tk) != Some(ptk.tk)).then_some(ptk.tk);
        self.ptk = Some(ptk);
        let group = self.take_group(gtk, key.rsc);
        Ok(Step { reply: len, pairwise, group })
    }

    // A group key message: a new GTK under the PTK's MIC; answered with group message 2.
    fn group(&mut self, frame: &[u8], key: &Key, out: &mut [u8; REPLY_MAX]) -> Result<Step, Error> {
        let required = SECURE | ENCRYPTED;
        if key.info & required != required || key.info & INSTALL != 0 { return Err(Error::Malformed); }
        let ptk = self.ptk().cloned().ok_or(Error::Unexpected)?;
        if !eapol::verify(&ptk.kck, frame, key) { return Err(Error::Mic); }
        self.replay = Some(key.replay);
        let mut plain = [0u8; KEY_DATA_MAX];
        let gtk = eapol::elements(eapol::unwrap(&ptk.kek, key.data, &mut plain)?)?.gtk.ok_or(Error::KeyData)?;
        let len = eapol::write(out, key.version, VERSION_SHA1_AES | MIC | SECURE, key.replay, &[0; 32], &[])?;
        eapol::sign(&ptk.kck, &mut out[..len])?;
        Ok(Step { reply: len, pairwise: None, group: self.take_group(gtk, key.rsc) })
    }

    fn take_group(&mut self, gtk: Gtk, rsc: [u8; 8]) -> Option<(Gtk, [u8; 8])> {
        let new = self.installed_gtk != Some(gtk);
        self.installed_gtk = Some(gtk);
        new.then_some((gtk, rsc))
    }
}
