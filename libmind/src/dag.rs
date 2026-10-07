//! Objects larger than a block as a Merkle-DAG of blocks (issue 301-STO-0001; MC-4.2, Appendix B.3). An object's
//! bytes are cut into chunks of CHUNK bytes, each a `raw` block; nodes link up to FANOUT children and are DAG-CBOR
//! blocks of the form `{"v": 1, "size": <bytes under the node>, "links": [<CID>, ...]}`. The tree's shape follows from
//! the size alone, so the same bytes always give the same root, and a reader checks every node and chunk against that
//! shape as well as against its CID. No heap and no system calls: tests/dag_host.rs.
//!
//! The shape: an object of at most CHUNK bytes is one raw block (an empty object too). A larger one has a root of the
//! least height h with size <= CHUNK * FANOUT^h. A node of height k has ceil(size / (CHUNK * FANOUT^(k-1))) children,
//! all full but the last, each of height k - 1; the children of height 1 are chunks.
use crate::cid::{self, Cid, Codec};

/// Bytes of a chunk.
pub const CHUNK: usize = 16384;
/// Children of a node at most.
pub const FANOUT: usize = 256;
/// The node schema's version (`"v"`); another is refused (MC-4.13).
pub const VERSION: u64 = 1;
// tag 42, a 37-byte byte string, the multibase identity prefix 0, the CID's binary form.
const LINK: usize = 2 + 2 + 1 + cid::BYTES;
/// Bytes of the largest node: the map, "v", "size" (with a 64-bit value), "links" and FANOUT links.
pub const NODE_MAX: usize = 1 + 2 + 1 + 5 + 9 + 6 + 3 + FANOUT * LINK;
/// Levels a builder keeps: a tree over u64 bytes is at most 7 nodes high.
const LEVELS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A node that is not canonical DAG-CBOR of this schema, or a link that is not a supported CID.
    Format,
    /// A node of another schema version.
    Version,
    /// A node or chunk that does not fit the shape its size gives: link count, the children's sizes and types, a
    /// chunk's length.
    Shape,
    /// A block whose bytes do not match its CID.
    Corrupt,
    /// The store has no block with this CID.
    NotFound,
    /// The store refused a block for lack of room.
    Full,
    /// More than u64 bytes, or a read past the end.
    TooLarge,
    /// Another failure of the store.
    Store,
}

/// Where blocks are kept: the block store, or memory in tests.
pub trait Blocks {
    /// Stores `data` as a block of type `codec`; returns its CID.
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, Error>;
    /// Copies the block `cid` names into `out` and returns its length.
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error>;
    /// Whether the store holds the block `cid` names.
    fn has(&mut self, cid: &Cid) -> Result<bool, Error>;
}

/// The least height of a tree over `size` bytes: 0 is a single chunk.
pub fn height(size: u64) -> u32 {
    let (mut h, mut capacity) = (0, CHUNK as u128);
    while size as u128 > capacity { capacity *= FANOUT as u128; h += 1; }
    h
}

/// Bytes under one child of a node of height `h` (at least 1).
fn capacity(h: u32) -> u128 { CHUNK as u128 * (FANOUT as u128).pow(h - 1) }

/// A decoded node: its size and its links' binary forms.
/// A decoded node: its size, and its links read from the node's own bytes when asked for, so a node takes no room on
/// the stack (a program's stack is 64 KiB).
pub struct Node<'a> { pub size: u64, links: &'a [u8], count: usize }

impl Node<'_> {
    pub fn len(&self) -> usize { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
    // Every link of a canonical node is LINK bytes: tag, string head, prefix, then the CID.
    pub fn link(&self, i: usize) -> Cid { Cid::from_bytes(&self.links[i * LINK + LINK - cid::BYTES..(i + 1) * LINK]).unwrap() }

    /// Whether this node fits a place of height `h` holding `size` bytes.
    fn check(&self, h: u32, size: u64) -> Result<(), Error> {
        let children = (size as u128).div_ceil(capacity(h));
        let codec = if h == 1 { Codec::Raw } else { Codec::DagCbor };
        if self.size != size || self.count as u128 != children || (0..self.count).any(|i| self.link(i).codec() != codec) {
            return Err(Error::Shape);
        }
        Ok(())
    }
}

