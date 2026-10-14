//! The transport (RFC 4253): the version exchange, packets, and one key exchange, curve25519-sha256 (RFC 8731) with an
//! ssh-ed25519 host key (RFC 8709) that must match the caller's pin, then chacha20-poly1305@openssh.com both ways. The
//! client asks for OpenSSH's strict key exchange (kex-strict-c-v00@openssh.com, against CVE-2023-48795): with a server
//! that offers it, nothing but the key exchange's own messages may come before NEWKEYS, and both sequence numbers start
//! again from 0 after it.
use crate::cipher::{ChaChaPoly, TAG};
use crate::wire::{has, Reader, Writer};
use crate::{Error, Io};
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

pub const VERSION: &[u8] = b"SSH-2.0-MIND_Core";
/// The largest packet taken, as RFC 4253 section 6.1 asks every implementation to take.
pub const PACKET_MAX: usize = 35000;
const LINE_MAX: usize = 255;

pub const DISCONNECT: u8 = 1;
pub const IGNORE: u8 = 2;
pub const UNIMPLEMENTED: u8 = 3;
pub const DEBUG: u8 = 4;
pub const KEXINIT: u8 = 20;
pub const NEWKEYS: u8 = 21;
pub const KEX_ECDH_INIT: u8 = 30;
pub const KEX_ECDH_REPLY: u8 = 31;
pub const GLOBAL_REQUEST: u8 = 80;
pub const REQUEST_FAILURE: u8 = 82;

const KEX: &[u8] = b"curve25519-sha256";
const KEX_LIBSSH: &[u8] = b"curve25519-sha256@libssh.org";
const STRICT_C: &[u8] = b"kex-strict-c-v00@openssh.com";
const STRICT_S: &[u8] = b"kex-strict-s-v00@openssh.com";
const HOST_KEY: &[u8] = b"ssh-ed25519";
const CIPHER: &[u8] = b"chacha20-poly1305@openssh.com";
// Not used with an AEAD cipher, but the lists must still agree on one.
const MAC: &[u8] = b"hmac-sha2-256";
const NONE: &[u8] = b"none";

pub struct Transport<I: Io> {
    io: I,
    // Received bytes not yet taken as a packet.
    input: Vec<u8>,
    seq_out: u32,
    seq_in: u32,
    sealing: Option<ChaChaPoly>,
    opening: Option<ChaChaPoly>,
    pub session_id: [u8; 32],
    /// The SHA-256 of the server's host key blob.
    pub host_key: [u8; 32],
}

impl<I: Io> Transport<I> {
    fn fill(&mut self, at_least: usize) -> Result<(), Error> {
        let mut buffer = [0u8; 4096];
        while self.input.len() < at_least {
            let n = self.io.receive(&mut buffer)?;
            if n == 0 { return Err(Error::Closed); }
            self.input.extend_from_slice(&buffer[..n]);
        }
        Ok(())
    }

