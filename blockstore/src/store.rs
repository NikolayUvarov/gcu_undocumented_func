//! The block store's layout and logic (issues 300-STO-0002, 301-STO-0002, 302-STO-0001, 303-STO-0001; MC-4.2, 4.3,
//! 4.4, 4.5, 4.8): immutable blocks named by their CID, and names whose current version points at a root, as records on
//! a block device; what no name retains and no lease protects is collected; several names change at once in one
//! commit record (304-STO-0007). Nothing here depends on the kernel: the host tests build it too.
//!
//! Sector 0 holds the superblock. Records follow from sector 1, each starting on a sector. A block record is a header
//! (magic, layout version, length, the block's CID, a digest of the header) and the block's bytes, padded to the
//! sector; a `dag-cbor` block must be a node of `dag`. A name record is one sector: the name, its version and root,
//! and a digest. Records are written only into blank sectors, so nothing stored is overwritten; a collection makes the
//! sectors of what it frees blank again. Every block read is checked against its CID, and one that does not match is
//! reported corrupt, never returned.
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
pub const LAYOUT: u16 = 2;
const SUPER_MAGIC: &[u8; 8] = b"MIND-STO";
const RECORD_MAGIC: &[u8; 8] = b"MIND-BLK";
const NAME_MAGIC: &[u8; 8] = b"MIND-REF";
// A record being freed: magic, layout, zero, the sectors it covers (u32), and the SHA-256 of these 16 bytes.
const FREE_MAGIC: &[u8; 8] = b"MIND-DEL";
/// How long a block no name retains is kept after its last put (or after a mount): the time a writer has to publish
/// the root of what it is putting (MC-4.5: operations in progress).
pub const LEASE_NS: u64 = 60_000_000_000;
/// Bytes of a name at most; a name is 1 to NAME_MAX of `A-Z a-z 0-9 . _ / -`.
pub const NAME_MAX: usize = 64;
// A name record: magic, layout, name length (u16), version (u64), root CID (zero for a removal), the previous
// version's root (zero for none), name (padded with zeros), kind (0: points at the root, 1: removes the name), zero,
// owner (u16), and the SHA-256 of these 160 bytes.
const NAME_RECORD: usize = 160 + 32;
// A pin record: magic, layout, zero, id (u32), owner (u16), zero, root CID, and the SHA-256 of these 56 bytes.
const PIN_MAGIC: &[u8; 8] = b"MIND-PIN";
const PIN_RECORD: usize = 56 + 32;
/// Versions of a name the store keeps and retains: the current one and the ones before it (MC-4.5).
pub const HISTORY: usize = 4;
// A commit record: a header sector (magic, layout, count (u16), zero, the SHA-256 of these 16 bytes and of every entry
// sector), then one sector per name: a name record's fields without its magic and digest, so no entry stands alone.
const TXN_MAGIC: &[u8; 8] = b"MIND-TXN";
/// Names one commit changes at most.
pub const COMMIT_MAX: usize = 8;

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
    /// A block of the root's object is not stored (MC-4.4): nothing was published; or a name's object lacks one, so
    /// nothing was collected.
    Incomplete,
    /// The owner would retain more than its quota (MC-4.11): nothing was published or pinned.
    Quota,
    /// A pin of another owner.
    Rights,
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
    /// Blank sectors the store can write.
    pub free: u64,
    /// Pins held.
    pub pins: u32,
}

/// What one owner retains: the distinct roots of its names' kept versions and of its pins, their bytes, and its quota.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage { pub retained: u64, pub quota: u64, pub names: u32, pub pins: u32 }

/// What a collection freed: block records (unretained, duplicate or corrupt copies), superseded name records, their
/// sectors; and the blank sectors after it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Collected { pub blocks: u32, pub names: u32, pub sectors: u64, pub free: u64 }

/// An index entry: the CID's binary form (their order is the CIDs' order), where its record starts, when its lease
/// began, and whether the last collection found a name that retains it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry { key: [u8; cid::BYTES], lba: u64, len: u32, lease: u64, live: bool }
impl Entry { pub const EMPTY: Entry = Entry { key: [0; cid::BYTES], lba: 0, len: 0, lease: 0, live: false }; }

/// A run of blank sectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent { start: u64, len: u64 }
impl Extent { pub const EMPTY: Extent = Extent { start: 0, len: 0 }; }

/// A version of a name: its number, who published it (a badge), the root it points at (none: this version removed
/// the name), and the size of that object, charged to the owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version { pub version: u64, pub owner: u16, root: [u8; cid::BYTES], points: bool, pub size: u64 }
impl Version {
    pub const EMPTY: Version = Version { version: 0, owner: 0, root: [0; cid::BYTES], points: false, size: 0 };
    pub fn root(&self) -> Option<Cid> { self.points.then(|| Cid::from_bytes(&self.root).unwrap()) }
}