// Where an encoding goes: written into a buffer, or compared with bytes already there (`decode`'s canonical check).
struct Out<'a> { bytes: &'a mut [u8], at: usize }
struct Same<'a> { bytes: &'a [u8], at: usize, equal: bool }
trait Sink {
    fn push(&mut self, data: &[u8]);
    // A CBOR head in its shortest form, as DAG-CBOR requires.
    fn head(&mut self, major: u8, value: u64) {
        let m = major << 5;
        match value {
            0..=23 => self.push(&[m | value as u8]),
            24..=0xff => self.push(&[m | 24, value as u8]),
            0x100..=0xffff => { self.push(&[m | 25]); self.push(&(value as u16).to_be_bytes()) }
            0x1_0000..=0xffff_ffff => { self.push(&[m | 26]); self.push(&(value as u32).to_be_bytes()) }
            _ => { self.push(&[m | 27]); self.push(&value.to_be_bytes()) }
        }
    }
    fn text(&mut self, text: &str) { self.head(3, text.len() as u64); self.push(text.as_bytes()); }
}
impl Sink for Out<'_> {
    fn push(&mut self, data: &[u8]) { self.bytes[self.at..self.at + data.len()].copy_from_slice(data); self.at += data.len(); }
}
impl Sink for Same<'_> {
    fn push(&mut self, data: &[u8]) {
        self.equal &= self.bytes.get(self.at..self.at + data.len()) == Some(data);
        self.at += data.len();
    }
}

/// Encodes a node over `size` bytes with `links` (at most FANOUT) into `out` (NODE_MAX bytes at least); returns its
/// length. The keys are in DAG-CBOR's order: shorter first.
pub fn encode(size: u64, links: &[Cid], out: &mut [u8]) -> usize {
    let mut o = Out { bytes: out, at: 0 };
    encode_with(size, links.len(), |k| links[k], &mut o);
    o.at
}

// The links come one by one, so no array of them is built on the stack.
fn encode_with(size: u64, count: usize, link: impl Fn(usize) -> Cid, o: &mut impl Sink) {
    o.head(5, 3);
    o.text("v");
    o.head(0, VERSION);
    o.text("size");
    o.head(0, size);
    o.text("links");
    o.head(4, count as u64);
    for k in 0..count {
        o.head(6, 42);
        o.head(2, 1 + cid::BYTES as u64);
        o.push(&[0]);
        o.push(&link(k).to_bytes());
    }
}

struct In<'a> { bytes: &'a [u8], at: usize }
impl<'a> In<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let taken = self.bytes.get(self.at..self.at + n).ok_or(Error::Format)?;
        self.at += n;
        Ok(taken)
    }
    fn head(&mut self) -> Result<(u8, u64), Error> {
        let first = self.take(1)?[0];
        let (major, info) = (first >> 5, first & 31);
        let value = match info {
            0..=23 => info as u64,
            24..=27 => self.take(1 << (info - 24))?.iter().fold(0u64, |v, &b| v << 8 | b as u64),
            _ => return Err(Error::Format), // indefinite lengths and reserved values
        };
        Ok((major, value))
    }
    fn expect(&mut self, major: u8) -> Result<u64, Error> {
        match self.head()? { (m, value) if m == major => Ok(value), _ => Err(Error::Format) }
    }
    fn key(&mut self, name: &str) -> Result<(), Error> {
        let len = self.expect(3)? as usize;
        if self.take(len)? != name.as_bytes() { return Err(Error::Format); }
        Ok(())
    }
}

/// Decodes a node, only if it is exactly the canonical encoding of this schema.
pub fn decode(bytes: &[u8]) -> Result<Node<'_>, Error> {
    let mut i = In { bytes, at: 0 };
    let entries = i.expect(5)?;
    // "v" sorts first, so another schema version is told apart before anything else of it is read.
    i.key("v")?;
    if i.expect(0)? != VERSION { return Err(Error::Version); }
    if entries != 3 { return Err(Error::Format); }
    i.key("size")?;
    let size = i.expect(0)?;
    i.key("links")?;
    let count = i.expect(4)? as usize;
    if count == 0 || count > FANOUT { return Err(Error::Format); }
    let start = i.at;
    for _ in 0..count {
        if i.expect(6)? != 42 || i.expect(2)? != 1 + cid::BYTES as u64 || i.take(1)? != [0] { return Err(Error::Format); }
        Cid::from_bytes(i.take(cid::BYTES)?).map_err(|_| Error::Format)?;
    }
    if i.at != bytes.len() || i.at - start != count * LINK { return Err(Error::Format); }
    let node = Node { size, links: &bytes[start..], count };
    // Shortest integer forms: the encoding of what was read must be the bytes themselves.
    let mut same = Same { bytes, at: 0, equal: true };
    encode_with(size, count, |k| node.link(k), &mut same);
    if !same.equal || same.at != bytes.len() { return Err(Error::Format); }
    Ok(node)
}

