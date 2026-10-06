//! The block store's layout and logic (issue 300-STO-0002; MC-4.2, 4.8): immutable blocks named by their CID in an
//! append-only log of records on a block device. Nothing here depends on the kernel: the host tests build it too.
//!
//! Sector 0 holds the superblock. Records follow from sector 1, each starting on a sector: a header (magic, layout
//! version, length, the block's CID, a digest of the header) and the block's bytes, padded to the sector. A put
//! writes only sectors after the last non-blank one, so nothing stored is ever overwritten; every block read is
//! checked against its CID, and one that does not match is reported corrupt, never returned.
use crate::cid::{self, Cid};
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
}

/// An index entry: the CID's binary form (their order is the CIDs' order) and where its record starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry { key: [u8; cid::BYTES], lba: u64, len: u32 }
impl Entry { pub const EMPTY: Entry = Entry { key: [0; cid::BYTES], lba: 0, len: 0 }; }

pub struct Store<'a, D: Device> {
    dev: D,
    index: &'a mut [Entry],
    count: usize,
    buffer: &'a mut [u8; BUFFER],
    end: u64,
    bytes: u64,
    corrupt: u32,
    damaged: u64,
}

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
    /// verified; anything else is refused. `index` bounds how many blocks it can hold.
    pub fn mount(mut dev: D, index: &'a mut [Entry], buffer: &'a mut [u8; BUFFER]) -> Result<Self, Error> {
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
        let mut store = Store { dev, index, count: 0, buffer, end: 1, bytes: 0, corrupt: 0, damaged: 0 };
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
        let whole = parse_header(&self.buffer[..SECTOR]) == Some((*cid, len)) && cid.matches(&self.buffer[HEADER..HEADER + len]);
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

    /// Stores `data` as a raw block and returns its CID, once the device has flushed it. A block already held is not
    /// written again.
    pub fn put(&mut self, data: &[u8]) -> Result<Cid, Error> {
        if data.len() > BLOCK_MAX { return Err(Error::TooLarge); }
        let cid = Cid::raw(data);
        if self.contains(&cid) { return Ok(cid); }
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let n = record_sectors(data.len());
        if self.count == self.index.len() || self.end + n as u64 > self.dev.sectors() { return Err(Error::Full); }
        let record = &mut self.buffer[..n * SECTOR];
        record.fill(0);
        record[..HEADER].copy_from_slice(&header(&cid, data.len()));
        record[HEADER..HEADER + data.len()].copy_from_slice(data);
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
            corrupt: self.corrupt, damaged: self.damaged, capacity: self.index.len() as u32,
        }
    }

    #[cfg(test)]
    pub fn device(&mut self) -> &mut D { &mut self.dev }
}
