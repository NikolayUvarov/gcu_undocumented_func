//! The registration's proof of work (docs/hub/README.md, 5.4): a counter such that SHA-256 of a prefix, the hub's
//! challenge and the counter has at least `bits` leading zero bits. Off unless the hub asks for one; the hub raises `bits`
//! when registrations come fast.

use sha2::{Digest, Sha256};

const PREFIX: &[u8] = b"MIND Hub work 1\0";
/// The most the hub may ask: about 2^28 hashes on average, minutes on a slow board.
pub const BITS_MAX: u8 = 28;

fn hash(challenge: &[u8; 32], counter: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(PREFIX);
    h.update(challenge);
    h.update(counter.to_le_bytes());
    h.finalize().into()
}

/// Leading zero bits of a digest.
pub fn zero_bits(digest: &[u8; 32]) -> u32 {
    let mut bits = 0;
    for b in digest {
        if *b == 0 { bits += 8; continue; }
        return bits + b.leading_zeros();
    }
    bits
}

/// Whether `counter` answers `challenge` at `bits`; a `bits` above BITS_MAX is never asked, so never answered.
pub fn check(challenge: &[u8; 32], counter: u64, bits: u8) -> bool {
    bits <= BITS_MAX && zero_bits(&hash(challenge, counter)) >= bits as u32
}

/// The first counter from 0 that answers; None when `bits` is above BITS_MAX.
pub fn solve(challenge: &[u8; 32], bits: u8) -> Option<u64> {
    if bits > BITS_MAX { return None; }
    (0..u64::MAX).find(|c| check(challenge, *c, bits))
}