// Fetches a block and checks it against its CID: the store is another process, and the reader does not trust it.
fn fetch<B: Blocks>(blocks: &mut B, cid: &Cid, buffer: &mut [u8; CHUNK]) -> Result<usize, Error> {
    let len = blocks.get(cid, buffer)?;
    if !cid.matches(&buffer[..len]) { return Err(Error::Corrupt); }
    Ok(len)
}

fn node<'b, B: Blocks>(blocks: &mut B, cid: &Cid, buffer: &'b mut [u8; CHUNK]) -> Result<Node<'b>, Error> {
    if cid.codec() != Codec::DagCbor { return Err(Error::Shape); }
    let len = fetch(blocks, cid, buffer)?;
    decode(&buffer[..len])
}

/// The size of the object `root` names, after checking its root block.
pub fn size<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK]) -> Result<u64, Error> {
    match root.codec() {
        Codec::Raw => fetch(blocks, root, buffer).map(|len| len as u64),
        Codec::DagCbor => {
            let n = node(blocks, root, buffer)?;
            let h = height(n.size);
            if h == 0 { return Err(Error::Shape); }
            n.check(h, n.size)?;
            Ok(n.size)
        }
    }
}

/// Reads up to `out.len()` bytes of the object `root` from `offset`; returns how many (0 at the end). Every node and
/// chunk on the way is checked against its CID and its place in the shape.
pub fn read_at<B: Blocks>(blocks: &mut B, root: &Cid, offset: u64, out: &mut [u8], buffer: &mut [u8; CHUNK]) -> Result<usize, Error> {
    let total = size(blocks, root, buffer)?;
    if offset > total { return Err(Error::TooLarge); }
    let mut done = 0;
    while done < out.len() && offset + (done as u64) < total {
        let at = offset + done as u64;
        // Down from the root to the chunk holding `at`.
        let (mut cid, mut h, mut held, mut within) = (*root, height(total), total, at);
        while h > 0 {
            let n = node(blocks, &cid, buffer)?;
            n.check(h, held)?;
            let capacity = capacity(h) as u64;
            let child = (within / capacity) as usize;
            cid = n.link(child);
            held = capacity.min(held - child as u64 * capacity);
            within -= child as u64 * capacity;
            h -= 1;
        }
        let len = fetch(blocks, &cid, buffer)?;
        if cid.codec() != Codec::Raw || len as u64 != held { return Err(Error::Shape); }
        let take = (len - within as usize).min(out.len() - done);
        out[done..done + take].copy_from_slice(&buffer[within as usize..within as usize + take]);
        done += take;
    }
    Ok(done)
}

/// Visits every block of the object `root`: each node (then read and checked against its CID and the shape) and each
/// chunk (named, not read). It descends from the root again for each node of height 1, so it needs no stack of nodes,
/// and a node above height 1 is visited once for each node of height 1 under it. Returns the object's size.
pub fn walk<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], mut visit: impl FnMut(&mut B, &Cid) -> Result<(), Error>) -> Result<u64, Error> {
    visit(blocks, root)?;
    let total = size(blocks, root, buffer)?;
    if root.codec() == Codec::Raw { return Ok(total); }
    let span = capacity(1) as u64;
    for leaf in 0..total.div_ceil(span) {
        let (mut cid, mut h, mut held, mut within) = (*root, height(total), total, leaf * span);
        loop {
            let n = node(blocks, &cid, buffer)?;
            n.check(h, held)?;
            if h == 1 {
                for i in 0..n.len() { visit(blocks, &n.link(i))?; }
                break;
            }
            let capacity = capacity(h) as u64;
            let child = (within / capacity) as usize;
            cid = n.link(child);
            visit(blocks, &cid)?;
            held = capacity.min(held - child as u64 * capacity);
            within -= child as u64 * capacity;
            h -= 1;
        }
    }
    Ok(total)
}

