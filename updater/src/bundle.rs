// A release bundle (751-UPD-0032): one ZIP of stored entries holding the layout of scripts/release.py (`release.py
// bundle`), read in place from the file the Effector gateway wrote into the updater's staging. Only the central directory
// is read into memory, and an entry is a range of that file. Its CRCs and dates are not read: every file of a release is
// checked by the signatures and SHA-256 digests the updater checks any source by. ZIP64, several disks, encryption,
// compression, a name that is not a plain relative path or that repeats, and a range outside the entries' part of the
// file are refused, never trimmed. No system calls: host-tested in tests/updater_host.rs.
use alloc::string::String;
use alloc::vec::Vec;

const END: u32 = 0x0605_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;
const END_LEN: usize = 22;
const CENTRAL_LEN: usize = 46;
/// A local header's fixed part; its name follows.
pub const LOCAL_LEN: usize = 30;
pub const ENTRIES_MAX: usize = 1024;
pub const NAME_MAX: usize = 255;
/// What `end` reads of a file's last bytes: the end record and the longest comment.
pub const TAIL_MAX: usize = END_LEN + 0xFFFF;

/// An entry of the central directory: its name, size and local header's offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry { pub name: String, pub size: u64, pub local: u64 }

fn u16_at(d: &[u8], at: usize) -> Option<u16> { d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])) }
fn u32_at(d: &[u8], at: usize) -> Option<u32> { d.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) }

/// The central directory's offset, size and count, from `tail`: the last bytes of a file of `len` bytes.
pub fn end(tail: &[u8], len: u64) -> Option<(u64, usize, usize)> {
    if tail.len() < END_LEN || tail.len() as u64 > len { return None; }
    let lowest = tail.len().saturating_sub(TAIL_MAX);
    let mut at = tail.len() - END_LEN;
    let at = loop {
        if u32_at(tail, at) == Some(END) && at + END_LEN + u16_at(tail, at + 20)? as usize == tail.len() { break at; }
        if at == lowest { return None; }
        at -= 1;
    };
    let (disk, central_disk, here, count) = (u16_at(tail, at + 4)?, u16_at(tail, at + 6)?, u16_at(tail, at + 8)?, u16_at(tail, at + 10)?);
    let (size, offset) = (u32_at(tail, at + 12)? as u64, u32_at(tail, at + 16)? as u64);
    if disk != 0 || central_disk != 0 || here != count || count == 0xFFFF || size == 0xFFFF_FFFF || offset == 0xFFFF_FFFF { return None; }
    if count as usize > ENTRIES_MAX || offset + size != len - (tail.len() - at) as u64 { return None; }
    Some((offset, size as usize, count as usize))
}

/// The `count` entries of central directory `data`, stored and not encrypted, each a plain relative name met once.
pub fn entries(data: &[u8], count: usize, directory: u64) -> Option<Vec<Entry>> {
    let mut found: Vec<Entry> = Vec::new();
    let mut at = 0;
    for _ in 0..count {
        if u32_at(data, at)? != CENTRAL { return None; }
        let (flags, method) = (u16_at(data, at + 8)?, u16_at(data, at + 10)?);
        let (compressed, size) = (u32_at(data, at + 20)?, u32_at(data, at + 24)?);
        let (name_len, extra_len, comment_len) = (u16_at(data, at + 28)? as usize, u16_at(data, at + 30)? as usize, u16_at(data, at + 32)? as usize);
        let (disk, local) = (u16_at(data, at + 34)?, u32_at(data, at + 42)? as u64);
        if flags & 0x41 != 0 || method != 0 || compressed != size || size == 0xFFFF_FFFF || local == 0xFFFF_FFFF || disk != 0 { return None; }
        let name = core::str::from_utf8(data.get(at + CENTRAL_LEN..at + CENTRAL_LEN + name_len)?).ok()?;
        if !super::zip::name_ok(name, NAME_MAX) || found.iter().any(|e| e.name == name) || local >= directory { return None; }
        found.push(Entry { name: String::from(name), size: size as u64, local });
        at += CENTRAL_LEN + name_len + extra_len + comment_len;
    }
    (at == data.len()).then_some(found)
}

/// Where `entry`'s bytes start, from its local header (`LOCAL_LEN` bytes and its name), if they end by `directory`.
pub fn start(header: &[u8], entry: &Entry, directory: u64) -> Option<u64> {
    let (flags, method) = (u16_at(header, 6)?, u16_at(header, 8)?);
    let (name_len, extra_len) = (u16_at(header, 26)? as usize, u16_at(header, 28)? as u64);
    if u32_at(header, 0)? != LOCAL || flags & 0x41 != 0 || method != 0 || header.get(LOCAL_LEN..LOCAL_LEN + name_len)? != entry.name.as_bytes() { return None; }
    let start = entry.local + (LOCAL_LEN + name_len) as u64 + extra_len;
    (start + entry.size <= directory).then_some(start)
}