/// A name and the versions of it the store keeps, newest first: at most HISTORY, only the last once it is removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head { name: [u8; NAME_MAX], len: u8, kept: [Version; HISTORY], count: u8 }
impl Head {
    pub const EMPTY: Head = Head { name: [0; NAME_MAX], len: 0, kept: [Version::EMPTY; HISTORY], count: 0 };
    fn new(name: &[u8], version: Version) -> Self {
        let mut h = Head { len: name.len() as u8, ..Head::EMPTY };
        h.name[..name.len()].copy_from_slice(name);
        h.keep(version);
        h
    }
    pub fn name(&self) -> &[u8] { &self.name[..self.len as usize] }
    pub fn latest(&self) -> &Version { &self.kept[0] }
    pub fn versions(&self) -> &[Version] { &self.kept[..self.count as usize] }
    fn removed(&self) -> bool { !self.kept[0].points }
    // The roots the name retains: those of its kept versions, none once it is removed.
    fn retains(&self) -> impl Iterator<Item = &Version> { self.versions().iter().filter(move |v| v.points && !self.removed()) }
    // Keeps `v` among the newest HISTORY versions; once the newest removes the name, only it is kept.
    fn keep(&mut self, v: Version) {
        let n = self.count as usize;
        if self.kept[..n].iter().any(|k| k.version == v.version) { return; }
        let at = self.kept[..n].iter().position(|k| k.version < v.version).unwrap_or(n);
        if at == HISTORY { return; }
        self.kept.copy_within(at..HISTORY - 1, at + 1);
        self.kept[at] = v;
        self.count = (n + 1).min(HISTORY) as u8;
        if self.removed() { self.count = 1; }
    }
}

/// A pin: a retention obligation of an owner (a badge) on an object, until the owner unpins it (MC-4.11, Appendix B.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin { pub id: u32, pub owner: u16, root: [u8; cid::BYTES], lba: u64, pub size: u64 }
impl Pin {
    pub const EMPTY: Pin = Pin { id: 0, owner: 0, root: [0; cid::BYTES], lba: 0, size: 0 };
    pub fn root(&self) -> Cid { Cid::from_bytes(&self.root).unwrap() }
}

/// One name's change in a commit: the version expected and the new root; none removes the name.
#[derive(Clone, Copy, Debug)]
pub struct Update<'n> { pub name: &'n [u8], pub expected: u64, pub root: Option<Cid> }

/// Whether `name` is 1 to NAME_MAX bytes of `A-Z a-z 0-9 . _ / -`.
pub fn valid_name(name: &[u8]) -> bool {
    (1..=NAME_MAX).contains(&name.len()) && name.iter().all(|&c| c.is_ascii_alphanumeric() || b"._/-".contains(&c))
}

