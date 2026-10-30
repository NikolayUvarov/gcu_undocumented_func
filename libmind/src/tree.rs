//! Directories as objects (issue 311-STO-0027; MC-4.2, 4.5, 4.13): a directory is one DAG-CBOR block of the form
//! `{"v": 1, "entries": [[name, kind, size, link], ...]}`, its entries sorted by the names' bytes, each naming a file (an
//! object of `dag`) or another directory by its CID. A walk goes through every block under a root, checking each
//! directory against its CID and the schema and each entry against the object it names. No heap and no system calls:
//! tests/tree_host.rs.
use crate::cid::{Cid, Codec};
use crate::dag::{self, fetch, Blocks, Error, In, Out, Same, Sink, CHUNK, LINK};

/// The directory schema's version (`"v"`); another is refused (MC-4.13).
pub const VERSION: u64 = 1;
/// Bytes of a name at most.
pub const NAME_MAX: usize = 255;
// The least bytes of an entry: its array, a name of one byte, kind, size and link.
const ENTRY_MIN: usize = 1 + 2 + 1 + 1 + LINK;
// Entries of a directory at most: as many of the least as fit a block.
const ENTRIES: usize = CHUNK / ENTRY_MIN;
/// Directories on a path down from a root at most, the root's included.
pub const DEPTH: usize = 32;

/// What an entry names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An object of `dag`: a `raw` block, or a node over chunks.
    File,
    /// A directory node.
    Directory,
}

impl Kind {
    const fn code(self) -> u64 { match self { Kind::File => 0, Kind::Directory => 1 } }
    const fn from_code(code: u64) -> Option<Self> { match code { 0 => Some(Kind::File), 1 => Some(Kind::Directory), _ => None } }
}

/// An entry of a directory: a name, what it names, its size (a file's bytes, a directory's entries) and its CID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry<'a> { pub name: &'a [u8], pub kind: Kind, pub size: u64, pub cid: Cid }

/// Whether `name` may name an entry: 1 to NAME_MAX bytes of UTF-8 without `/` or NUL, neither `.` nor `..`.
pub fn valid_name(name: &[u8]) -> bool {
    (1..=NAME_MAX).contains(&name.len()) && core::str::from_utf8(name).is_ok() && !name.iter().any(|&b| b == b'/' || b == 0)
        && name != b"." && name != b".."
}

impl Entry<'_> {
    // A valid name; a directory linked as `dag-cbor`.
    fn valid(&self) -> bool { valid_name(self.name) && (self.kind == Kind::File || self.cid.codec() == Codec::DagCbor) }
}

// Counts the bytes of an encoding, so a directory over a block is refused before anything is written.
struct Count(usize);
impl Sink for Count {
    fn push(&mut self, data: &[u8]) { self.0 += data.len(); }
}

/// Encodes a directory of `entries`, sorted by name with each name once, into `out`; returns its length. Entries that
/// make no valid directory are refused (Format), and so is a directory over a block (TooLarge).
pub fn encode(entries: &[Entry], out: &mut [u8; CHUNK]) -> Result<usize, Error> { encode_each(entries.len(), |k| entries[k], out) }

/// As `encode`, the `count` entries given one by one, so no array of them is built (`worktree` reads them from a node).
pub(crate) fn encode_each<'a>(count: usize, entry: impl Fn(usize) -> Entry<'a>, out: &mut [u8; CHUNK]) -> Result<usize, Error> {
    if !(0..count).all(|k| entry(k).valid()) || (1..count).any(|k| entry(k - 1).name >= entry(k).name) { return Err(Error::Format); }
    let mut n = Count(0);
    encode_with(count, &entry, &mut n);
    if n.0 > CHUNK { return Err(Error::TooLarge); }
    let mut o = Out { bytes: out, at: 0 };
    encode_with(count, &entry, &mut o);
    Ok(o.at)
}

// The entries come one by one, so the decoder's check builds no array of them. The keys are in DAG-CBOR's order.
fn encode_with<'a>(count: usize, entry: impl Fn(usize) -> Entry<'a>, o: &mut impl Sink) {
    o.head(5, 2);
    o.text("v");
    o.head(0, VERSION);
    o.text("entries");
    o.head(4, count as u64);
    for k in 0..count {
        let e = entry(k);
        o.head(4, 4);
        o.head(3, e.name.len() as u64);
        o.push(e.name);
        o.head(0, e.kind.code());
        o.head(0, e.size);
        o.link(&e.cid);
    }
}

// Reads an entry and checks its name and link.
fn entry<'a>(i: &mut In<'a>) -> Result<Entry<'a>, Error> {
    if i.expect(4)? != 4 { return Err(Error::Format); }
    let len = i.expect(3)? as usize;
    let name = i.take(len)?;
    let kind = Kind::from_code(i.expect(0)?).ok_or(Error::Format)?;
    let size = i.expect(0)?;
    let e = Entry { name, kind, size, cid: i.link()? };
    if !e.valid() { return Err(Error::Format); }
    Ok(e)
}

/// A decoded directory: its entries read from the node's own bytes when asked for, by where each one starts.
pub struct Dir<'a> { bytes: &'a [u8], count: usize, starts: [u16; ENTRIES] }

impl<'a> Dir<'a> {
    pub fn len(&self) -> usize { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
    pub fn entry(&self, i: usize) -> Entry<'a> { self.at(self.starts[i]) }
    /// The node's bytes.
    pub(crate) fn bytes(&self) -> &'a [u8] { self.bytes }

    /// The entry named `name`, by a binary search of the sorted names.
    pub fn find(&self, name: &[u8]) -> Option<Entry<'a>> {
        let i = self.starts[..self.count].binary_search_by(|&at| self.at(at).name.cmp(name)).ok()?;
        Some(self.entry(i))
    }

