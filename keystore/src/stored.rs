// The device key kept across boots (351-NET-0005), as an interim before a TPM seals it: the seed in the key service's
// private directory of the boot disk, which vfs_server lets only this service open. It is not encrypted: whoever has
// the disk itself reads it (docs/profile/threat-model.md).
use mind::fs::{self, File};
use mind::sha256;

pub const DIR: &str = "system/keystore";
pub const FILE: &str = "system/keystore/device.key";
const MAGIC: &[u8; 8] = b"MINDKEY1";
const LEN: usize = 8 + 32 + 32; // magic, seed, SHA-256 of both

pub enum Loaded { Seed([u8; 32]), Missing, Damaged, Unreadable(fs::Error) }

fn check(magic_and_seed: &[u8]) -> [u8; 32] { sha256::digest(magic_and_seed) }

pub fn load() -> Loaded {
    let file = match File::open(FILE) { Ok(file) => file, Err(fs::Error::NotFound) => return Loaded::Missing, Err(error) => return Loaded::Unreadable(error) };
    let mut data = [0u8; LEN + 1];
    let n = match file.read_at(0, &mut data) { Ok(n) => n, Err(error) => return Loaded::Unreadable(error) };
    let whole = n == LEN && &data[..8] == MAGIC && data[40..LEN] == check(&data[..40]);
    let mut seed = [0u8; 32];
    if whole { seed.copy_from_slice(&data[8..40]); }
    wipe(&mut data);
    if whole { Loaded::Seed(seed) } else { Loaded::Damaged }
}

/// Writes the seed, then flushes the volume.
pub fn store(seed: &[u8; 32]) -> Result<(), fs::Error> {
    fs::mkdir(DIR)?;
    let mut data = [0u8; LEN];
    data[..8].copy_from_slice(MAGIC);
    data[8..40].copy_from_slice(seed);
    let digest = check(&data[..40]);
    data[40..].copy_from_slice(&digest);
    let result = File::create(FILE).and_then(|mut file| { file.write_at(0, &data)?; file.flush() });
    wipe(&mut data);
    result
}

/// Overwrites a copy of key material.
pub fn wipe(bytes: &mut [u8]) { for byte in bytes.iter_mut() { unsafe { core::ptr::write_volatile(byte, 0) }; } }
