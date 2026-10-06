//! The block store's layout and logic (issues 300-STO-0002, 301-STO-0002, 302-STO-0001; MC-4.2, 4.3, 4.4, 4.8):
//! immutable blocks named by their CID, and names whose current version points at a root, in an append-only log of
//! records on a block device. Nothing here depends on the kernel: the host tests build it too.
//!
//! Sector 0 holds the superblock. Records follow from sector 1, each starting on a sector. A block record is a header
//! (magic, layout version, length, the block's CID, a digest of the header) and the block's bytes, padded to the
//! sector; a `dag-cbor` block must be a node of `dag`. A name record is one sector: the name, its version and root,
//! and a digest. Writes go only after the last non-blank sector, so nothing stored is ever overwritten; every block
//! read is checked against its CID, and one that does not match is reported corrupt, never returned.
use crate::cid::{self, Cid, Codec};
use crate::dag;
use crate::sha256;

pub const SECTOR: usize = 512;
/// Largest block a put takes.
pub const BLOCK_MAX: usize = 16384;
/// Bytes of a record header.
pub const HEADER: usize = 84;
/// Sectors of the largest record; the store's buffer holds one.
pub const RECORD_SECTORS: usize = (HEADER + BLOCK_MAX).div_ceil(SECTOR);
pub const BUFFER: usize = RECORD_SECTORS * SECTOR;
/// The layout's version; another is refused, never read as this one (MC-4.13).
pub const LAYOUT: u16 = 1;
const SUPER_MAGIC: &[u8; 8] = b"MIND-STO";
const RECORD_MAGIC: &[u8; 8] = b"MIND-BLK";
const NAME_MAGIC: &[u8; 8] = b"MIND-REF";
/// Bytes of a name at most; a name is 1 to NAME_MAX of `A-Z a-z 0-9 . _ / -`.
pub const NAME_MAX: usize = 64;
// A name record: magic, layout, name length (u16), version (u64), root CID, name (padded with zeros), digest.
const NAME_RECORD: usize = 120 + 32;

/// The medium under the store: sectors of SECTOR bytes, read and written up to RECORD_SECTORS at a time.
pub trait Device {
    fn sectors(&self) -> u64;
    fn writable(&self) -> bool;
    /// Reads `out.len() / SECTOR` sectors from `lba`.
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool;
    /// Writes `data.len() / SECTOR` sectors at `lba`.
    fn write(&mut self, lba: u64, data: &[u8]) -> bool;
    fn flush(&mut self) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// No block with this CID is stored.
    NotFound,
    /// The stored bytes do not match the CID; they are not returned.
    Corrupt,
    /// No room on the medium or in the index for the block.
    Full,
    /// The block is larger than BLOCK_MAX, or than the buffer it is to be read into.
    TooLarge,
    /// The medium is write-protected or the client may not write.
    ReadOnly,
    /// The device failed a read, write or flush.
    Device,
    /// The medium is neither wholly blank nor a store (sector 0 holds no valid superblock): it belongs to something
    /// else and is left alone.
    Foreign,
    /// A store of another layout version.
    Layout,
    /// A `dag-cbor` block that is not a node of `dag`'s schema (MC-4.2: the type is bound to the data); a block whose
    /// bytes would start a sector with a record's magic; a name that is not 1 to NAME_MAX allowed bytes; a root whose
    /// tree is out of shape.
    Invalid,
    /// The name's current version is not the one the publisher expected (MC-4.3): nothing was published.
    Conflict,
    /// A block of the root's object is not stored (MC-4.4): nothing was published.
    Incomplete,
}

/// What the store holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Intact blocks in the index and their bytes.
    pub blocks: u32,
    pub bytes: u64,
    /// Sectors from the superblock to the end of the log, and of the medium.
    pub used: u64,
    pub sectors: u64,
    /// Records whose bytes did not match their CID, found when mounting or reading.
    pub corrupt: u32,
    /// Non-blank sectors outside every valid record, found when mounting.
    pub damaged: u64,
    /// Blocks the index can hold.
    pub capacity: u32,
    /// Names published.
    pub names: u32,
}

