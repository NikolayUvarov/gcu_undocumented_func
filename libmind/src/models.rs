//! A model disk's `MANIFEST.json` (251-APP-0009, 0010), read as the importer into the block store needs it
//! (251-STO-0014): how many models it lists, and of one model its id, where its entry lies in the text (to be kept with
//! the model whole, licence and terms included), and its files with their sizes and SHA-256. Read in the parser service
//! with `mind::json`; the importer checks every file against what it says as it reads it.
use crate::json::{self, Kind, Reader};

/// A model's id and a file's path: their longest.
pub const ID_MAX: usize = 64;
pub const PATH_MAX: usize = 96;
/// The largest manifest read.
pub const MANIFEST_MAX: usize = 60_000;

/// The text is not a model manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Malformed;
impl From<json::Malformed> for Malformed { fn from(_: json::Malformed) -> Self { Malformed } }

/// Bytes of a name: an id or a relative path whose parts are letters, digits, `.`, `_` and `-`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Name<const N: usize> { bytes: [u8; N], len: usize }
impl<const N: usize> Name<N> {
    pub const EMPTY: Self = Self { bytes: [0; N], len: 0 };
    pub fn as_bytes(&self) -> &[u8] { &self.bytes[..self.len] }
    pub fn as_str(&self) -> &str { core::str::from_utf8(self.as_bytes()).unwrap_or("") }
    // `text` if every part between slashes (`path`) or the whole (an id) is a word, and it fits.
    fn checked(text: &[u8], path: bool) -> Option<Self> {
        let word = |p: &[u8]| !p.is_empty() && p != b"." && p != b".." && p.iter().all(|&b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-');
        let ok = if path { text.split(|&b| b == b'/').all(word) } else { word(text) };
        if !ok || text.len() > N { return None; }
        let mut bytes = [0; N];
        bytes[..text.len()].copy_from_slice(text);
        Some(Self { bytes, len: text.len() })
    }
}

/// A file of a model, its path below the model's directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct File { pub path: Name<PATH_MAX>, pub size: u64, pub sha256: [u8; 32] }
impl File { pub const EMPTY: File = File { path: Name::EMPTY, size: 0, sha256: [0; 32] }; }

/// One model of the manifest: its id, its entry's bytes in the text (`start..end`), and its number of files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model { pub id: Name<ID_MAX>, pub start: usize, pub end: usize, pub files: usize }

fn sha256(text: &[u8]) -> Option<[u8; 32]> {
    if text.len() != 64 { return None; }
    let mut out = [0u8; 32];
    for (i, pair) in text.chunks_exact(2).enumerate() {
        let digit = |b: u8| match b { b'0'..=b'9' => Some(b - b'0'), b'a'..=b'f' => Some(b - b'a' + 10), _ => None };
        out[i] = digit(pair[0])? << 4 | digit(pair[1])?;
    }
    Some(out)
}

/// Reads the manifest whole and gives how many models it lists and model `index`, with its files from `first` copied
/// into `files` (as many as fit; the model says how many it has). The top object holds `models`, a list of objects, and
/// optionally `format`, which must be 1; a model needs `id` and `files`, each file `path`, `size` and `sha256`. Other
/// keys are passed over. Ids are unique; a model's paths are unique.
pub fn read(text: &[u8], index: usize, first: usize, files: &mut [File]) -> Result<(usize, Model, usize), Malformed> {
    if text.len() > MANIFEST_MAX { return Err(Malformed); }
    let mut r = Reader::new(text)?;
    let mut key = [0u8; 16];
    let (mut count, mut found, mut copied) = (0, None, 0);
    let mut seen_models = false;
    r.object()?;
    while let Some(k) = r.key(&mut key)? {
        match k {
            b"format" => { if r.integer()? != 1 { return Err(Malformed); } }
            b"models" if !seen_models => {
                seen_models = true;
                r.array()?;
                while r.item()? {
                    if r.peek()? != Kind::Object { return Err(Malformed); }
                    let start = r.position();
                    let (model, n) = read_model(&mut r, if count == index { Some((first, &mut *files)) } else { None })?;
                    if count == index { found = Some(Model { start, end: r.position(), ..model }); copied = n; }
                    count += 1;
                }
            }
            _ => r.skip()?,
        }
    }
    r.end()?;
    if !seen_models { return Err(Malformed); }
    let model = found.ok_or(Malformed)?;
    // Every other model's id differs from this one's: read again and compare (the manifest is small).
    let mut r = Reader::new(text)?;
    r.object()?;
    while let Some(k) = r.key(&mut key)? {
        if k != b"models" { r.skip()?; continue; }
        r.array()?;
        let mut i = 0;
        while r.item()? {
            let (other, _) = read_model(&mut r, None)?;
            if i != index && other.id == model.id { return Err(Malformed); }
            i += 1;
        }
    }
    Ok((count, model, copied))
}

// One model object; its files from `first` into `out` when given. `start` and `end` are the caller's.
fn read_model(r: &mut Reader, mut out: Option<(usize, &mut [File])>) -> Result<(Model, usize), Malformed> {
    let mut key = [0u8; 16];
    let mut text = [0u8; PATH_MAX];
    let (mut id, mut files, mut copied, mut have_files) = (None, 0, 0, false);
    // Paths seen, compared by a digest of each, to refuse a path twice (at most 64 files a model).
    let mut paths = [[0u8; 32]; 64];
    r.object()?;
    while let Some(k) = r.key(&mut key)? {
        match k {
            b"id" if id.is_none() => {
                let s = r.string(&mut text)?;
                id = Some(Name::checked(s, false).ok_or(Malformed)?);
            }
            b"files" if !have_files => {
                have_files = true;
                r.array()?;
                while r.item()? {
                    let file = read_file(r)?;
                    let tag = crate::sha256::digest(file.path.as_bytes());
                    if files == paths.len() || paths[..files].contains(&tag) { return Err(Malformed); }
                    paths[files] = tag;
                    if let Some((first, out)) = out.as_mut() {
                        if files >= *first && files - *first < out.len() { out[files - *first] = file; copied += 1; }
                    }
                    files += 1;
                }
            }
            _ => r.skip()?,
        }
    }
    let id = id.ok_or(Malformed)?;
    if !have_files { return Err(Malformed); }
    Ok((Model { id, start: 0, end: 0, files }, copied))
}

fn read_file(r: &mut Reader) -> Result<File, Malformed> {
    let mut key = [0u8; 16];
    let mut text = [0u8; PATH_MAX];
    let (mut path, mut size, mut digest) = (None, None, None);
    r.object()?;
    while let Some(k) = r.key(&mut key)? {
        match k {
            b"path" if path.is_none() => path = Some(Name::checked(r.string(&mut text)?, true).ok_or(Malformed)?),
            b"size" if size.is_none() => size = Some(r.integer()?),
            b"sha256" if digest.is_none() => digest = Some(sha256(r.string(&mut text)?).ok_or(Malformed)?),
            _ => r.skip()?,
        }
    }
    Ok(File { path: path.ok_or(Malformed)?, size: size.ok_or(Malformed)?, sha256: digest.ok_or(Malformed)? })
}
