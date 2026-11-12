//! A model as one object of the block store, named `models/<id>` (issues 251-STO-0014, 0015; docs/storage, "Speech
//! models in the store"): its header, then its files' bytes one after another in the header's order,
//!   MIND-MODEL 1\n  id <id>\n  entry <n>\n<the manifest's entry, n bytes>\n  file <size> <sha256> <path>\n ...  data\n
//! the entry keeping the licence and the terms with the model. A reader finds a file by its path in the header and reads
//! it whole, checked against its SHA-256 (MC-4.2), into memory the caller gives: a recognizer's own, or a memory object
//! it then seals (`blocks models map`). No heap and no system calls: tests/model_object_host.rs.
use crate::cid::Cid;
use crate::dag::{self, Blocks, CHUNK};
use crate::sha256::Sha256;

/// Bytes of a header at most.
pub const HEADER_MAX: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The store refused or failed a read, or the object is not of the shape its root says: the read's own error.
    Store(dag::Error),
    /// No header of this format before its `data` line within HEADER_MAX bytes, or the object ends before it.
    Header,
    /// The header names no file of that path.
    NoFile,
    /// The object ends before the file does.
    Short,
    /// The file's bytes are not the SHA-256 its header gives.
    Digest,
    /// A buffer that is not the file's size.
    Size,
}

/// A file of a model object: where its bytes start in the object, its size and its SHA-256.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Located { pub at: u64, pub size: u64, pub sha256: [u8; 32] }

/// Where the file lines start: after the format, id and entry lines and the entry's bytes.
pub fn files_start(header: &[u8]) -> Option<usize> {
    if !header.starts_with(b"MIND-MODEL 1\nid ") { return None; }
    let at = header.windows(7).position(|w| w == b"\nentry ")? + 7;
    let n_end = at + header[at..].iter().position(|&b| b == b'\n')?;
    let entry: usize = core::str::from_utf8(&header[at..n_end]).ok()?.parse().ok()?;
    let end = n_end.checked_add(1)?.checked_add(entry)?;
    (header.get(end) == Some(&b'\n')).then_some(end + 1)
}

/// Where the files' bytes start: just after the `data` line that follows the file lines.
pub fn find_data(header: &[u8]) -> Option<usize> {
    let mut at = files_start(header)?;
    loop {
        let line_end = at + header.get(at..)?.iter().position(|&b| b == b'\n')?;
        if &header[at..line_end] == b"data" { return Some(line_end + 1); }
        at = line_end + 1;
    }
}

/// The file `path`'s offset from the start of the files' bytes, its size and its SHA-256, by the header's file lines in
/// order; none if no line names it, or a line before it is not a file line.
pub fn locate(header: &[u8], path: &str) -> Option<(u64, u64, [u8; 32])> {
    let text = core::str::from_utf8(&header[files_start(header)?..]).ok()?;
    let mut offset = 0u64;
    for line in text.lines().take_while(|&l| l != "data") {
        let mut parts = line.strip_prefix("file ")?.splitn(3, ' ');
        let (size, digest, p) = (parts.next()?.parse::<u64>().ok()?, parts.next()?, parts.next()?);
        if digest.len() != 64 { return None; }
        if p == path {
            let mut out = [0u8; 32];
            for (i, pair) in digest.as_bytes().chunks_exact(2).enumerate() { out[i] = u8::from_str_radix(core::str::from_utf8(pair).ok()?, 16).ok()?; }
            return Some((offset, size, out));
        }
        offset = offset.checked_add(size)?;
    }
    None
}

/// The file `path` of the model object `root`: its header read into `header` (HEADER_MAX bytes are enough) until its
/// `data` line, every chunk through `buffer` and checked against its CID.
pub fn find<B: Blocks>(blocks: &mut B, root: &Cid, path: &str, header: &mut [u8], buffer: &mut [u8; CHUNK]) -> Result<Located, Error> {
    let mut have = 0;
    let data = loop {
        if have == header.len() { return Err(Error::Header); }
        let k = dag::read_at(blocks, root, have as u64, &mut header[have..], buffer).map_err(Error::Store)?;
        if k == 0 { return Err(Error::Header); }
        have += k;
        if let Some(at) = find_data(&header[..have]) { break at; }
    };
    let (offset, size, sha256) = locate(&header[..data], path).ok_or(Error::NoFile)?;
    Ok(Located { at: data as u64 + offset, size, sha256 })
}

/// Reads the file `file` of the model object `root` whole into `out`, its size exactly, every chunk through `buffer`
/// and checked against its CID, and checks the bytes against the SHA-256 its header gives.
pub fn read<B: Blocks>(blocks: &mut B, root: &Cid, file: &Located, out: &mut [u8], buffer: &mut [u8; CHUNK]) -> Result<(), Error> {
    if out.len() as u64 != file.size { return Err(Error::Size); }
    let mut done = 0;
    while done < out.len() {
        let k = dag::read_at(blocks, root, file.at + done as u64, &mut out[done..], buffer).map_err(Error::Store)?;
        if k == 0 { return Err(Error::Short); }
        done += k;
    }
    let mut hash = Sha256::new();
    hash.update(out);
    if hash.finish() != file.sha256 { return Err(Error::Digest); }
    Ok(())
}

