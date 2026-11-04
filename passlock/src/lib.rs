//! The device key's seed sealed under the owner's passphrase where no TPM seals it (308-NET-0055): the file
//! `system/keystore/device.locked` that `keystore` keeps. The file's key is Argon2id's (RFC 9106, version 0x13) of the
//! passphrase and a random salt, with the parameters recorded in the file; the seed is sealed under it with
//! ChaCha20-Poly1305, the header as associated data, so changed parameters do not open it. Argon2id's memory is the
//! caller's and is wiped after each derivation. No allocation, no I/O and no libmind, so its tests run on the host
//! (`cargo test`).
#![no_std]
#![forbid(unsafe_code)]

use argon2::{Algorithm, Argon2, AssociatedData, ParamsBuilder, Version};
use chacha20poly1305::aead::AeadInPlace;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce, Tag};
use zeroize::Zeroize;

pub use argon2::Block;

/// The file, in `keystore`'s private directory of the boot volume.
pub const FILE: &str = "system/keystore/device.locked";
pub const MAGIC: &[u8; 8] = b"MINDPWD1";
/// This build's version of the file; a file of another is refused by its number.
pub const VERSION: u8 = 1;
pub const SALT: usize = 16;
pub const NONCE: usize = 12;
pub const SEED: usize = 32;
const TAG: usize = 16;
const KEY: usize = 32;
/// The header: MAGIC, the version, the parameters (memory in KiB, passes, lanes: each a u32, little-endian), the salt
/// and the nonce. It is the sealed seed's associated data.
pub const HEADER: usize = MAGIC.len() + 1 + 3 * 4 + SALT + NONCE;
/// The whole file: the header, the seed sealed, its tag.
pub const LEN: usize = HEADER + SEED + TAG;
/// A passphrase's length in bytes (`keystore.wit` 1.6): one outside it is never sealed under, so it never opens.
pub const PASSPHRASE_MIN: usize = 8;
pub const PASSPHRASE_MAX: usize = 128;

/// Wrong passphrases in a row before the next try must wait.
pub const FREE_TRIES: u32 = 2;
const WAIT_FIRST_MS: u64 = 1000;
const WAIT_MAX_MS: u64 = 60_000;

/// The wait in milliseconds after `wrong` wrong passphrases in a row: none for the first two, then 1 s, doubling up to 60 s.
pub fn wait_ms(wrong: u32) -> u64 {
    if wrong <= FREE_TRIES { 0 } else { (WAIT_FIRST_MS << (wrong - FREE_TRIES - 1).min(6)).min(WAIT_MAX_MS) }
}

/// The wrong passphrases in a row, and when the next try may come on the caller's clock (milliseconds). A try before
/// then is refused at once, nothing derived, so no answer waits on a sleep; an opened passphrase starts the count again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tries { wrong: u32, next_ms: u64 }

impl Tries {
    /// The milliseconds left before a try may come at `now`, or None when it may.
    pub fn early(&self, now: u64) -> Option<u64> { (self.wrong > FREE_TRIES && now < self.next_ms).then(|| self.next_ms - now) }
    /// Counts a wrong passphrase at `now`: the wait before the next try.
    pub fn wrong(&mut self, now: u64) -> u64 {
        self.wrong = self.wrong.saturating_add(1);
        let wait = wait_ms(self.wrong);
        self.next_ms = now.saturating_add(wait);
        wait
    }
    pub fn opened(&mut self) { *self = Self::default(); }
    pub fn in_a_row(&self) -> u32 { self.wrong }
}

/// Argon2id's parameters, recorded in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params { pub memory_kib: u32, pub passes: u32, pub lanes: u32 }

impl Params {
    /// A new file's: OWASP's Argon2id row of 9 MiB and 4 passes (its equivalent of 19 MiB and 2), within a task's
    /// default quota of 16 MiB.
    pub const DEFAULT: Self = Self { memory_kib: 9216, passes: 4, lanes: 1 };
    /// The most this build derives with: a file asking more is refused as damaged, before any derivation.
    pub const MEMORY_KIB_MAX: u32 = 1 << 20;
    pub const PASSES_MAX: u32 = 32;
    pub const LANES_MAX: u32 = 16;

    fn argon2(self, data: &[u8], out: usize) -> Option<argon2::Params> {
        if self.memory_kib > Self::MEMORY_KIB_MAX || self.passes > Self::PASSES_MAX || self.lanes > Self::LANES_MAX { return None; }
        let data = AssociatedData::new(data).ok()?;
        ParamsBuilder::new().m_cost(self.memory_kib).t_cost(self.passes).p_cost(self.lanes).output_len(out).data(data).build().ok()
    }

    /// The 1 KiB blocks a derivation with these parameters needs; None for parameters this build does not derive with.
    pub fn blocks(self) -> Option<usize> { self.argon2(&[], KEY).map(|p| p.block_count()) }
}

/// Why a file did not open, or a seed was not sealed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The passphrase does not open the file; to the cipher a changed sealed part or salt is the same.
    Wrong,
    /// Not the file's form: another magic or length, or parameters this build does not derive with.
    Damaged,
    /// A file of another version, by its number.
    Version(u8),
    /// The memory given holds fewer blocks than the parameters need.
    Memory,
    /// A passphrase outside PASSPHRASE_MIN..=PASSPHRASE_MAX bytes: nothing derived.
    Passphrase,
}