fn name_record(name: &[u8], v: &Version, previous: Option<Cid>) -> [u8; SECTOR] {
    let mut r = [0u8; SECTOR];
    r[..8].copy_from_slice(NAME_MAGIC);
    r[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
    r[10..12].copy_from_slice(&(name.len() as u16).to_le_bytes());
    r[12..20].copy_from_slice(&v.version.to_le_bytes());
    if v.points { r[20..56].copy_from_slice(&v.root); }
    if let Some(p) = previous { r[56..92].copy_from_slice(&p.to_bytes()); }
    r[92..92 + name.len()].copy_from_slice(name);
    r[156] = !v.points as u8;
    r[158..160].copy_from_slice(&v.owner.to_le_bytes());
    let check = sha256::digest(&r[..160]);
    r[160..NAME_RECORD].copy_from_slice(&check);
    r
}

/// The name and version a sector records, if it is a whole, valid name record of this layout.
fn parse_name(sector: &[u8]) -> Option<([u8; NAME_MAX], usize, Version)> {
    if &sector[..8] != NAME_MAGIC || sector[160..NAME_RECORD] != sha256::digest(&sector[..160]) { return None; }
    name_fields(sector)
}

// A commit's entry: a name record's fields with zeros where its magic and digest would be.
fn parse_entry(sector: &[u8]) -> Option<([u8; NAME_MAX], usize, Version)> {
    if sector[..8].iter().any(|&b| b != 0) || sector[160..NAME_RECORD].iter().any(|&b| b != 0) { return None; }
    name_fields(sector)
}

// The fields of a name record or commit entry, checked.
fn name_fields(sector: &[u8]) -> Option<([u8; NAME_MAX], usize, Version)> {
    if u16::from_le_bytes([sector[8], sector[9]]) != LAYOUT || sector[NAME_RECORD..].iter().any(|&b| b != 0) { return None; }
    let len = u16::from_le_bytes([sector[10], sector[11]]) as usize;
    let version = u64::from_le_bytes(sector[12..20].try_into().unwrap());
    if !valid_name(sector.get(92..92 + len)?) || sector[92 + len..156].iter().any(|&b| b != 0) || version == 0 || sector[157] != 0 { return None; }
    let points = match sector[156] { 0 => true, 1 => false, _ => return None };
    // A version points at a supported CID or removes the name; its link to the one before is one or zero.
    if points { Cid::from_bytes(&sector[20..56]).ok()?; } else if sector[20..56].iter().any(|&b| b != 0) { return None; }
    if sector[56..92].iter().any(|&b| b != 0) { Cid::from_bytes(&sector[56..92]).ok()?; }
    let owner = u16::from_le_bytes([sector[158], sector[159]]);
    let v = Version { version, owner, root: sector[20..56].try_into().unwrap(), points, size: 0 };
    Some((sector[92..156].try_into().unwrap(), len, v))
}

// The count of entries a commit header announces, if the sector is one of this layout.
fn parse_txn(sector: &[u8]) -> Option<usize> {
    if &sector[..8] != TXN_MAGIC || u16::from_le_bytes([sector[8], sector[9]]) != LAYOUT || sector[12..16] != [0; 4] { return None; }
    if sector[48..].iter().any(|&b| b != 0) { return None; }
    let count = u16::from_le_bytes([sector[10], sector[11]]) as usize;
    (1..=COMMIT_MAX).contains(&count).then_some(count)
}

// The digest a commit header carries: of its first 16 bytes and every entry sector.
fn txn_digest(record: &[u8]) -> [u8; 32] {
    let mut h = sha256::Sha256::new();
    h.update(&record[..16]);
    h.update(&record[SECTOR..]);
    h.finish()
}

fn pin_record(pin: &Pin) -> [u8; SECTOR] {
    let mut r = [0u8; SECTOR];
    r[..8].copy_from_slice(PIN_MAGIC);
    r[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
    r[12..16].copy_from_slice(&pin.id.to_le_bytes());
    r[16..18].copy_from_slice(&pin.owner.to_le_bytes());
    r[20..56].copy_from_slice(&pin.root);
    let check = sha256::digest(&r[..56]);
    r[56..PIN_RECORD].copy_from_slice(&check);
    r
}

/// The pin a sector records, if it is a whole, valid pin record of this layout.
fn parse_pin(sector: &[u8]) -> Option<Pin> {
    if &sector[..8] != PIN_MAGIC || sector[56..PIN_RECORD] != sha256::digest(&sector[..56]) { return None; }
    if u16::from_le_bytes([sector[8], sector[9]]) != LAYOUT || sector[10..12] != [0, 0] || sector[18..20] != [0, 0] { return None; }
    if sector[PIN_RECORD..].iter().any(|&b| b != 0) { return None; }
    Cid::from_bytes(&sector[20..56]).ok()?;
    let id = u32::from_le_bytes(sector[12..16].try_into().unwrap());
    Some(Pin { id, owner: u16::from_le_bytes([sector[16], sector[17]]), root: sector[20..56].try_into().unwrap(), lba: 0, size: 0 })
}

pub struct Store<'a, D: Device> {
    dev: D,
    index: &'a mut [Entry],
    count: usize,
    heads: &'a mut [Head],
    names: usize,
    pins: &'a mut [Pin],
    pinned: usize,
    next_pin: u32,
    // What each owner may retain: three quarters of the medium, until whoever grants the clients sets quotas.
    quota: u64,
    // Blank runs, sorted; runs past its length are not used until the next scan.
    holes: &'a mut [Extent],
    runs: usize,
    buffer: &'a mut [u8; BUFFER],
    // For walks of objects (publish, collect); taken while one runs, since the walk reads through the store itself.
    scratch: Option<&'a mut [u8; dag::CHUNK]>,
    now: u64,
    end: u64,
    bytes: u64,
    corrupt: u32,
    damaged: u64,
}

fn free_marker(sectors: u64) -> [u8; SECTOR] {
    let mut r = [0u8; SECTOR];
    r[..8].copy_from_slice(FREE_MAGIC);
    r[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
    r[12..16].copy_from_slice(&(sectors as u32).to_le_bytes());
    let check = sha256::digest(&r[..16]);
    r[16..48].copy_from_slice(&check);
    r
}

/// The sectors a free marker covers, if the sector is one.
fn parse_free(sector: &[u8]) -> Option<u64> {
    if &sector[..8] != FREE_MAGIC || sector[16..48] != sha256::digest(&sector[..16]) || sector[48..].iter().any(|&b| b != 0) { return None; }
    if u16::from_le_bytes([sector[8], sector[9]]) != LAYOUT || sector[10..12] != [0, 0] { return None; }
    let n = u32::from_le_bytes(sector[12..16].try_into().unwrap()) as u64;
    (n >= 1).then_some(n)
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
    /// Mounts the store on `dev` at time `now` (ns): a blank medium is formatted (if writable), a store is scanned and
    /// its blocks verified; anything else is refused. `index` bounds how many blocks it can hold, `heads` how many
    /// names, `pins` how many pins, `holes` how many runs of blank sectors it uses. Every block starts a lease at `now`.
    #[allow(clippy::too_many_arguments)]
    pub fn mount(mut dev: D, index: &'a mut [Entry], heads: &'a mut [Head], pins: &'a mut [Pin], holes: &'a mut [Extent],
                 buffer: &'a mut [u8; BUFFER], scratch: &'a mut [u8; dag::CHUNK], now: u64) -> Result<Self, Error> {
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
        let quota = dev.sectors() * SECTOR as u64 / 4 * 3;
        let mut store = Store {
            dev, index, count: 0, heads, names: 0, pins, pinned: 0, next_pin: 1, quota, holes, runs: 0, buffer,
            scratch: Some(scratch), now, end: 1, bytes: 0, corrupt: 0, damaged: 0,
        };
        store.pass(false)?;
        store.size_retained();
        Ok(store)
    }

    /// The time (ns, monotonic) the next requests happen at: leases are measured with it.
    pub fn set_time(&mut self, now: u64) { self.now = now; }

    /// Starts every block's lease at `now`: called when a mount is done, since verifying a whole medium can take longer
    /// than a lease, and a restart must not shorten the time a writer has.
    pub fn renew(&mut self, now: u64) {
        self.now = now;
        for e in self.index[..self.count].iter_mut() { e.lease = now; }
    }

    // Reads the medium in windows of RECORD_SECTORS and finds the runs of blank sectors. Mounting (`sweep` false)
    // reads every record whole, verifies it and builds the index and the names. A sweep frees the block records the
    // index does not retain at that place and the name records that are not current; nothing else is written.
    fn pass(&mut self, sweep: bool) -> Result<Collected, Error> {
        let sectors = self.dev.sectors();
        let mut freed = Collected::default();
        let (mut lba, mut run) = (1u64, None::<u64>);
        (self.runs, self.end, self.damaged) = (0, 1, 0);
        if sweep { self.corrupt = 0; }
        while lba < sectors {
            let window = (sectors - lba).min(RECORD_SECTORS as u64) as usize;
            if !self.dev.read(lba, &mut self.buffer[..window * SECTOR]) { return Err(Error::Device); }
            let mut at = 0;
            let mut next = lba + window as u64;
            while at < window {
                let here = lba + at as u64;
                let sector = &self.buffer[at * SECTOR..(at + 1) * SECTOR];
                if let Some((cid, len)) = parse_header(sector).filter(|&(_, len)| here + record_sectors(len) as u64 <= sectors) {
                    let n = record_sectors(len) as u64;
                    let keep = if sweep {
                        match self.position(&cid.to_bytes()) {
                            Ok(i) if self.index[i].lba == here && self.retained(i) => true,
                            // The indexed copy leaves the index before its sectors are erased, so a failure later in
                            // the sweep never leaves an erased block acknowledged (audit A05, 175-STO-0012).
                            Ok(i) if self.index[i].lba == here => { self.remove(i); false }
                            _ => false,
                        }
                    } else {
                        if self.verify(here, &cid, len)? && !self.contains(&cid) {
                            if self.count == self.index.len() { return Err(Error::Full); }
                            self.insert(cid.to_bytes(), here, len);
                        }
                        true
                    };
                    if keep { self.occupied(&mut run, here, n); } else { self.erase(here, n)?; run.get_or_insert(here); freed.blocks += 1; freed.sectors += n; }
                    next = here + n;
                    break;
                }
                if let Some(count) = parse_txn(sector).filter(|&c| here + 1 + c as u64 <= sectors) {
                    // A commit applies whole or not at all: its digest covers every entry (MC-4.10).
                    let n = 1 + count as u64;
                    if !self.dev.read(here, &mut self.buffer[..n as usize * SECTOR]) { return Err(Error::Device); }
                    match self.commit_entries(count) {
                        Some(entries) => {
                            let entries = &entries[..count];
                            let kept = if sweep {
                                entries.iter().any(|(name, len, v)| self.find(&name[..*len]).is_ok_and(|i| self.heads[i].versions().iter().any(|k| k.version == v.version)))
                            } else {
                                for (name, len, v) in entries { self.keep_version(&name[..*len], *v)?; }
                                true
                            };
                            if kept { self.occupied(&mut run, here, n); } else {
                                self.erase(here, n)?;
                                run.get_or_insert(here);
                                freed.names += count as u32;
                                freed.sectors += n;
                            }
                            next = here + n;
                        }
                        None => {
                            // A damaged commit: its header is counted, its entries are counted as the scan meets them.
                            self.damaged += 1;
                            self.occupied(&mut run, here, 1);
                            next = here + 1;
                        }
                    }
                    break;
                }
                if let Some((name, len, v)) = parse_name(sector) {
                    let name = &name[..len];
                    let head = self.find(name);
                    if !sweep {
                        match head {
                            Ok(i) => self.heads[i].keep(v),
                            Err(i) => { if self.names == self.heads.len() { return Err(Error::Full); } self.add(i, Head::new(name, v)); }
                        }
                    } else if !head.is_ok_and(|i| self.heads[i].versions().iter().any(|k| k.version == v.version)) {
                        // A version the name no longer keeps.
                        self.erase(here, 1)?;
                        run.get_or_insert(here);
                        freed.names += 1;
                        freed.sectors += 1;
                        next = here + 1;
                        break;
                    }
                    self.occupied(&mut run, here, 1);
                    at += 1;
                    continue;
                }
                if let Some(mut pin) = parse_pin(sector) {
                    let held = self.pins[..self.pinned].iter().position(|p| p.id == pin.id);
                    if !sweep {
                        if held.is_none() {
                            if self.pinned == self.pins.len() { return Err(Error::Full); }
                            pin.lba = here;
                            self.pins[self.pinned] = pin;
                            self.pinned += 1;
                            self.next_pin = self.next_pin.max(pin.id.wrapping_add(1));
                        }
                    } else if !held.is_some_and(|k| self.pins[k].lba == here) {
                        self.erase(here, 1)?;
                        run.get_or_insert(here);
                        freed.sectors += 1;
                        next = here + 1;
                        break;
                    }
                    self.occupied(&mut run, here, 1);
                    at += 1;
                    continue;
                }
                if let Some(n) = parse_free(sector).filter(|&n| here + n <= sectors) {
                    // A collection stopped in the middle of freeing this: finish it.
                    if self.dev.writable() { self.erase(here, n)?; run.get_or_insert(here); } else { self.occupied(&mut run, here, n); }
                    next = here + n;
                    break;
                }
                if sector.iter().any(|&b| b != 0) {
                    self.damaged += 1;
                    self.occupied(&mut run, here, 1);
                } else {
                    run.get_or_insert(here);
                }
                at += 1;
            }
            lba = next;
        }
        if let Some(start) = run { self.hole(start, sectors - start); }
        if sweep {
            // An entry the sweep did not meet and does not retain leaves the index too (its record was not found).
            let mut k = 0;
            for i in 0..self.count {
                if self.retained(i) { self.index[k] = self.index[i]; k += 1; } else { self.bytes -= self.index[i].len as u64; }
            }
            self.count = k;
        }
        freed.free = self.holes[..self.runs].iter().map(|h| h.len).sum();
        Ok(freed)
    }

    // A version found on the medium: the name keeps it among its newest.
    fn keep_version(&mut self, name: &[u8], v: Version) -> Result<(), Error> {
        match self.find(name) {
            Ok(i) => self.heads[i].keep(v),
            Err(i) => { if self.names == self.heads.len() { return Err(Error::Full); } self.add(i, Head::new(name, v)); }
        }
        Ok(())
    }

    // The entries of the commit read into the buffer, if its digest and every entry check.
    fn commit_entries(&self, count: usize) -> Option<[([u8; NAME_MAX], usize, Version); COMMIT_MAX]> {
        let record = &self.buffer[..(1 + count) * SECTOR];
        if record[16..48] != txn_digest(record) { return None; }
        let mut entries = [([0u8; NAME_MAX], 0, Version::EMPTY); COMMIT_MAX];
        for (k, entry) in entries.iter_mut().take(count).enumerate() {
            *entry = parse_entry(&record[(1 + k) * SECTOR..(2 + k) * SECTOR])?;
            let name = &entry.0[..entry.1];
            // One entry per name.
            if record[SECTOR..(1 + k) * SECTOR].chunks(SECTOR).any(|s| parse_entry(s).is_some_and(|(n, l, _)| &n[..l] == name)) { return None; }
        }
        Some(entries)
    }

    // A record or damaged sector at `here`: the blank run before it ends.
    fn occupied(&mut self, run: &mut Option<u64>, here: u64, n: u64) {
        if let Some(start) = run.take() { self.hole(start, here - start); }
        self.end = self.end.max(here + n);
    }

    fn hole(&mut self, start: u64, len: u64) {
        if len > 0 && self.runs < self.holes.len() { self.holes[self.runs] = Extent { start, len }; self.runs += 1; }
    }

    // Sectors made blank outside a scan: back among the runs, in order (dropped if the list is full until the next scan).
    fn release(&mut self, start: u64, len: u64) {
        if self.runs == self.holes.len() { return; }
        let at = self.holes[..self.runs].iter().position(|h| h.start > start).unwrap_or(self.runs);
        self.holes.copy_within(at..self.runs, at + 1);
        self.holes[at] = Extent { start, len };
        self.runs += 1;
    }

    // Frees `n` sectors at `lba`: a marker first, so a scan after a stop in the middle finishes it, then zeros.
    fn erase(&mut self, lba: u64, n: u64) -> Result<(), Error> {
        if !self.dev.write(lba, &free_marker(n)) || !self.dev.flush() { return Err(Error::Device); }
        self.buffer.fill(0);
        let mut at = lba + 1;
        while at < lba + n {
            let k = (lba + n - at).min(RECORD_SECTORS as u64) as usize;
            if !self.dev.write(at, &self.buffer[..k * SECTOR]) { return Err(Error::Device); }
            at += k as u64;
        }
        if !self.dev.flush() || !self.dev.write(lba, &self.buffer[..SECTOR]) || !self.dev.flush() { return Err(Error::Device); }
        Ok(())
    }

    // The first run of blank sectors that holds `n`.
    fn find_room(&self, n: u64) -> Option<u64> { self.holes[..self.runs].iter().find(|h| h.len >= n).map(|h| h.start) }

    // Takes `n` sectors at `lba`, the start of a run.
    fn claim(&mut self, lba: u64, n: u64) {
        if let Some(i) = self.holes[..self.runs].iter().position(|h| h.start == lba) {
            self.holes[i].start += n;
            self.holes[i].len -= n;
            if self.holes[i].len == 0 { self.holes.copy_within(i + 1..self.runs, i); self.runs -= 1; }
        }
        self.end = self.end.max(lba + n);
    }

    // Room for `n` sectors, after a collection if there is none.
    fn room(&mut self, n: u64) -> Result<u64, Error> {
        if let Some(lba) = self.find_room(n) { return Ok(lba); }
        self.collect()?;
        self.find_room(n).ok_or(Error::Full)
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
    // Whether a collection keeps the block: a name retains it, or its lease runs.
    fn retained(&self, i: usize) -> bool { self.index[i].live || self.now < self.index[i].lease.saturating_add(LEASE_NS) }

    fn insert(&mut self, key: [u8; cid::BYTES], lba: u64, len: usize) {
        let at = self.position(&key).unwrap_err();
        self.index.copy_within(at..self.count, at + 1);
        self.index[at] = Entry { key, lba, len: len as u32, lease: self.now, live: false };
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

    /// The current version and root of `name`; not found once it is removed.
    pub fn resolve(&self, name: &[u8]) -> Result<(u64, Cid), Error> {
        if !valid_name(name) { return Err(Error::Invalid); }
        let head = &self.heads[self.find(name).map_err(|_| Error::NotFound)?];
        head.latest().root().map(|root| (head.latest().version, root)).ok_or(Error::NotFound)
    }

    /// The versions of `name` the store keeps, newest first (a version without a root removed the name).
    pub fn history(&self, name: &[u8]) -> Result<&[Version], Error> {
        if !valid_name(name) { return Err(Error::Invalid); }
        Ok(self.heads[self.find(name).map_err(|_| Error::NotFound)?].versions())
    }

    // Walks the object `root` through the store; `visit` sees the store and each block's CID.
    fn walk(&mut self, root: &Cid, visit: impl FnMut(&mut Self, &Cid) -> Result<(), dag::Error>) -> Result<u64, Error> {
        let scratch = self.scratch.take().ok_or(Error::Device)?;
        let walked = dag::walk(self, root, scratch, visit);
        self.scratch = Some(scratch);
        walked.map_err(|e| match e {
            dag::Error::NotFound => Error::Incomplete,
            dag::Error::Corrupt => Error::Corrupt,
            dag::Error::Store => Error::Device,
            _ => Error::Invalid,
        })
    }

    // The size of every retained object, for the owners' accounts: read from each root once a mount is done.
    fn size_retained(&mut self) {
        let Some(scratch) = self.scratch.take() else { return };
        for i in 0..self.names {
            for k in 0..self.heads[i].count as usize {
                if let Some(root) = self.heads[i].kept[k].root() { self.heads[i].kept[k].size = dag::size(self, &root, scratch).unwrap_or(0); }
            }
        }
        for i in 0..self.pinned { let root = self.pins[i].root(); self.pins[i].size = dag::size(self, &root, scratch).unwrap_or(0); }
        self.scratch = Some(scratch);
    }

    // The roots `owner` retains through its names' kept versions and its pins, with their objects' sizes.
    fn owned(&self, owner: u16) -> impl Iterator<Item = ([u8; cid::BYTES], u64)> + '_ {
        self.heads[..self.names].iter().flat_map(|h| h.retains()).filter(move |v| v.owner == owner).map(|v| (v.root, v.size))
            .chain(self.pins[..self.pinned].iter().filter(move |p| p.owner == owner).map(|p| (p.root, p.size)))
    }

    // The bytes `owner` retains, each root counted once however many names and pins retain it; with the roots of
    // `extra` added unless it holds them already.
    fn retained_by(&self, owner: u16, extra: &[([u8; cid::BYTES], u64)]) -> u64 {
        let mut total = 0;
        for (k, (root, size)) in self.owned(owner).enumerate() {
            if !self.owned(owner).take(k).any(|(r, _)| r == root) { total += size; }
        }
        for (k, (root, size)) in extra.iter().enumerate() {
            if !self.owned(owner).any(|(r, _)| r == *root) && !extra[..k].iter().any(|(r, _)| r == root) { total += size; }
        }
        total
    }

    /// What `owner` retains and may retain.
    pub fn usage(&self, owner: u16) -> Usage {
        Usage {
            retained: self.retained_by(owner, &[]),
            quota: self.quota,
            names: self.heads[..self.names].iter().filter(|h| !h.removed() && h.latest().owner == owner).count() as u32,
            pins: self.pins[..self.pinned].iter().filter(|p| p.owner == owner).count() as u32,
        }
    }

    /// Publishes `root` as the next version of `name` for `owner` if `expected` is its current version (0: a new
    /// name; after a removal, the removal's version), once every block of the object `root` names is stored (MC-4.3,
    /// 4.4) and within the owner's quota (MC-4.11); returns the new version after the device has flushed it. The
    /// record links the version before; the name keeps its last HISTORY versions.
    pub fn publish(&mut self, name: &[u8], expected: u64, root: &Cid, owner: u16) -> Result<u64, Error> {
        if !valid_name(name) { return Err(Error::Invalid); }
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let current = self.find(name).map_or(0, |i| self.heads[i].latest().version);
        if expected != current { return Err(Error::Conflict); }
        if self.find(name).is_err() && self.names == self.heads.len() { return Err(Error::Full); }
        // Room first: a collection it starts may free blocks of `root` whose lease ended, and the check below sees it.
        let lba = self.room(1)?;
        let size = self.walk(root, |s, cid| if s.contains(cid) { Ok(()) } else { Err(dag::Error::NotFound) })?;
        if self.retained_by(owner, &[(root.to_bytes(), size)]) > self.quota { return Err(Error::Quota); }
        let previous = self.find(name).ok().and_then(|i| self.heads[i].latest().root());
        let v = Version { version: current + 1, owner, root: root.to_bytes(), points: true, size };
        self.claim(lba, 1);
        if !self.dev.write(lba, &name_record(name, &v, previous)) || !self.dev.flush() { return Err(Error::Device); }
        match self.find(name) { Ok(i) => self.heads[i].keep(v), Err(i) => self.add(i, Head::new(name, v)) }
        Ok(v.version)
    }

    /// Removes `name` for `owner` if `expected` is its current version: a version without a root, which ends what the
    /// name retains (deleting a reference, not data: the objects go when a collection finds nothing else retains
    /// them, MC-4.8). Returns the removal's version; publishing from it creates the name again.
    pub fn unpublish(&mut self, name: &[u8], expected: u64, owner: u16) -> Result<u64, Error> {
        if !valid_name(name) { return Err(Error::Invalid); }
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let i = self.find(name).map_err(|_| Error::NotFound)?;
        if self.heads[i].removed() { return Err(Error::NotFound); }
        if expected != self.heads[i].latest().version { return Err(Error::Conflict); }
        let lba = self.room(1)?;
        let i = self.find(name).map_err(|_| Error::NotFound)?;
        let previous = self.heads[i].latest().root();
        let v = Version { version: expected + 1, owner, ..Version::EMPTY };
        self.claim(lba, 1);
        if !self.dev.write(lba, &name_record(name, &v, previous)) || !self.dev.flush() { return Err(Error::Device); }
        self.heads[i].keep(v);
        Ok(v.version)
    }

    /// Changes up to COMMIT_MAX names for `owner` at once, all or none (MC-4.10): each from the version it expects, to
    /// a complete root or to a removal, within the owner's quota. One record holds every change and one digest covers
    /// it, so a mount finds all of them or none; returns the new versions after the device has flushed it.
    pub fn commit(&mut self, updates: &[Update], owner: u16) -> Result<[u64; COMMIT_MAX], Error> {
        let n = updates.len();
        if n == 0 || n > COMMIT_MAX { return Err(Error::Invalid); }
        for (k, u) in updates.iter().enumerate() {
            if !valid_name(u.name) || updates[..k].iter().any(|w| w.name == u.name) { return Err(Error::Invalid); }
        }
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let mut new = 0;
        for u in updates {
            let current = match self.find(u.name) {
                Ok(i) if u.root.is_none() && self.heads[i].removed() => return Err(Error::NotFound),
                Ok(i) => self.heads[i].latest().version,
                Err(_) if u.root.is_none() => return Err(Error::NotFound),
                Err(_) => { new += 1; 0 }
            };
            if u.expected != current { return Err(Error::Conflict); }
        }
        if self.names + new > self.heads.len() { return Err(Error::Full); }
        // Room first: a collection it starts may free blocks whose lease ended, and the checks below see it.
        let lba = self.room(1 + n as u64)?;
        let mut extra = [([0u8; cid::BYTES], 0u64); COMMIT_MAX];
        for (k, u) in updates.iter().enumerate() {
            if let Some(root) = u.root {
                let size = self.walk(&root, |s, cid| if s.contains(cid) { Ok(()) } else { Err(dag::Error::NotFound) })?;
                extra[k] = (root.to_bytes(), size);
            }
        }
        // A removal adds a zero root of no size: it changes nothing.
        if self.retained_by(owner, &extra[..n]) > self.quota { return Err(Error::Quota); }
        let mut versions = [0u64; COMMIT_MAX];
        let record = &mut self.buffer[..(1 + n) * SECTOR];
        record.fill(0);
        record[..8].copy_from_slice(TXN_MAGIC);
        record[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
        record[10..12].copy_from_slice(&(n as u16).to_le_bytes());
        for (k, u) in updates.iter().enumerate() {
            let head = self.heads[..self.names].binary_search_by(|h| h.name().cmp(u.name)).ok().map(|i| &self.heads[i]);
            let previous = head.and_then(|h| h.latest().root());
            versions[k] = head.map_or(0, |h| h.latest().version) + 1;
            let v = Version { version: versions[k], owner, root: u.root.map_or([0; cid::BYTES], |r| r.to_bytes()), points: u.root.is_some(), size: extra[k].1 };
            let mut entry = name_record(u.name, &v, previous);
            entry[..8].fill(0);
            entry[160..NAME_RECORD].fill(0);
            record[(1 + k) * SECTOR..(2 + k) * SECTOR].copy_from_slice(&entry);
        }
        let check = txn_digest(record);
        record[16..48].copy_from_slice(&check);
        self.claim(lba, 1 + n as u64);
        if !self.dev.write(lba, &self.buffer[..(1 + n) * SECTOR]) || !self.dev.flush() { return Err(Error::Device); }
        for (k, u) in updates.iter().enumerate() {
            let v = Version { version: versions[k], owner, root: u.root.map_or([0; cid::BYTES], |r| r.to_bytes()), points: u.root.is_some(), size: extra[k].1 };
            self.keep_version(u.name, v)?;
        }
        Ok(versions)
    }

    /// The current version and root of each name, read between two requests (no root: removed; version 0: none).
    pub fn snapshot(&self, name: &[u8]) -> Result<(u64, Option<Cid>), Error> {
        if !valid_name(name) { return Err(Error::Invalid); }
        Ok(self.find(name).map_or((0, None), |i| (self.heads[i].latest().version, self.heads[i].latest().root())))
    }

    /// Pins the object `root` for `owner` until it unpins it, once every block of it is stored and within the owner's
    /// quota; returns the pin's id after the device has flushed it.
    pub fn pin(&mut self, root: &Cid, owner: u16) -> Result<u32, Error> {
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        if self.pinned == self.pins.len() { return Err(Error::Full); }
        let lba = self.room(1)?;
        let size = self.walk(root, |s, cid| if s.contains(cid) { Ok(()) } else { Err(dag::Error::NotFound) })?;
        if self.retained_by(owner, &[(root.to_bytes(), size)]) > self.quota { return Err(Error::Quota); }
        let pin = Pin { id: self.next_pin, owner, root: root.to_bytes(), lba, size };
        self.claim(lba, 1);
        if !self.dev.write(lba, &pin_record(&pin)) || !self.dev.flush() { return Err(Error::Device); }
        self.next_pin = self.next_pin.wrapping_add(1).max(1);
        self.pins[self.pinned] = pin;
        self.pinned += 1;
        Ok(pin.id)
    }

    /// Ends `owner`'s pin `id`: its record is freed at once; the object goes when nothing else retains it.
    pub fn unpin(&mut self, id: u32, owner: u16) -> Result<(), Error> {
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let k = self.pins[..self.pinned].iter().position(|p| p.id == id).ok_or(Error::NotFound)?;
        if self.pins[k].owner != owner { return Err(Error::Rights); }
        let lba = self.pins[k].lba;
        self.erase(lba, 1)?;
        self.pins.copy_within(k + 1..self.pinned, k);
        self.pinned -= 1;
        self.release(lba, 1);
        Ok(())
    }

    /// The pins of `owner`.
    pub fn pins(&self, owner: u16) -> impl Iterator<Item = &Pin> { self.pins[..self.pinned].iter().filter(move |p| p.owner == owner) }

    /// Frees what no name retains, no pin holds and no lease protects: unretained blocks, other copies of a block
    /// (corrupt ones too), name versions a name no longer keeps (MC-4.5). Every retained object is walked first, every
    /// node read and checked; if one lacks a block or holds a corrupt node, nothing is freed.
    pub fn collect(&mut self) -> Result<Collected, Error> {
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        for e in self.index[..self.count].iter_mut() { e.live = false; }
        let mark = |s: &mut Self, cid: &Cid| match s.position(&cid.to_bytes()) {
            Ok(k) => { s.index[k].live = true; Ok(()) }
            Err(_) => Err(dag::Error::NotFound),
        };
        for i in 0..self.names {
            for k in 0..self.heads[i].count as usize {
                let head = &self.heads[i];
                if head.removed() { break; }
                if let Some(root) = head.kept[k].root() { self.walk(&root, mark)?; }
            }
        }
        for i in 0..self.pinned { let root = self.pins[i].root(); self.walk(&root, mark)?; }
        self.pass(true)
    }

    /// Stores `data` as a block of type `codec` and returns its CID, once the device has flushed it. A node must decode
    /// first; a block already held is not written again, and its lease starts again. Without room a collection runs.
    pub fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, Error> {
        if data.len() > BLOCK_MAX { return Err(Error::TooLarge); }
        if !typed(codec, data) { return Err(Error::Invalid); }
        let cid = Cid::of(codec, data);
        if let Ok(i) = self.position(&cid.to_bytes()) { self.index[i].lease = self.now; return Ok(cid); }
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let n = record_sectors(data.len()) as u64;
        if self.count == self.index.len() { self.collect()?; }
        if self.count == self.index.len() { return Err(Error::Full); }
        let lba = self.room(n)?;
        let record = &mut self.buffer[..n as usize * SECTOR];
        record.fill(0);
        record[..HEADER].copy_from_slice(&header(&cid, data.len()));
        record[HEADER..HEADER + data.len()].copy_from_slice(data);
        // Only a header sector starts with a record's magic, so a scan that resumes after a damaged sector cannot take
        // a client's bytes for a record of any kind.
        if record.chunks(SECTOR).skip(1).any(|s| [RECORD_MAGIC, NAME_MAGIC, FREE_MAGIC, PIN_MAGIC, TXN_MAGIC].contains(&s[..8].try_into().unwrap())) { return Err(Error::Invalid); }
        // The sectors are taken even if the write fails: they may hold part of it now, and are not written again.
        self.claim(lba, n);
        if !self.dev.write(lba, &self.buffer[..n as usize * SECTOR]) || !self.dev.flush() { return Err(Error::Device); }
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
            free: self.holes[..self.runs].iter().map(|h| h.len).sum(), pins: self.pinned as u32,
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
