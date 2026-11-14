// The device key kept across boots in the key service's private directory of the boot disk, which vfs_server lets only
// this service open: sealed by the TPM where the machine has one (351-NET-0006), so the blob opens only in that TPM;
// else the seed itself, unencrypted (351-NET-0005), whoever has the disk reads it (docs/profile/threat-model.md), until
// the owner sets a passphrase: then sealed under it instead (lock.rs, 308-NET-0055).
use mind::fs::{self, File};
use mind::idl::tpm;
use mind::ipc::Endpoint;
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

/// Overwrites a decoded text that held a passphrase, its length and its bytes.
pub fn wipe_text<const N: usize>(text: &mut mind::idl::codec::Text<N>) { unsafe { core::ptr::write_volatile(text, Default::default()) }; }

// The sealed key: magic, the blob's length (u16), the blob the TPM service gave, the SHA-256 of all before.
pub const SEALED: &str = "system/keystore/device.sealed";
const SEALED_MAGIC: &[u8; 8] = b"MINDSEL1";
const BLOB_MAX: usize = 1024;
const TPM: Endpoint = Endpoint(4); // the TPM service's client with the seal badge (351-KRN-0043)

pub enum Unsealed { Seed([u8; 32]), Missing, Damaged, Refused, Failed, Unreadable(fs::Error) }

/// Moves a key or networks file that does not open to `<path>.before` (one set aside before goes), so that nothing is
/// written over it: it is the evidence, and may be the only copy (801-NET-0079). Never called on a boot on trial.
/// Whether it was moved.
pub fn set_aside(path: &str) -> bool {
    let mut to = alloc::string::String::from(path);
    to.push_str(".before");
    let replaced = fs::remove(to.as_str()).is_ok();
    let moved = fs::rename(path, to.as_str()).is_ok() && flush();
    if moved { mind::println!("[KEYSTORE] {} SET ASIDE AS {}{}", path, to, if replaced { ", IN PLACE OF ONE SET ASIDE BEFORE" } else { "" }); }
    else { mind::println!("[KEYSTORE] {} NOT SET ASIDE, SO NOTHING IS WRITTEN IN ITS PLACE", path); }
    moved
}

/// Whether a key sealed by a TPM is there, or may be: any answer but not-found (801-NET-0079).
pub fn sealed_present() -> bool { !matches!(File::open(SEALED), Err(fs::Error::NotFound)) }

/// Whether an unencrypted copy is there, or may be: any answer but not-found.
pub fn plain_present() -> bool { !matches!(File::open(FILE), Err(fs::Error::NotFound)) }

/// Whether the machine has a TPM the key service may seal with.
pub fn tpm() -> bool { matches!(tpm::info(TPM), Ok(Ok(_))) }

/// The sealed key, opened by the TPM.
pub fn unseal() -> Unsealed {
    let file = match File::open(SEALED) { Ok(file) => file, Err(fs::Error::NotFound) => return Unsealed::Missing, Err(error) => return Unsealed::Unreadable(error) };
    let mut data = [0u8; 10 + BLOB_MAX + 32 + 1];
    let n = match file.read_at(0, &mut data) { Ok(n) => n, Err(error) => return Unsealed::Unreadable(error) };
    let blob = u16::from_le_bytes([data[8], data[9]]) as usize;
    if n < 10 || &data[..8] != SEALED_MAGIC || blob > BLOB_MAX || n != 10 + blob + 32 || data[10 + blob..n] != check(&data[..10 + blob]) { return Unsealed::Damaged; }
    let mut secret = [0u8; 32];
    let result = match tpm::unseal(TPM, &data[10..10 + blob], &mut secret) {
        Ok(Ok(32)) => Unsealed::Seed(secret),
        Ok(Ok(_)) => Unsealed::Damaged,
        // The TPM refused the blob: it is not this TPM's. Anything else may pass: the blob stays.
        Ok(Err(tpm::Error::Refused)) => Unsealed::Refused,
        Ok(Err(_)) | Err(_) => Unsealed::Failed,
    };
    wipe(&mut secret);
    result
}

/// Seals the seed with the TPM and writes the blob, then flushes the volume.
pub fn seal(seed: &[u8; 32]) -> Result<(), &'static str> {
    let mut data = [0u8; 10 + BLOB_MAX + 32];
    let blob = match tpm::seal(TPM, seed, &mut data[10..10 + BLOB_MAX]) { Ok(Ok(n)) => n, _ => return Err("THE TPM DID NOT SEAL IT") };
    data[..8].copy_from_slice(SEALED_MAGIC);
    data[8..10].copy_from_slice(&(blob as u16).to_le_bytes());
    let digest = check(&data[..10 + blob]);
    data[10 + blob..10 + blob + 32].copy_from_slice(&digest);
    fs::mkdir(DIR).map_err(|_| "NO DIRECTORY")?;
    File::create(SEALED).and_then(|mut file| { file.write_at(0, &data[..10 + blob + 32])?; file.flush() }).map_err(|_| "NOT WRITTEN")
}

/// Flushes the boot volume: vfs_server keeps a removal or a rename in its cache until then.
pub fn flush() -> bool { fs::Dir::open(DIR).and_then(|dir| dir.flush()).is_ok() }

/// Removes the unencrypted copy, once the key is sealed: zeroed and flushed first, so its clusters do not keep the seed
/// (a flash medium's own remapping may still), then removed and the volume flushed. NotFound when there is none.
pub fn remove_plain() -> Result<(), fs::Error> {
    if let Ok(mut file) = File::open_mode(FILE, fs::MODE_WRITE) { let _ = file.write_at(0, &[0u8; LEN]).and_then(|_| file.flush()); }
    fs::remove(FILE)?;
    flush();
    Ok(())
}