/// An index entry: the CID's binary form (their order is the CIDs' order) and where its record starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry { key: [u8; cid::BYTES], lba: u64, len: u32 }
impl Entry { pub const EMPTY: Entry = Entry { key: [0; cid::BYTES], lba: 0, len: 0 }; }

/// A name's current version and root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head { name: [u8; NAME_MAX], len: u8, pub version: u64, root: [u8; cid::BYTES] }
impl Head {
    pub const EMPTY: Head = Head { name: [0; NAME_MAX], len: 0, version: 0, root: [0; cid::BYTES] };
    pub fn name(&self) -> &[u8] { &self.name[..self.len as usize] }
    pub fn root(&self) -> Cid { Cid::from_bytes(&self.root).unwrap() }
}

/// Whether `name` is 1 to NAME_MAX bytes of `A-Z a-z 0-9 . _ / -`.
pub fn valid_name(name: &[u8]) -> bool {
    (1..=NAME_MAX).contains(&name.len()) && name.iter().all(|&c| c.is_ascii_alphanumeric() || b"._/-".contains(&c))
}

fn name_record(head: &Head) -> [u8; SECTOR] {
    let mut r = [0u8; SECTOR];
    r[..8].copy_from_slice(NAME_MAGIC);
    r[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
    r[10..12].copy_from_slice(&(head.len as u16).to_le_bytes());
    r[12..20].copy_from_slice(&head.version.to_le_bytes());
    r[20..56].copy_from_slice(&head.root);
    r[56..120].copy_from_slice(&head.name);
    let check = sha256::digest(&r[..120]);
    r[120..NAME_RECORD].copy_from_slice(&check);
    r
}

/// The head a sector names, if it is a whole, valid name record of this layout.
fn parse_name(sector: &[u8]) -> Option<Head> {
    if &sector[..8] != NAME_MAGIC || sector[120..NAME_RECORD] != sha256::digest(&sector[..120]) { return None; }
    if u16::from_le_bytes([sector[8], sector[9]]) != LAYOUT || sector[NAME_RECORD..].iter().any(|&b| b != 0) { return None; }
    let len = u16::from_le_bytes([sector[10], sector[11]]) as usize;
    let version = u64::from_le_bytes(sector[12..20].try_into().unwrap());
    if !valid_name(sector.get(56..56 + len)?) || sector[56 + len..120].iter().any(|&b| b != 0) || version == 0 { return None; }
    Cid::from_bytes(&sector[20..56]).ok()?;
    Some(Head { name: sector[56..120].try_into().unwrap(), len: len as u8, version, root: sector[20..56].try_into().unwrap() })
}

pub struct Store<'a, D: Device> {
    dev: D,
    index: &'a mut [Entry],
    count: usize,
    heads: &'a mut [Head],
    names: usize,
    buffer: &'a mut [u8; BUFFER],
    end: u64,
    bytes: u64,
    corrupt: u32,
    damaged: u64,
}

// Whether `data` is what its type says: any bytes are raw, a node must decode.
fn typed(codec: Codec, data: &[u8]) -> bool { codec == Codec::Raw || dag::decode(data).is_ok() }

/// Sectors of the record of a block of `len` bytes.
pub const fn record_sectors(len: usize) -> usize { (HEADER + len).div_ceil(SECTOR) }

