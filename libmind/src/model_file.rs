//! Model files (251): read whole into 8-byte words, so that a network file's weights can be read in place, and hashed
//! with SHA-256 on the way. A file on the model disk (`models:`) is used only if its hash is the one the disk's
//! MANIFEST.json lists for it among the files added to the disk (`scripts/models.py disk --add`; MC-4.2).
use crate::sha256::Sha256;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// A file in memory.
pub struct File { words: Vec<u64>, len: usize, sha256: [u8; 32] }

impl File {
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: the words hold `len` bytes of the file; a byte view of u64s is always valid.
        unsafe { core::slice::from_raw_parts(self.words.as_ptr() as *const u8, self.len) }
    }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    /// Its SHA-256 in lower-case hex.
    pub fn hex(&self) -> [u8; 64] {
        let mut hex = [0u8; 64];
        for (i, b) in self.sha256.iter().enumerate() { hex[2 * i] = b"0123456789abcdef"[(b >> 4) as usize]; hex[2 * i + 1] = b"0123456789abcdef"[(b & 15) as usize]; }
        hex
    }
}

/// Reads `path` whole; on models: refuses a file MANIFEST.json does not list with this hash.
pub fn read(path: &str) -> Result<File, String> {
    let mut file = crate::fs::File::open(path).map_err(|e| format!("{}: {:?}", path, e))?;
    let len = file.size();
    let mut words = vec![0u64; len.div_ceil(8)];
    // SAFETY: the slice covers the words' first `len` bytes.
    let bytes = unsafe { core::slice::from_raw_parts_mut(words.as_mut_ptr() as *mut u8, len) };
    let mut hash = Sha256::new();
    let mut at = 0;
    while at < len {
        let end = (at + crate::fs::CHUNK).min(len);
        match file.read(&mut bytes[at..end]) {
            Ok(0) => break,
            Ok(n) => { hash.update(&bytes[at..at + n]); at += n }
            Err(e) => return Err(format!("{}: {:?}", path, e)),
        }
    }
    if at < len { return Err(format!("{}: {} of {} bytes", path, at, len)); }
    let file = File { words, len, sha256: hash.finish() };
    if let Some(rest) = path.strip_prefix("models:") {
        match listed(rest) {
            Some(hex) if hex == file.hex() => {}
            Some(_) => return Err(format!("{}: its SHA-256 is not the one MANIFEST.json lists", path)),
            None => return Err(format!("{}: not in MANIFEST.json", path)),
        }
    }
    Ok(file)
}

// The SHA-256 models:MANIFEST.json lists for `path` among the files added to the disk.
fn listed(path: &str) -> Option<[u8; 64]> {
    let mut file = crate::fs::File::open("models:MANIFEST.json").ok()?;
    let mut text = vec![0u8; file.size().min(1 << 20)];
    let mut at = 0;
    while at < text.len() { match file.read(&mut text[at..]) { Ok(0) | Err(_) => break, Ok(n) => at += n } }
    let text = core::str::from_utf8(&text[..at]).ok()?;
    let added = &text[text.find("\"added\"")?..];
    let entry = &added[added.find(&format!("\"path\": \"{}\"", path))?..];
    let entry = &entry[..entry.find('}')?];
    let hex = entry[entry.find("\"sha256\": \"")? + 11..].get(..64)?;
    hex.as_bytes().try_into().ok()
}