/// Whether every block of the object `root` is stored: `walk` with every chunk asked for with `has`. Returns the
/// object's size; Err(NotFound) names nothing, only that one is missing.
pub fn complete<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK]) -> Result<u64, Error> {
    walk(blocks, root, buffer, |b, cid| if b.has(cid)? { Ok(()) } else { Err(Error::NotFound) })
}

#[derive(Clone, Copy)]
struct Entry { key: [u8; cid::BYTES], size: u64 }
impl Entry { fn cid(&self) -> Cid { Cid::from_bytes(&self.key).unwrap() } }

/// Writes an object's bytes as chunks and nodes while they come; `finish` returns the root.
pub struct Builder { chunk: [u8; CHUNK], filled: usize, total: u64, levels: [[Entry; FANOUT]; LEVELS], counts: [usize; LEVELS], node: [u8; NODE_MAX] }

impl Default for Builder { fn default() -> Self { Self::new() } }

impl Builder {
    /// A builder takes about 110 KiB: a program keeps one in a static, `const` lets it.
    pub const fn new() -> Self {
        let empty = Entry { key: [0; cid::BYTES], size: 0 };
        Self { chunk: [0; CHUNK], filled: 0, total: 0, levels: [[empty; FANOUT]; LEVELS], counts: [0; LEVELS], node: [0; NODE_MAX] }
    }

    pub fn write<B: Blocks>(&mut self, blocks: &mut B, mut data: &[u8]) -> Result<(), Error> {
        self.total = self.total.checked_add(data.len() as u64).ok_or(Error::TooLarge)?;
        while !data.is_empty() {
            if self.filled == CHUNK { self.flush_chunk(blocks)?; }
            let take = data.len().min(CHUNK - self.filled);
            self.chunk[self.filled..self.filled + take].copy_from_slice(&data[..take]);
            self.filled += take;
            data = &data[take..];
        }
        Ok(())
    }

    // Puts a block and checks the CID the store returns.
    fn put<B: Blocks>(blocks: &mut B, codec: Codec, data: &[u8]) -> Result<Cid, Error> {
        let cid = blocks.put(codec, data)?;
        if cid != Cid::of(codec, data) { return Err(Error::Corrupt); }
        Ok(cid)
    }

    fn flush_chunk<B: Blocks>(&mut self, blocks: &mut B) -> Result<(), Error> {
        let cid = Self::put(blocks, Codec::Raw, &self.chunk[..self.filled])?;
        let entry = Entry { key: cid.to_bytes(), size: self.filled as u64 };
        self.filled = 0;
        self.push(blocks, 0, entry)
    }

    // Adds a subtree of height `level`; a full level becomes a node one higher.
    fn push<B: Blocks>(&mut self, blocks: &mut B, level: usize, entry: Entry) -> Result<(), Error> {
        self.levels[level][self.counts[level]] = entry;
        self.counts[level] += 1;
        if self.counts[level] == FANOUT { self.wrap(blocks, level)?; }
        Ok(())
    }

    fn wrap<B: Blocks>(&mut self, blocks: &mut B, level: usize) -> Result<(), Error> {
        let count = self.counts[level];
        self.counts[level] = 0;
        let size = self.levels[level][..count].iter().map(|e| e.size).sum();
        let entries = &self.levels[level];
        let mut o = Out { bytes: &mut self.node, at: 0 };
        encode_with(size, count, |k| entries[k].cid(), &mut o);
        let len = o.at;
        let cid = Self::put(blocks, Codec::DagCbor, &self.node[..len])?;
        self.push(blocks, level + 1, Entry { key: cid.to_bytes(), size })
    }

    /// Writes what is left and the nodes above it; returns the root and the object's size.
    pub fn finish<B: Blocks>(&mut self, blocks: &mut B) -> Result<(Cid, u64), Error> {
        if self.filled > 0 || self.total == 0 { self.flush_chunk(blocks)?; }
        for level in 0..LEVELS {
            let higher = self.counts[level + 1..].iter().any(|&c| c > 0);
            match self.counts[level] {
                0 => {}
                1 if !higher => { self.counts[level] = 0; return Ok((self.levels[level][0].cid(), self.total)); }
                _ => self.wrap(blocks, level)?,
            }
        }
        Err(Error::TooLarge)
    }
}