    // The server's identification line, the lines before it skipped (RFC 4253 section 4.2).
    fn version(&mut self) -> Result<Vec<u8>, Error> {
        for _ in 0..32 {
            let line = loop {
                if let Some(end) = self.input.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = self.input.drain(..=end).collect();
                    break line;
                }
                if self.input.len() > LINE_MAX { return Err(Error::Protocol); }
                let have = self.input.len();
                self.fill(have + 1)?;
            };
            let line = line.strip_suffix(b"\n").unwrap_or(&line);
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.starts_with(b"SSH-2.0-") { return Ok(line.to_vec()); }
            if line.starts_with(b"SSH-") { return Err(Error::Algorithms); }
        }
        Err(Error::Protocol)
    }

    /// Sends one packet: padded to a multiple of 8 (without the length field once encrypted), padding random.
    pub fn send(&mut self, payload: &[u8], random: &mut dyn FnMut(&mut [u8]) -> bool) -> Result<(), Error> {
        let sealed = self.sealing.is_some();
        let unpadded = 1 + payload.len() + if sealed { 0 } else { 4 };
        let mut pad = 8 - unpadded % 8;
        if pad < 4 { pad += 8; }
        let length = 1 + payload.len() + pad;
        let mut packet = Vec::with_capacity(4 + length + TAG);
        packet.extend_from_slice(&(length as u32).to_be_bytes());
        packet.push(pad as u8);
        packet.extend_from_slice(payload);
        let mut padding = [0u8; 16];
        if !random(&mut padding[..pad]) { return Err(Error::Random); }
        packet.extend_from_slice(&padding[..pad]);
        if let Some(cipher) = &self.sealing {
            let tag = cipher.seal(self.seq_out, &mut packet);
            packet.extend_from_slice(&tag);
        }
        self.seq_out = self.seq_out.wrapping_add(1);
        self.io.send(&packet)
    }

    /// The next packet's payload.
    pub fn receive(&mut self) -> Result<Vec<u8>, Error> {
        self.fill(4)?;
        let length = match &self.opening { Some(cipher) => cipher.length(self.seq_in, &self.input[..4]), None => u32::from_be_bytes([self.input[0], self.input[1], self.input[2], self.input[3]]) } as usize;
        if !(5..=PACKET_MAX).contains(&length) || (self.opening.is_some() && length % 8 != 0) { return Err(Error::Protocol); }
        let tag = if self.opening.is_some() { TAG } else { 0 };
        self.fill(4 + length + tag)?;
        let mut packet: Vec<u8> = self.input.drain(..4 + length + tag).collect();
        if let Some(cipher) = &self.opening {
            let (body, mac) = packet.split_at_mut(4 + length);
            cipher.open(self.seq_in, body, mac)?;
        }
        self.seq_in = self.seq_in.wrapping_add(1);
        let pad = packet[4] as usize;
        if pad < 4 || pad + 1 >= length { return Err(Error::Protocol); }
        Ok(packet[5..4 + length - pad].to_vec())
    }

    /// The next packet that is not transport chatter: IGNORE, DEBUG and UNIMPLEMENTED are skipped, a global request
    /// that wants a reply is refused, and DISCONNECT ends the session.
    pub fn next(&mut self, random: &mut dyn FnMut(&mut [u8]) -> bool) -> Result<Vec<u8>, Error> {
        loop {
            let payload = self.receive()?;
            match payload.first() {
                Some(&IGNORE) | Some(&DEBUG) | Some(&UNIMPLEMENTED) => continue,
                Some(&DISCONNECT) => return Err(Error::Disconnected),
                Some(&GLOBAL_REQUEST) => {
                    let mut r = Reader::new(&payload[1..]);
                    let _name = r.string()?;
                    if r.bool()? { self.send(&[REQUEST_FAILURE], random)?; }
                    continue;
                }
                Some(&KEXINIT) => return Err(Error::Rekey),
                Some(_) => return Ok(payload),
                None => return Err(Error::Protocol),
            }
        }
    }

    /// The version exchange and the key exchange, the server known by `pin` (the SHA-256 of its host key blob).
    pub fn connect(mut io: I, pin: &[u8; 32], random: &mut dyn FnMut(&mut [u8]) -> bool) -> Result<Self, Error> {
        let mut ours = Vec::from(VERSION);
        ours.extend_from_slice(b"\r\n");
        io.send(&ours)?;
        let mut t = Transport { io, input: Vec::new(), seq_out: 0, seq_in: 0, sealing: None, opening: None, session_id: [0; 32], host_key: [0; 32] };
        let theirs = t.version()?;

        let mut cookie = [0u8; 16];
        if !random(&mut cookie) { return Err(Error::Random); }
        let mut kexinit = Writer::new(KEXINIT);
        kexinit.raw(&cookie);
        let mut kex = Vec::from(KEX);
        kex.extend_from_slice(b",");
        kex.extend_from_slice(KEX_LIBSSH);
        kex.extend_from_slice(b",");
        kex.extend_from_slice(STRICT_C);
        for list in [&kex[..], HOST_KEY, CIPHER, CIPHER, MAC, MAC, NONE, NONE, b"", b""] { kexinit.string(list); }
        kexinit.bool(false).u32(0);
        let i_c = kexinit.data;
        t.send(&i_c, random)?;

        // The server's KEXINIT must be its first packet when the exchange is strict, so it is taken as the first here.
        let i_s = t.receive()?;
        if i_s.first() != Some(&KEXINIT) || i_s.len() < 17 { return Err(Error::Protocol); }
        let mut r = Reader::new(&i_s[17..]);
        let lists: [&[u8]; 8] = [r.string()?, r.string()?, r.string()?, r.string()?, r.string()?, r.string()?, r.string()?, r.string()?];
        let strict = has(lists[0], STRICT_S);
        if !(has(lists[0], KEX) || has(lists[0], KEX_LIBSSH)) || !has(lists[1], HOST_KEY) || !has(lists[2], CIPHER) || !has(lists[3], CIPHER)
            || !has(lists[4], MAC) || !has(lists[5], MAC) || !has(lists[6], NONE) || !has(lists[7], NONE) { return Err(Error::Algorithms); }
        let _ = (r.string()?, r.string()?);
        if r.bool()? { return Err(Error::Algorithms); } // a guessed first packet, which this client never wants

        let mut seed = [0u8; 32];
        if !random(&mut seed) { return Err(Error::Random); }
        let secret = x25519_dalek::StaticSecret::from(seed);
        seed.fill(0);
        let q_c = x25519_dalek::PublicKey::from(&secret);
        let mut init = Writer::new(KEX_ECDH_INIT);
        init.string(q_c.as_bytes());
        t.send(&init.data, random)?;

        let reply = loop {
            let p = t.receive()?;
            match p.first() {
                Some(&KEX_ECDH_REPLY) => break p,
                Some(&IGNORE) | Some(&DEBUG) if !strict => continue,
                _ => return Err(Error::Protocol),
            }
        };
        let mut r = Reader::new(&reply[1..]);
        let k_s = r.string()?;
        let q_s = r.string()?;
        let signature = r.string()?;
        let host_key: [u8; 32] = Sha256::digest(k_s).into();
        t.host_key = host_key;
        if host_key != *pin { return Err(Error::HostKey); }
        let mut key = Reader::new(k_s);
        if key.string()? != HOST_KEY { return Err(Error::HostKey); }
        let public: [u8; 32] = key.string()?.try_into().map_err(|_| Error::HostKey)?;
        let q_s: [u8; 32] = q_s.try_into().map_err(|_| Error::Protocol)?;
        let shared = secret.diffie_hellman(&x25519_dalek::PublicKey::from(q_s));
        if !shared.was_contributory() { return Err(Error::Protocol); }
        let mut k = Writer::default();
        k.mpint(shared.as_bytes());

        let mut h = Writer::default();
        h.string(VERSION).string(&theirs).string(&i_c).string(&i_s).string(k_s).string(q_c.as_bytes()).string(&q_s).raw(&k.data);
        let h: [u8; 32] = Sha256::digest(&h.data).into();
        let mut sig = Reader::new(signature);
        if sig.string()? != HOST_KEY { return Err(Error::Signature); }
        let sig: [u8; 64] = sig.string()?.try_into().map_err(|_| Error::Signature)?;
        let verifying = ed25519_dalek::VerifyingKey::from_bytes(&public).map_err(|_| Error::Signature)?;
        verifying.verify_strict(&h, &ed25519_dalek::Signature::from_bytes(&sig)).map_err(|_| Error::Signature)?;
        t.session_id = h;

        t.send(&[NEWKEYS], random)?;
        t.sealing = Some(ChaChaPoly::new(&derive(&k.data, &h, b'C')));
        if strict { t.seq_out = 0; }
        let newkeys = t.receive()?;
        if newkeys.first() != Some(&NEWKEYS) { return Err(Error::Protocol); }
        t.opening = Some(ChaChaPoly::new(&derive(&k.data, &h, b'D')));
        if strict { t.seq_in = 0; }
        k.data.fill(0);
        Ok(t)
    }
}

// The 64 bytes of a direction's key (RFC 4253 section 7.2): HASH(K || H || letter || session_id), then extended.
fn derive(k: &[u8], h: &[u8; 32], letter: u8) -> [u8; 64] {
    let mut first = Sha256::new();
    first.update(k);
    first.update(h);
    first.update([letter]);
    first.update(h);
    let first: [u8; 32] = first.finalize().into();
    let mut second = Sha256::new();
    second.update(k);
    second.update(h);
    second.update(first);
    let second: [u8; 32] = second.finalize().into();
    let mut key = [0u8; 64];
    key[..32].copy_from_slice(&first);
    key[32..].copy_from_slice(&second);
    key
}
