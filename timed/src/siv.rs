// AEAD_AES_SIV_CMAC_256 (RFC 5297) for NTS's NTP packets (111-NET-0075): S2V over each header in order, then the
// plaintext; NTS's headers are the associated data and the nonce. RustCrypto's `aes-siv`.
extern crate alloc;
use aes_siv::siv::Aes128Siv;
use aes_siv::KeyInit;
use alloc::vec::Vec;

/// The SIV and the ciphertext of `plain` under `key` (32 bytes: AES-128's two keys).
pub fn seal(key: &[u8; 32], headers: &[&[u8]], plain: &[u8]) -> Vec<u8> {
    Aes128Siv::new(key.into()).encrypt(headers, plain).expect("S2V takes at most 126 headers")
}

/// The plaintext, when the SIV leading `sealed` holds under `key`; None otherwise.
pub fn open(key: &[u8; 32], headers: &[&[u8]], sealed: &[u8]) -> Option<Vec<u8>> {
    Aes128Siv::new(key.into()).decrypt(headers, sealed).ok()
}