fn header(cid: &Cid, len: usize) -> [u8; HEADER] {
    let mut h = [0u8; HEADER];
    h[..8].copy_from_slice(RECORD_MAGIC);
    h[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
    h[12..16].copy_from_slice(&(len as u32).to_le_bytes());
    h[16..52].copy_from_slice(&cid.to_bytes());
    let check = sha256::digest(&h[..52]);
    h[52..84].copy_from_slice(&check);
    h
}

/// The CID and length a sector's header names, if it is a whole, valid record header of this layout.
fn parse_header(sector: &[u8]) -> Option<(Cid, usize)> {
    if &sector[..8] != RECORD_MAGIC || sector[52..84] != sha256::digest(&sector[..52]) { return None; }
    if u16::from_le_bytes([sector[8], sector[9]]) != LAYOUT || sector[10..12] != [0, 0] { return None; }
    let len = u32::from_le_bytes(sector[12..16].try_into().unwrap()) as usize;
    if len > BLOCK_MAX { return None; }
    Some((Cid::from_bytes(&sector[16..52]).ok()?, len))
}

// Whether every sector of the medium is zero.
fn blank<D: Device>(dev: &mut D, buffer: &mut [u8; BUFFER]) -> Result<bool, Error> {
    let sectors = dev.sectors();
    let mut lba = 0;
    while lba < sectors {
        let window = (sectors - lba).min(RECORD_SECTORS as u64) as usize;
        if !dev.read(lba, &mut buffer[..window * SECTOR]) { return Err(Error::Device); }
        if buffer[..window * SECTOR].iter().any(|&b| b != 0) { return Ok(false); }
        lba += window as u64;
    }
    Ok(true)
}

fn superblock() -> [u8; SECTOR] {
    let mut s = [0u8; SECTOR];
    s[..8].copy_from_slice(SUPER_MAGIC);
    s[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
    s[10..12].copy_from_slice(&(SECTOR as u16).to_le_bytes());
    let check = sha256::digest(&s[..16]);
    s[16..48].copy_from_slice(&check);
    s
}

impl<'a, D: Device> Store<'a, D> {
    /// Mounts the store on `dev`: a blank medium is formatted (if writable), a store is scanned and its blocks
    /// verified; anything else is refused. `index` bounds how many blocks it can hold, `heads` how many names.
    pub fn mount(mut dev: D, index: &'a mut [Entry], heads: &'a mut [Head], buffer: &'a mut [u8; BUFFER]) -> Result<Self, Error> {
        if dev.sectors() < 2 { return Err(Error::Full); }
        if !dev.read(0, &mut buffer[..SECTOR]) { return Err(Error::Device); }
        if buffer[..SECTOR].iter().all(|&b| b == 0) {
            // Only a wholly blank medium is formatted: some file systems leave their first sectors zero.
            if !blank(&mut dev, buffer)? { return Err(Error::Foreign); }
            if !dev.writable() { return Err(Error::ReadOnly); }
            if !dev.write(0, &superblock()) || !dev.flush() { return Err(Error::Device); }
        } else if &buffer[..8] != SUPER_MAGIC || buffer[16..48] != sha256::digest(&buffer[..16]) {
            return Err(Error::Foreign);
        } else if buffer[..16] != superblock()[..16] {
            return Err(Error::Layout);
        }
        let mut store = Store { dev, index, count: 0, heads, names: 0, buffer, end: 1, bytes: 0, corrupt: 0, damaged: 0 };
        store.scan()?;
        Ok(store)
    }

    // Reads the log in windows of RECORD_SECTORS; each valid header's record is read whole and verified.
    fn scan(&mut self) -> Result<(), Error> {
        let sectors = self.dev.sectors();
        let mut lba = 1;
        while lba < sectors {
            let window = (sectors - lba).min(RECORD_SECTORS as u64) as usize;
            if !self.dev.read(lba, &mut self.buffer[..window * SECTOR]) { return Err(Error::Device); }
            let mut at = 0;
            while at < window {
                let sector = &self.buffer[at * SECTOR..(at + 1) * SECTOR];
                let here = lba + at as u64;
                if let Some((cid, len)) = parse_header(sector) {
                    let n = record_sectors(len) as u64;
                    if here + n <= sectors {
                        self.end = here + n;
                        if self.verify(here, &cid, len)? && !self.contains(&cid) {
                            if self.count == self.index.len() { return Err(Error::Full); }
                            self.insert(cid.to_bytes(), here, len);
                        }
                        lba = here + n;
                        break;
                    }
                }
                if let Some(head) = parse_name(sector) {
                    // The latest version of a name is current.
                    self.end = here + 1;
                    match self.find(head.name()) {
                        Ok(i) => if head.version > self.heads[i].version { self.heads[i] = head; },
                        Err(i) => { if self.names == self.heads.len() { return Err(Error::Full); } self.add(i, head); }
                    }
                    at += 1;
                    continue;
                }
                if sector.iter().any(|&b| b != 0) { self.damaged += 1; self.end = here + 1; }
                at += 1;
            }
            if at == window { lba += window as u64; }
        }
        Ok(())
    }

    // Reads the record at `lba` into the buffer; whether its header names `cid` and `len` and its bytes match.
    fn verify(&mut self, lba: u64, cid: &Cid, len: usize) -> Result<bool, Error> {
        let n = record_sectors(len);
        if !self.dev.read(lba, &mut self.buffer[..n * SECTOR]) { return Err(Error::Device); }
        let data = &self.buffer[HEADER..HEADER + len];
        let whole = parse_header(&self.buffer[..SECTOR]) == Some((*cid, len)) && cid.matches(data) && typed(cid.codec(), data);
        if !whole { self.corrupt += 1; }
        Ok(whole)
    }

    fn position(&self, key: &[u8; cid::BYTES]) -> Result<usize, usize> { self.index[..self.count].binary_search_by(|e| e.key.cmp(key)) }
    fn contains(&self, cid: &Cid) -> bool { self.position(&cid.to_bytes()).is_ok() }

    fn insert(&mut self, key: [u8; cid::BYTES], lba: u64, len: usize) {
        let at = self.position(&key).unwrap_err();
        self.index.copy_within(at..self.count, at + 1);
        self.index[at] = Entry { key, lba, len: len as u32 };
        self.count += 1;
        self.bytes += len as u64;
    }

    fn remove(&mut self, at: usize) {
        self.bytes -= self.index[at].len as u64;
        self.index.copy_within(at + 1..self.count, at);
        self.count -= 1;
    }

    pub fn has(&self, cid: &Cid) -> bool { self.contains(cid) }

    fn find(&self, name: &[u8]) -> Result<usize, usize> { self.heads[..self.names].binary_search_by(|h| h.name().cmp(name)) }

    fn add(&mut self, at: usize, head: Head) {
        self.heads.copy_within(at..self.names, at + 1);
        self.heads[at] = head;
        self.names += 1;
    }

    /// The current version and root of `name`.
    pub fn resolve(&self, name: &[u8]) -> Result<(u64, Cid), Error> {
        if !valid_name(name) { return Err(Error::Invalid); }
        let head = &self.heads[self.find(name).map_err(|_| Error::NotFound)?];
        Ok((head.version, head.root()))
    }

    /// Publishes `root` as the next version of `name` if `expected` is its current version (0: a new name), once
    /// every block of the object `root` names is stored (MC-4.3, 4.4); returns the new version after the device has
    /// flushed it. `scratch` holds the blocks read on the way.
    pub fn publish(&mut self, name: &[u8], expected: u64, root: &Cid, scratch: &mut [u8; dag::CHUNK]) -> Result<u64, Error> {
        if !valid_name(name) { return Err(Error::Invalid); }
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let found = self.find(name);
        let current = found.map_or(0, |i| self.heads[i].version);
        if expected != current { return Err(Error::Conflict); }
        match dag::complete(self, root, scratch) {
            Ok(_) => {}
            Err(dag::Error::NotFound) => return Err(Error::Incomplete),
            Err(dag::Error::Corrupt) => return Err(Error::Corrupt),
            Err(dag::Error::Store) => return Err(Error::Device),
            Err(_) => return Err(Error::Invalid),
        }
        if found.is_err() && self.names == self.heads.len() { return Err(Error::Full); }
        if self.end + 1 > self.dev.sectors() { return Err(Error::Full); }
        let mut head = Head { name: [0; NAME_MAX], len: name.len() as u8, version: current + 1, root: root.to_bytes() };
        head.name[..name.len()].copy_from_slice(name);
        let lba = self.end;
        self.end += 1;
        if !self.dev.write(lba, &name_record(&head)) || !self.dev.flush() { return Err(Error::Device); }
        match found { Ok(i) => self.heads[i] = head, Err(i) => self.add(i, head) }
        Ok(head.version)
    }

    /// Stores `data` as a block of type `codec` and returns its CID, once the device has flushed it. A node must decode
    /// first; a block already held is not written again.
    pub fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, Error> {
        if data.len() > BLOCK_MAX { return Err(Error::TooLarge); }
        if !typed(codec, data) { return Err(Error::Invalid); }
        let cid = Cid::of(codec, data);
        if self.contains(&cid) { return Ok(cid); }
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let n = record_sectors(data.len());
        if self.count == self.index.len() || self.end + n as u64 > self.dev.sectors() { return Err(Error::Full); }
        let record = &mut self.buffer[..n * SECTOR];
        record.fill(0);
        record[..HEADER].copy_from_slice(&header(&cid, data.len()));
        record[HEADER..HEADER + data.len()].copy_from_slice(data);
        // Only a header sector starts with a record's magic, so a scan that resumes after a damaged sector cannot take
        // a client's bytes for a name or a block record.
        if record.chunks(SECTOR).skip(1).any(|s| &s[..8] == RECORD_MAGIC || &s[..8] == NAME_MAGIC) { return Err(Error::Invalid); }
        let lba = self.end;
        // The sectors are taken even if the write fails: they may hold part of it now, and are never written again.
        self.end += n as u64;
        if !self.dev.write(lba, &self.buffer[..n * SECTOR]) || !self.dev.flush() { return Err(Error::Device); }
        self.insert(cid.to_bytes(), lba, data.len());
        Ok(cid)
    }

    /// Copies the block named by `cid` into `out` and returns its length, only if its bytes match the CID. A corrupt
    /// block leaves the index, so a put of the same bytes stores them again.
    pub fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> {
        let at = self.position(&cid.to_bytes()).map_err(|_| Error::NotFound)?;
        let Entry { lba, len, .. } = self.index[at];
        let len = len as usize;
        if out.len() < len { return Err(Error::TooLarge); }
        if !self.verify(lba, cid, len)? { self.remove(at); return Err(Error::Corrupt); }
        out[..len].copy_from_slice(&self.buffer[HEADER..HEADER + len]);
        Ok(len)
    }

    pub fn stats(&self) -> Stats {
        Stats {
            blocks: self.count as u32, bytes: self.bytes, used: self.end, sectors: self.dev.sectors(),
            corrupt: self.corrupt, damaged: self.damaged, capacity: self.index.len() as u32, names: self.names as u32,
        }
    }

    #[cfg(test)]
    pub fn device(&mut self) -> &mut D { &mut self.dev }
}

// The store as the blocks of `dag`: reads go through `get`, so every block is checked against its CID.
impl<D: Device> dag::Blocks for Store<'_, D> {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> {
        Store::put(self, codec, data).map_err(|e| match e { Error::Full => dag::Error::Full, _ => dag::Error::Store })
    }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> {
        Store::get(self, cid, out).map_err(|e| match e {
            Error::NotFound => dag::Error::NotFound,
            Error::Corrupt => dag::Error::Corrupt,
            _ => dag::Error::Store,
        })
    }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { Ok(self.contains(cid)) }
}
