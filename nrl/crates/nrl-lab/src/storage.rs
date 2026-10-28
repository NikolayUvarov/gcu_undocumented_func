use crate::Result;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

#[rustfmt::skip]
#[path = "../../../../libmind/src/sha256.rs"]
mod sha256;

pub fn hash(bytes: &[u8]) -> String {
    sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub(crate) fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?
        .write_all(bytes)
        .map_err(|e| e.to_string())
}
pub(crate) fn read_regular(path: &Path, limit: usize) -> Result<Vec<u8>> {
    if !fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_file()
    {
        return Err("input must be a regular file, not a symlink".into());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("file byte limit exceeded".into());
    }
    Ok(bytes)
}

pub(crate) fn hash_file(path: &Path, limit: usize) -> Result<String> {
    if !fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("hash input is not a regular file".into());
    }
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = sha256::Sha256::new();
    let mut total = 0usize;
    let mut buffer = [0u8; 16384];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count)
            .filter(|n| *n <= limit)
            .ok_or("hash file byte limit")?;
        hash.update(&buffer[..count]);
    }
    Ok(hash.finish().iter().map(|b| format!("{b:02x}")).collect())
}