    // Every entry was read and checked by `decode`.
    fn at(&self, start: u16) -> Entry<'a> { entry(&mut In { bytes: self.bytes, at: start as usize }).unwrap() }
}

/// Decodes a directory, only if it is exactly the canonical encoding of this schema.
pub fn decode(bytes: &[u8]) -> Result<Dir<'_>, Error> {
    if bytes.len() > CHUNK { return Err(Error::TooLarge); }
    let mut i = In { bytes, at: 0 };
    let keys = i.expect(5)?;
    // "v" sorts first, so another schema version is told apart before anything else of it is read.
    i.key("v")?;
    if i.expect(0)? != VERSION { return Err(Error::Version); }
    if keys != 2 { return Err(Error::Format); }
    i.key("entries")?;
    let count = i.expect(4)? as usize;
    if count > ENTRIES { return Err(Error::Format); }
    let mut dir = Dir { bytes, count, starts: [0; ENTRIES] };
    let mut last: &[u8] = &[];
    for k in 0..count {
        dir.starts[k] = i.at as u16;
        let e = entry(&mut i)?;
        // Sorted by the names' bytes, each name once.
        if e.name <= last { return Err(Error::Format); }
        last = e.name;
    }
    if i.at != bytes.len() { return Err(Error::Format); }
    // Shortest integer forms: the encoding of what was read must be the bytes themselves.
    let mut same = Same { bytes, at: 0, equal: true };
    encode_with(count, |k| dir.entry(k), &mut same);
    if !same.equal || same.at != bytes.len() { return Err(Error::Format); }
    Ok(dir)
}

/// Visits every block under `root`, a file or a directory: each directory node (then read and checked against its CID
/// and the schema) and every block of each file (`dag::walk`). Each entry's size is checked against its object: a
/// file's bytes, a directory's entries. The directories on the way down are a stack of (CID, next entry), each read
/// again on the way back, so `buffer` is the only block held. Returns the bytes of the files under the root; TooLarge
/// once it has taken `steps` steps (an entry gone through, a block visited), so its work is bounded whatever the tree.
pub fn walk<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], steps: u64, visit: impl FnMut(&mut B, &Cid) -> Result<(), Error>) -> Result<u64, Error> {
    tour(blocks, root, buffer, true, steps, visit)
}

/// The bytes of the files under `root`, as `walk` returns them, from the directory nodes and each file's root alone,
/// within `steps` steps (an entry gone through, a directory visited).
pub fn size<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], steps: u64) -> Result<u64, Error> {
    tour(blocks, root, buffer, false, steps, |_, _| Ok(()))
}

// The entries of the directory `cid` names; none for a file (a `raw` one is not read).
fn entries<B: Blocks>(blocks: &mut B, cid: &Cid, buffer: &mut [u8; CHUNK]) -> Result<Option<usize>, Error> {
    if cid.codec() != Codec::DagCbor { return Ok(None); }
    let len = fetch(blocks, cid, buffer)?;
    Ok(decode(&buffer[..len]).ok().map(|dir| dir.len()))
}

// A file's bytes: every block of it visited (`whole`), or its root alone read.
fn file<B: Blocks>(blocks: &mut B, cid: &Cid, buffer: &mut [u8; CHUNK], whole: bool, visit: &mut impl FnMut(&mut B, &Cid) -> Result<(), Error>) -> Result<u64, Error> {
    if whole { dag::walk(blocks, cid, buffer, visit) } else { dag::size(blocks, cid, buffer) }
}

fn tour<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], whole: bool, steps: u64, mut visit: impl FnMut(&mut B, &Cid) -> Result<(), Error>) -> Result<u64, Error> {
    // Each entry and each block visited is a step, the walks of files' included.
    let left = core::cell::Cell::new(steps);
    let step = || left.get().checked_sub(1).map(|n| left.set(n)).ok_or(Error::TooLarge);
    let mut visit = |b: &mut B, cid: &Cid| { step()?; visit(b, cid) };
    let Some(count) = entries(blocks, root, buffer)? else { return file(blocks, root, buffer, whole, &mut visit) };
    visit(blocks, root)?;
    // The directories from the root down: each one's CID, the entries its parent says it has, the next entry.
    let mut stack = [(*root, count as u64, 0usize); DEPTH];
    let (mut depth, mut total) = (1, 0u64);
    while depth > 0 {
        let (cid, count, next) = stack[depth - 1];
        // Read again each time: a file's walk takes the buffer.
        let len = fetch(blocks, &cid, buffer)?;
        let dir = match decode(&buffer[..len]) {
            // A file's node where a directory belongs.
            Err(Error::Format) if dag::decode(&buffer[..len]).is_ok() => return Err(Error::Shape),
            dir => dir?,
        };
        if dir.len() as u64 != count { return Err(Error::Shape); }
        if next == dir.len() { depth -= 1; continue; }
        step()?;
        stack[depth - 1].2 += 1;
        let Entry { kind, size, cid, .. } = dir.entry(next);
        match kind {
            Kind::File => {
                let held = match file(blocks, &cid, buffer, whole, &mut visit) {
                    // A directory where a file belongs.
                    Err(Error::Format) if matches!(entries(blocks, &cid, buffer), Ok(Some(_))) => Err(Error::Shape),
                    held => held,
                }?;
                if held != size { return Err(Error::Shape); }
                total = total.checked_add(size).ok_or(Error::TooLarge)?;
            }
            Kind::Directory => {
                if depth == DEPTH { return Err(Error::Shape); }
                visit(blocks, &cid)?;
                stack[depth] = (cid, size, 0);
                depth += 1;
            }
        }
    }
    Ok(total)
}
