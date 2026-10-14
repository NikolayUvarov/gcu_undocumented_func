//! `chacha20-poly1305@openssh.com` (OpenSSH's PROTOCOL.chacha20poly1305): a 64-byte key, K_2 its first half and K_1 its
//! second. The packet length is encrypted with K_1, the rest with K_2 from block 1; Poly1305, keyed by K_2's block 0,
//! authenticates both. The nonce is the packet's sequence number.
use crate::Error;
use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use chacha20::ChaCha20Legacy;
use poly1305::universal_hash::KeyInit;
use poly1305::Poly1305;

pub const TAG: usize = 16;

pub struct ChaChaPoly { main: [u8; 32], header: [u8; 32] }

impl Drop for ChaChaPoly {
    fn drop(&mut self) { self.main.fill(0); self.header.fill(0); }
}

impl ChaChaPoly {
    pub fn new(key: &[u8; 64]) -> Self {
        let (mut main, mut header) = ([0u8; 32], [0u8; 32]);
        main.copy_from_slice(&key[..32]);
        header.copy_from_slice(&key[32..]);
        ChaChaPoly { main, header }
    }

    fn stream(key: &[u8; 32], seq: u32) -> ChaCha20Legacy { ChaCha20Legacy::new(key.into(), &(seq as u64).to_be_bytes().into()) }

    fn mac(&self, seq: u32, data: &[u8]) -> [u8; TAG] {
        let mut key = [0u8; 32];
        Self::stream(&self.main, seq).apply_keystream(&mut key);
        let tag = Poly1305::new(&key.into()).compute_unpadded(data);
        key.fill(0);
        tag.into()
    }

    /// Encrypts `packet` (its 4-byte length first) in place and gives its tag.
    pub fn seal(&self, seq: u32, packet: &mut [u8]) -> [u8; TAG] {
        Self::stream(&self.header, seq).apply_keystream(&mut packet[..4]);
        let mut body = Self::stream(&self.main, seq);
        body.seek(64u64);
        body.apply_keystream(&mut packet[4..]);
        self.mac(seq, packet)
    }

    /// The length field of a received packet, decrypted.
    pub fn length(&self, seq: u32, encrypted: &[u8]) -> u32 {
        let mut length = [encrypted[0], encrypted[1], encrypted[2], encrypted[3]];
        Self::stream(&self.header, seq).apply_keystream(&mut length);
        u32::from_be_bytes(length)
    }

    /// Checks `tag` over `packet` (as received, its length first), then decrypts the packet after its length in place.
    pub fn open(&self, seq: u32, packet: &mut [u8], tag: &[u8]) -> Result<(), Error> {
        let expected = self.mac(seq, packet);
        if expected.iter().zip(tag).fold(0u8, |d, (a, b)| d | (a ^ b)) != 0 { return Err(Error::Mac); }
        let mut body = Self::stream(&self.main, seq);
        body.seek(64u64);
        body.apply_keystream(&mut packet[4..]);
        Ok(())
    }
}