/// A file's header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header { pub params: Params, pub salt: [u8; SALT], pub nonce: [u8; NONCE] }

impl Header {
    fn encode(&self) -> [u8; HEADER] {
        let mut out = [0u8; HEADER];
        out[..8].copy_from_slice(MAGIC);
        out[8] = VERSION;
        for (at, word) in [(9, self.params.memory_kib), (13, self.params.passes), (17, self.params.lanes)] { out[at..at + 4].copy_from_slice(&word.to_le_bytes()); }
        out[21..21 + SALT].copy_from_slice(&self.salt);
        out[21 + SALT..].copy_from_slice(&self.nonce);
        out
    }
}

/// The header of `file`, read without deriving anything: the version is read before the length, so that a file of
/// another version is refused by its number whatever its length.
pub fn header(file: &[u8]) -> Result<Header, Refused> {
    if file.len() <= MAGIC.len() || &file[..MAGIC.len()] != MAGIC { return Err(Refused::Damaged); }
    if file[8] != VERSION { return Err(Refused::Version(file[8])); }
    if file.len() != LEN { return Err(Refused::Damaged); }
    let word = |at: usize| u32::from_le_bytes([file[at], file[at + 1], file[at + 2], file[at + 3]]);
    let params = Params { memory_kib: word(9), passes: word(13), lanes: word(17) };
    params.blocks().ok_or(Refused::Damaged)?;
    let (mut salt, mut nonce) = ([0u8; SALT], [0u8; NONCE]);
    salt.copy_from_slice(&file[21..21 + SALT]);
    nonce.copy_from_slice(&file[21 + SALT..HEADER]);
    Ok(Header { params, salt, nonce })
}

/// Argon2id (RFC 9106, version 0x13) of `password` and `salt`, with the secret `secret` and the associated data `data`
/// (both empty for the file), into `out`, in `memory`, which is wiped after.
pub fn argon2id(password: &[u8], salt: &[u8], secret: &[u8], data: &[u8], params: Params, memory: &mut [Block], out: &mut [u8]) -> Result<(), Refused> {
    let derived = (|| {
        let built = params.argon2(data, out.len()).ok_or(Refused::Damaged)?;
        if memory.len() < built.block_count() { return Err(Refused::Memory); }
        let argon2 = Argon2::new_with_secret(secret, Algorithm::Argon2id, Version::V0x13, built).map_err(|_| Refused::Damaged)?;
        argon2.hash_password_into_with_memory(password, salt, out, &mut *memory).map_err(|_| Refused::Damaged)
    })();
    for block in memory.iter_mut() { block.zeroize(); }
    if derived.is_err() { out.zeroize(); }
    derived
}

// The file's cipher: under the key Argon2id derives from the passphrase and the header's salt.
fn cipher(passphrase: &[u8], header: &Header, memory: &mut [Block]) -> Result<ChaCha20Poly1305, Refused> {
    if !(PASSPHRASE_MIN..=PASSPHRASE_MAX).contains(&passphrase.len()) { return Err(Refused::Passphrase); }
    let mut key = [0u8; KEY];
    argon2id(passphrase, &header.salt, &[], &[], header.params, memory, &mut key)?;
    let cipher = ChaCha20Poly1305::new(&key.into());
    key.zeroize();
    Ok(cipher)
}

/// The file that seals `seed` under `passphrase`, with Argon2id's `params`, a random `salt` and a random `nonce` (the
/// caller's), in `memory`, which is wiped after.
pub fn seal(passphrase: &[u8], salt: &[u8; SALT], nonce: &[u8; NONCE], params: Params, seed: &[u8; SEED], memory: &mut [Block]) -> Result<[u8; LEN], Refused> {
    let header = Header { params, salt: *salt, nonce: *nonce };
    let cipher = cipher(passphrase, &header, memory)?;
    let mut file = [0u8; LEN];
    let (head, rest) = file.split_at_mut(HEADER);
    head.copy_from_slice(&header.encode());
    let (sealed, tag) = rest.split_at_mut(SEED);
    sealed.copy_from_slice(seed);
    match cipher.encrypt_in_place_detached(Nonce::from_slice(nonce), head, sealed) {
        Ok(made) => { tag.copy_from_slice(&made); Ok(file) }
        Err(_) => { file.zeroize(); Err(Refused::Damaged) }
    }
}

/// The seed `file` seals, opened with `passphrase` in `memory`, which is wiped after; the caller wipes the seed.
pub fn open(file: &[u8], passphrase: &[u8], memory: &mut [Block]) -> Result<[u8; SEED], Refused> {
    let header = header(file)?;
    let cipher = cipher(passphrase, &header, memory)?;
    let mut seed = [0u8; SEED];
    seed.copy_from_slice(&file[HEADER..HEADER + SEED]);
    match cipher.decrypt_in_place_detached(Nonce::from_slice(&header.nonce), &file[..HEADER], &mut seed, Tag::from_slice(&file[HEADER + SEED..])) {
        Ok(()) => Ok(seed),
        Err(_) => { seed.zeroize(); Err(Refused::Wrong) }
    }
}
