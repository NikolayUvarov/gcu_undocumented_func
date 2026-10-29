//! The block store's layout and logic (issues 300-STO-0002, 301-STO-0002, 302-STO-0001, 303-STO-0001; MC-4.2, 4.3,
//! 4.4, 4.5, 4.8): immutable blocks named by their CID, and names whose current version points at a root, as records on
//! a block device; what no name retains and no lease protects is collected; several names change at once in one
//! commit record (304-STO-0007). Nothing here depends on the kernel: the host tests build it too.
//!
//! Sector 0 holds the superblock and the volume key, or only the id a keeper keeps it under (308-STO-0022). Records follow from sector 1, each starting on a sector and sealed
//! (308-STO-0021): a plain frame (magic, layout, domain, the sectors it covers, nonce, tag), then the record and the
//! zeros up to the end of its last sector, encrypted and authenticated with ChaCha20-Poly1305 under the volume key, the
//! frame and the record's first sector as associated data. Nothing but the frame is in the clear, and it does not say
//! what kind of record it seals. A block record is a header (magic, layout version, length, the block's CID, a digest of the header) and
//! the block's bytes; a `dag-cbor` block must be a node of `dag`. A name record is the name, its version and root, and
//! a digest. Records are written only into blank sectors, so nothing stored is overwritten; a collection makes the
//! sectors of what it frees blank again. Every block read is opened and checked against its CID, and one that does not
//! open or match is reported corrupt, never returned.
//!
//! Memory domains (308-STO-0023): a domain's records are sealed under a key of its own, random, kept in a key record
//! sealed under the volume key. Names, pins and blocks belong to a domain; the same bytes in two domains are two
//! records. Forgetting a domain writes a tombstone and erases its key record, so its records never open again, and the
//! next collection frees them.
use crate::cid::{self, Cid, Codec};
use crate::dag;
use crate::sha256;

pub const SECTOR: usize = 512;
/// Largest block a put takes.
pub const BLOCK_MAX: usize = 16384;
/// Bytes of a record header.
pub const HEADER: usize = 84;
/// Bytes of a sealed record's frame: magic, layout, domain (u16), the sectors it covers (u32), nonce (12) and tag (16).
pub const FRAME: usize = 44;
/// Sectors of the largest record; the store's buffer holds one.
pub const RECORD_SECTORS: usize = sealed_sectors(HEADER + BLOCK_MAX);
pub const BUFFER: usize = RECORD_SECTORS * SECTOR;
/// The layout's version; another is refused, never read as this one (MC-4.13).
pub const LAYOUT: u16 = 4;
const SUPER_MAGIC: &[u8; 8] = b"MIND-STO";
// The superblock: magic, layout, sector size (u16), the key's source, zero, the SHA-256 of these 16 bytes; the volume
// key; a check (32 zero bytes sealed under it, as nonce, ciphertext and tag); and the SHA-256 of the 140 bytes before.
const SUPER: usize = 172;
/// The key's sources: none yet (a superblock written by make_usb_image.py), kept in the superblock in the clear, or
/// kept by a keeper off the medium, the superblock holding only its id (308-STO-0022).
pub const KEY_NONE: u8 = 0;
pub const KEY_CLEAR: u8 = 1;
pub const KEY_KEPT: u8 = 2;
const FRAME_MAGIC: &[u8; 8] = b"MIND-SLD";
const RECORD_MAGIC: &[u8; 8] = b"MIND-BLK";
const NAME_MAGIC: &[u8; 8] = b"MIND-REF";
// A record being freed: magic, layout, zero, the sectors it covers (u32), and the SHA-256 of these 16 bytes.
const FREE_MAGIC: &[u8; 8] = b"MIND-DEL";
const FREE_RECORD: usize = 48;
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
/// The volume's own domain: its records are sealed under keys from the volume key (308-STO-0023).
pub const VOLUME: u16 = 0;
// The frame's domain of the records that keep the domains' keys, sealed under the volume key.
const KEYS: u16 = 0xFFFF;
/// Domains a store keeps at most, live and forgotten together.
pub const DOMAINS: usize = 32;
// A domain's key record: magic, layout, the domain (u16), its state (u16), the owner (u16), zero (4), the key (32
// bytes, zeros once forgotten), and the SHA-256 of these 52 bytes.
const KEY_MAGIC: &[u8; 8] = b"MIND-KEY";
const KEY_RECORD: usize = 52 + 32;
const KEY_LIVE: u16 = 1;
const KEY_GONE: u16 = 2;

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

/// The cipher records are sealed with: ChaCha20-Poly1305 (RFC 8439) on the platform, a stand-in in the host tests.
pub trait Crypto {
    /// Encrypts `data` in place and returns the tag over `aad` and the ciphertext.
    fn seal(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16];
    /// Decrypts `data` in place if `tag` matches `aad` and the ciphertext; false, `data` unchanged, if not.
    fn open(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], tag: &[u8; 16]) -> bool;
    /// Fills `out` with random bytes; false without a random source.
    fn random(&self, out: &mut [u8]) -> bool;
}

/// Where a volume key is kept off the medium (308-STO-0022): `keystore`'s secrets on the platform.
pub trait Keeper {
    /// Keeps `key` under `id`; false if it was not kept.
    fn put(&mut self, id: &[u8; 8], key: &[u8; 32]) -> bool;
    /// The key kept under `id`, if one is.
    fn get(&mut self, id: &[u8; 8]) -> Option<[u8; 32]>;
}

/// HMAC-SHA-256 (RFC 2104) of the concatenated `parts` under a key of 32 bytes.
pub fn hmac_sha256(key: &[u8; 32], parts: &[&[u8]]) -> [u8; 32] {
    let mut pad = [0x36u8; 64];
    for (p, k) in pad.iter_mut().zip(key) { *p ^= k; }
    let mut inner = sha256::Sha256::new();
    inner.update(&pad);
    for part in parts { inner.update(part); }
    let inner = inner.finish();
    for p in pad.iter_mut() { *p ^= 0x36 ^ 0x5c; }
    let mut outer = sha256::Sha256::new();
    outer.update(&pad);
    outer.update(&inner);
    outer.finish()
}

// The keys a volume or domain key gives: one seals records, one makes their nonces.
#[derive(Clone, Copy)]
struct Keys { seal: [u8; 32], nonce: [u8; 32] }
impl Keys {
    const NONE: Keys = Keys { seal: [0; 32], nonce: [0; 32] };
    fn new(volume: &[u8; 32]) -> Self { Keys { seal: hmac_sha256(volume, &[b"MIND-STO seal"]), nonce: hmac_sha256(volume, &[b"MIND-STO nonce"]) } }
    // Seals `data` in place; returns its nonce and tag. The nonce comes from the record and its associated data, so a
    // write needs no random source; it repeats only for the same record at the same place, which seals the same way.
    fn seal(&self, crypto: &dyn Crypto, aad: &[u8], data: &mut [u8]) -> ([u8; 12], [u8; 16]) {
        let nonce: [u8; 12] = hmac_sha256(&self.nonce, &[aad, data])[..12].try_into().unwrap();
        (nonce, crypto.seal(&self.seal, &nonce, aad, data))
    }
}

/// Sectors of a sealed record of `len` bytes.
pub const fn sealed_sectors(len: usize) -> usize { (FRAME + len).div_ceil(SECTOR) }

// The sectors a frame covers, if the sector starts a frame of this layout.
fn parse_frame(sector: &[u8]) -> Option<usize> {
    if &sector[..8] != FRAME_MAGIC || u16::from_le_bytes([sector[8], sector[9]]) != LAYOUT { return None; }
    let n = u32::from_le_bytes(sector[12..16].try_into().unwrap()) as usize;
    (1..=RECORD_SECTORS).contains(&n).then_some(n)
}

// The associated data of a sealed record: its frame's first 16 bytes and its first sector, so a copy elsewhere fails.
fn aad(frame: &[u8], lba: u64) -> [u8; 24] {
    let mut a = [0u8; 24];
    a[..16].copy_from_slice(&frame[..16]);
    a[16..].copy_from_slice(&lba.to_le_bytes());
    a
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
    /// A store of another layout version, the one found: this build reads only its own and converts none.
    Layout(u16),
    /// The superblock's key does not open its check, or comes from a source this build does not know: nothing is read.
    Key,
    /// The volume key is kept off the medium under this id, and no keeper gave it: none to ask, or another machine's
    /// medium.
    Kept([u8; 8]),
    /// A `dag-cbor` block that is not a node of `dag`'s schema (MC-4.2: the type is bound to the data); a name that is
    /// not 1 to NAME_MAX allowed bytes; a root whose tree is out of shape.
    Invalid,
    /// The name's current version is not the one the publisher expected (MC-4.3): nothing was published.
    Conflict,
    /// A block of the root's object is not stored (MC-4.4): nothing was published; or a name's object lacks one, so
    /// nothing was collected.
    Incomplete,
    /// The owner would retain more than its quota (MC-4.11): nothing was published or pinned.
    Quota,
    /// A pin or a domain of another owner.
    Rights,
    /// No random source for a domain's key: one derived from anything kept could not be destroyed (308-STO-0023).
    NoRandom,
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

/// What a scrub found (309-STO-0025): records read and checked, those that did not open or are not whole, and
/// non-blank sectors outside every record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scrubbed { pub records: u32, pub corrupt: u32, pub damaged: u64 }

/// A slot of the index, a hash table with open addressing (251-STO-0013): when the block's lease began, where its
/// record starts (a sector below 2^32), its length, whether the slot is used and whether the last collection found the
/// block retained, the CID's binary form and the block's domain. 56 bytes; all zeros is an empty slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry { lease: u64, lba: u32, len: u16, flags: u8, key: [u8; cid::BYTES], domain: u16 }
impl Entry {
    pub const EMPTY: Entry = Entry { lease: 0, lba: 0, len: 0, flags: 0, key: [0; cid::BYTES], domain: VOLUME };
    fn used(&self) -> bool { self.flags & USED != 0 }
    fn live(&self) -> bool { self.flags & LIVE != 0 }
}
const USED: u8 = 1;
const LIVE: u8 = 2;

/// Index slots for a medium of `sectors`: room for a block per 8 sectors (records of 4 KiB on average) and at least
/// 4096 blocks, at most 2^20 slots. The service gives the store as many as its memory allows, down to the least.
pub fn slots_for(sectors: u64) -> usize { ((sectors / 8).max(4096) * 8 / 7).min(1 << 20) as usize }

// The slot a key starts its search at: its digest's first bytes are uniform, the codec and the domain tell copies apart.
fn home(key: &[u8; cid::BYTES], domain: u16, slots: usize) -> usize {
    let h = u64::from_le_bytes(key[4..12].try_into().unwrap()) ^ (key[1] as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (domain as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    (h % slots as u64) as usize
}

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

/// A name of a domain and the versions of it the store keeps, newest first: at most HISTORY, only the last once it is
/// removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head { name: [u8; NAME_MAX], len: u8, kept: [Version; HISTORY], count: u8, domain: u16 }
impl Head {
    pub const EMPTY: Head = Head { name: [0; NAME_MAX], len: 0, kept: [Version::EMPTY; HISTORY], count: 0, domain: VOLUME };
    fn new(domain: u16, name: &[u8], version: Version) -> Self {
        let mut h = Head { len: name.len() as u8, domain, ..Head::EMPTY };
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

/// A pin: a retention obligation of an owner (a badge) on an object of a domain, until the owner unpins it (MC-4.11,
/// Appendix B.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin { pub id: u32, pub owner: u16, root: [u8; cid::BYTES], lba: u64, pub size: u64, domain: u16 }
impl Pin {
    pub const EMPTY: Pin = Pin { id: 0, owner: 0, root: [0; cid::BYTES], lba: 0, size: 0, domain: VOLUME };
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
    Some(Pin { id, owner: u16::from_le_bytes([sector[16], sector[17]]), root: sector[20..56].try_into().unwrap(), lba: 0, size: 0, domain: VOLUME })
}

fn key_record(domain: u16, state: u16, owner: u16, key: &[u8; 32]) -> [u8; KEY_RECORD] {
    let mut r = [0u8; KEY_RECORD];
    r[..8].copy_from_slice(KEY_MAGIC);
    r[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
    r[10..12].copy_from_slice(&domain.to_le_bytes());
    r[12..14].copy_from_slice(&state.to_le_bytes());
    r[14..16].copy_from_slice(&owner.to_le_bytes());
    r[20..52].copy_from_slice(key);
    let check = sha256::digest(&r[..52]);
    r[52..KEY_RECORD].copy_from_slice(&check);
    r
}

// A domain's number, state, owner and key, if the sector is a whole key record of this layout.
fn parse_key(sector: &[u8]) -> Option<(u16, u16, u16, [u8; 32])> {
    if &sector[..8] != KEY_MAGIC || sector[52..KEY_RECORD] != sha256::digest(&sector[..52]) || sector[KEY_RECORD..].iter().any(|&b| b != 0) { return None; }
    let field = |at: usize| u16::from_le_bytes([sector[at], sector[at + 1]]);
    let (domain, state, owner) = (field(10), field(12), field(14));
    let key: [u8; 32] = sector[20..52].try_into().unwrap();
    let valid = field(8) == LAYOUT && sector[16..20] == [0; 4] && domain != VOLUME && domain != KEYS
        && (state == KEY_LIVE || (state == KEY_GONE && key == [0; 32]));
    valid.then_some((domain, state, owner, key))
}

/// A memory domain (308-STO-0023): its number, its owner (a badge), whether its key is forgotten, and how many of its
/// records the last pass met; its keys and where its key record and tombstone lie.
#[derive(Clone, Copy)]
pub struct Domain { pub id: u16, pub owner: u16, pub forgotten: bool, pub records: u32, keys: Keys, key_at: u64, tomb_at: u64 }

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
    crypto: &'a dyn Crypto,
    keys: Keys,
    // Whether this mount made the volume key, and from random bytes; and where the key is kept.
    made: Option<bool>,
    source: u8,
    // The domain requests work in, and the domains the store keeps.
    domain: u16,
    domains: [Option<Domain>; DOMAINS],
    // Records a mount met before their domain's key record (sector, sectors, domain), and how many; more than fit make
    // the mount scan once more, every key known.
    deferred: [(u64, u64, u16); DEFERRED],
    waiting: usize,
    // Where the scrub goes on from (0: no pass under way), and what this pass found so far.
    scrub_at: u64,
    scrubbing: Scrubbed,
}

// Records of domains not yet met a mount keeps for later.
const DEFERRED: usize = 64;

fn free_marker(sectors: u64) -> [u8; FREE_RECORD] {
    let mut r = [0u8; FREE_RECORD];
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
pub const fn record_sectors(len: usize) -> usize { sealed_sectors(HEADER + len) }

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

/// The superblock of this layout: with the volume key's check, and the key in the clear or (`kept`) only the id it is
/// kept under; or (none) with no key yet.
pub fn superblock(key: Option<(&[u8; 32], &dyn Crypto)>, kept: Option<&[u8; 8]>) -> [u8; SECTOR] {
    let mut s = [0u8; SECTOR];
    s[..8].copy_from_slice(SUPER_MAGIC);
    s[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
    s[10..12].copy_from_slice(&(SECTOR as u16).to_le_bytes());
    s[12] = match (key, kept) { (None, _) => KEY_NONE, (Some(_), None) => KEY_CLEAR, (Some(_), Some(_)) => KEY_KEPT };
    let check = sha256::digest(&s[..16]);
    s[16..48].copy_from_slice(&check);
    if let Some((key, crypto)) = key {
        match kept { Some(id) => s[48..56].copy_from_slice(id), None => s[48..80].copy_from_slice(key) }
        let aad = aad(&s, 0);
        let (nonce, tag) = Keys::new(key).seal(crypto, &aad, &mut s[92..124]);
        s[80..92].copy_from_slice(&nonce);
        s[124..140].copy_from_slice(&tag);
    }
    let check = sha256::digest(&s[..140]);
    s[140..SUPER].copy_from_slice(&check);
    s
}

// The volume key of a superblock of this layout, in it or from its keeper, once it opens the check; none if it has no
// key yet.
fn key_of(s: &[u8], crypto: &dyn Crypto, keeper: &mut Option<&mut dyn Keeper>) -> Result<Option<[u8; 32]>, Error> {
    let key: [u8; 32] = match s[12] {
        KEY_NONE if s[48..140].iter().all(|&b| b == 0) => return Ok(None),
        KEY_CLEAR => s[48..80].try_into().unwrap(),
        KEY_KEPT if s[56..80].iter().all(|&b| b == 0) => {
            let id: [u8; 8] = s[48..56].try_into().unwrap();
            keeper.as_mut().and_then(|k| k.get(&id)).ok_or(Error::Kept(id))?
        }
        _ => return Err(Error::Key),
    };
    let mut check: [u8; 32] = s[92..124].try_into().unwrap();
    let opened = crypto.open(&Keys::new(&key).seal, s[80..92].try_into().unwrap(), &aad(s, 0), &mut check, s[124..140].try_into().unwrap());
    if opened && check == [0; 32] { Ok(Some(key)) } else { Err(Error::Key) }
}

/// The id a superblock's key is kept under by its keeper, if it is kept so.
#[cfg(test)]
pub fn kept_id(s: &[u8]) -> Option<[u8; 8]> { (s[..8] == *SUPER_MAGIC && s[12] == KEY_KEPT).then(|| s[48..56].try_into().unwrap()) }

impl<'a, D: Device> Store<'a, D> {
    /// Mounts the store on `dev` at time `now` (ns), its records sealed with `crypto`: a blank medium is formatted (if
    /// writable), a store is scanned and its blocks opened and verified; anything else is refused. A medium without a
    /// volume key yet gets one, kept by `keeper` when there is one and the key is random, else in the superblock. `index`
    /// bounds how many blocks it can hold, `heads` how many names, `pins` how many pins, `holes` how many runs of blank
    /// sectors it uses. Every block starts a lease at `now`.
    #[allow(clippy::too_many_arguments)]
    pub fn mount(mut dev: D, crypto: &'a dyn Crypto, mut keeper: Option<&mut dyn Keeper>, index: &'a mut [Entry], heads: &'a mut [Head],
                 pins: &'a mut [Pin], holes: &'a mut [Extent], buffer: &'a mut [u8; BUFFER], scratch: &'a mut [u8; dag::CHUNK], now: u64) -> Result<Self, Error> {
        if dev.sectors() < 2 { return Err(Error::Full); }
        // An index entry names a record by a sector below 2^32 (2 TiB of 512-byte sectors).
        if dev.sectors() > 1 << 32 { return Err(Error::TooLarge); }
        if !dev.read(0, &mut buffer[..SECTOR]) { return Err(Error::Device); }
        let s: [u8; SECTOR] = buffer[..SECTOR].try_into().unwrap();
        let key = if s.iter().all(|&b| b == 0) {
            // Only a wholly blank medium is formatted: some file systems leave their first sectors zero.
            if !blank(&mut dev, buffer)? { return Err(Error::Foreign); }
            None
        } else {
            if &s[..8] != SUPER_MAGIC || s[16..48] != sha256::digest(&s[..16]) { return Err(Error::Foreign); }
            let layout = u16::from_le_bytes([s[8], s[9]]);
            if layout != LAYOUT { return Err(Error::Layout(layout)); }
            if s[10..12] != (SECTOR as u16).to_le_bytes() || s[13..16] != [0; 3] || s[140..SUPER] != sha256::digest(&s[..140])
                || s[SUPER..].iter().any(|&b| b != 0) { return Err(Error::Foreign); }
            key_of(&s, crypto, &mut keeper)?
        };
        let (key, made, source) = match key {
            Some(key) => (key, None, s[12]),
            None => {
                if !dev.writable() { return Err(Error::ReadOnly); }
                let mut key = [0u8; 32];
                let random = crypto.random(&mut key);
                // Without a random source the key comes from the medium's size and the time: no weaker while it is kept
                // in the clear (308-STO-0021).
                if !random { key = hmac_sha256(&[0; 32], &[b"MIND-STO volume key", &dev.sectors().to_le_bytes(), &now.to_le_bytes()]); }
                // Only a random key is kept off the medium: a derived one could be found again without its keeper.
                let mut id = [0u8; 8];
                let kept = (random && crypto.random(&mut id) && keeper.as_mut().is_some_and(|k| k.put(&id, &key))).then_some(id);
                if !dev.write(0, &superblock(Some((&key, crypto)), kept.as_ref())) || !dev.flush() { return Err(Error::Device); }
                (key, Some(random), if kept.is_some() { KEY_KEPT } else { KEY_CLEAR })
            }
        };
        let quota = dev.sectors() * SECTOR as u64 / 4 * 3;
        // Every slot starts empty: what a slice held before is not this medium's.
        index.fill(Entry::EMPTY);
        let mut store = Store {
            dev, index, count: 0, heads, names: 0, pins, pinned: 0, next_pin: 1, quota, holes, runs: 0, buffer,
            scratch: Some(scratch), now, end: 1, bytes: 0, corrupt: 0, damaged: 0, crypto, keys: Keys::new(&key), made, source,
            domain: VOLUME, domains: [None; DOMAINS], deferred: [(0, 0, 0); DEFERRED], waiting: 0, scrub_at: 0,
            scrubbing: Scrubbed::default(),
        };
        store.mount_pass()?;
        store.size_retained();
        Ok(store)
    }

    /// Whether this mount made the volume key (it found none), and from random bytes.
    pub fn made(&self) -> Option<bool> { self.made }

    /// Where the volume key is kept: in the superblock in the clear (KEY_CLEAR) or by the keeper (KEY_KEPT).
    pub fn key_source(&self) -> u8 { self.source }

    /// The time (ns, monotonic) the next requests happen at: leases are measured with it.
    pub fn set_time(&mut self, now: u64) { self.now = now; }

    /// Starts every block's lease at `now`: called when a mount is done, since verifying a whole medium can take longer
    /// than a lease, and a restart must not shorten the time a writer has.
    pub fn renew(&mut self, now: u64) {
        self.now = now;
        for e in self.index.iter_mut().filter(|e| e.used()) { e.lease = now; }
    }

    // Reads the medium in windows of RECORD_SECTORS and finds the runs of blank sectors. Mounting (`sweep` false)
    // opens every record, verifies it and builds the index and the names. A sweep frees the block records the index
    // does not retain at that place, the name records that are not current and the records that do not open; nothing
    // else is written.
    fn pass(&mut self, sweep: bool) -> Result<Collected, Error> {
        let sectors = self.dev.sectors();
        let mut freed = Collected::default();
        let (mut lba, mut run) = (1u64, None::<u64>);
        (self.runs, self.end, self.damaged) = (0, 1, 0);
        if sweep { self.corrupt = 0; }
        for d in self.domains.iter_mut().flatten() { d.records = 0; }
        while lba < sectors {
            let window = (sectors - lba).min(RECORD_SECTORS as u64) as usize;
            if !self.dev.read(lba, &mut self.buffer[..window * SECTOR]) { return Err(Error::Device); }
            let mut at = 0;
            let mut next = lba + window as u64;
            while at < window {
                let here = lba + at as u64;
                let sector = &self.buffer[at * SECTOR..(at + 1) * SECTOR];
                if let Some(n) = parse_frame(sector).filter(|&n| here + n as u64 <= sectors) {
                    // The record is read again whole to be opened, so the next window starts after it.
                    let domain = u16::from_le_bytes([sector[10], sector[11]]);
                    next = here + self.record(here, n as u64, domain, sweep, &mut run, &mut freed)?;
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
            // An entry the sweep did not meet and does not retain leaves the index too (its record was not found). A
            // removal shifts later entries back, so the same slot is looked at again.
            let mut i = 0;
            while i < self.index.len() {
                if self.index[i].used() && !self.retained(i) { self.remove(i); } else { i += 1; }
            }
            // A forgotten domain's records are all freed now: its tombstone goes, and its number may be given again.
            for k in 0..DOMAINS {
                let Some(d) = self.domains[k].filter(|d| d.forgotten) else { continue };
                if d.tomb_at != 0 { self.erase(d.tomb_at, 1)?; self.release(d.tomb_at, 1); }
                self.domains[k] = None;
            }
        }
        freed.free = self.holes[..self.runs].iter().map(|h| h.len).sum();
        Ok(freed)
    }

    // The mount's scan. A domain's key record may lie after its records: those met before it wait, and are opened once
    // the scan is done; if more wait than fit, the store is scanned once more, every key known by then. A domain
    // forgotten whose key record is still there (a forget that stopped in the middle) has that record erased.
    fn mount_pass(&mut self) -> Result<(), Error> {
        self.waiting = 0;
        self.pass(false)?;
        if self.waiting > DEFERRED {
            (self.count, self.bytes, self.names, self.pinned, self.next_pin, self.corrupt) = (0, 0, 0, 0, 1, 0);
            self.index.fill(Entry::EMPTY);
            self.waiting = usize::MAX;
            self.pass(false)?;
        } else {
            for k in 0..self.waiting {
                let (lba, n, domain) = self.deferred[k];
                if self.slot_of(domain).is_some() { self.record(lba, n, domain, false, &mut None, &mut Collected::default())?; } else { self.corrupt += 1; }
            }
        }
        self.waiting = 0;
        for k in 0..DOMAINS {
            let Some(d) = self.domains[k].as_mut().filter(|d| d.forgotten && d.key_at != 0) else { continue };
            let key_at = core::mem::take(&mut d.key_at);
            if self.dev.writable() { self.erase(key_at, 1)?; self.release(key_at, 1); }
        }
        Ok(())
    }

    // A key record or tombstone found at `lba`.
    fn key_found(&mut self, lba: u64, (id, state, owner, key): (u16, u16, u16, [u8; 32])) -> Result<(), Error> {
        let k = match self.slot_of(id) {
            Some(k) => k,
            None => {
                let k = self.domains.iter().position(Option::is_none).ok_or(Error::Full)?;
                self.domains[k] = Some(Domain { id, owner, forgotten: false, records: 0, keys: Keys::NONE, key_at: 0, tomb_at: 0 });
                k
            }
        };
        let d = self.domains[k].as_mut().unwrap();
        if state == KEY_GONE {
            let was_live = !d.forgotten;
            (d.forgotten, d.tomb_at, d.keys) = (true, lba, Keys::NONE);
            // What the scan opened of it before the tombstone leaves the tables again.
            if was_live { self.drop_domain(id); }
        } else if d.key_at == 0 {
            (d.key_at, d.owner) = (lba, owner);
            if !d.forgotten { d.keys = Keys::new(&key); }
        }
        Ok(())
    }

    fn slot_of(&self, id: u16) -> Option<usize> { self.domains.iter().position(|d| d.is_some_and(|d| d.id == id)) }

    // The keys the records of `domain` are sealed under: the volume's for its own and for the key records, a live
    // domain's own; none for a forgotten or unknown one.
    fn keys_for(&self, domain: u16) -> Option<Keys> {
        match domain {
            VOLUME | KEYS => Some(self.keys),
            _ => self.slot_of(domain).and_then(|k| self.domains[k]).filter(|d| !d.forgotten).map(|d| d.keys),
        }
    }

    // Opens the sealed record of `n` sectors at `here`: a mount indexes what it holds, a sweep keeps or frees it.
    // Returns the sectors it covers (a free marker covers more than its own).
    fn record(&mut self, here: u64, n: u64, domain: u16, sweep: bool, run: &mut Option<u64>, freed: &mut Collected) -> Result<u64, Error> {
        if domain == KEYS {
            // A mount learns the domains from their key records; a sweep keeps them, and drops a forgotten domain's
            // tombstone after it.
            if !sweep {
                match self.open(here, n as usize, KEYS)? {
                    true if n == 1 => match parse_key(&self.buffer[..SECTOR]) { Some(key) => self.key_found(here, key)?, None => self.corrupt += 1 },
                    _ => self.corrupt += 1,
                }
            }
            return self.settle(true, here, n, run, freed);
        }
        if domain != VOLUME {
            let Some(k) = self.slot_of(domain) else {
                // Its key record may lie further on: the mount opens it then (mount_pass).
                if !sweep && self.waiting != usize::MAX {
                    if self.waiting < DEFERRED { self.deferred[self.waiting] = (here, n, domain); }
                    self.waiting += 1;
                    return self.settle(true, here, n, run, freed);
                }
                // No key and no tombstone: its key record may only be damaged, so the record is kept, counted corrupt.
                self.corrupt += 1;
                return self.settle(true, here, n, run, freed);
            };
            let d = self.domains[k].as_mut().unwrap();
            d.records += 1;
            if d.forgotten {
                // Its key is destroyed, so it never opens again: a sweep frees it.
                if sweep { freed.blocks += 1; }
                return self.settle(!sweep, here, n, run, freed);
            }
        }
        // What does not open is read as zeros, which no record is.
        let opened = self.open(here, n as usize, domain)?;
        let sector = &self.buffer[..SECTOR];
        let one = opened && n == 1;
        if let Some((cid, size)) = parse_header(sector).filter(|&(_, size)| opened && record_sectors(size) as u64 == n) {
            let keep = if sweep {
                match self.position(&cid.to_bytes(), domain) {
                    Ok(i) if self.index[i].lba as u64 == here && self.retained(i) => true,
                    // The indexed copy leaves the index before its sectors are erased, so a failure later in the sweep
                    // never leaves an erased block acknowledged (audit A05, 175-STO-0012).
                    Ok(i) if self.index[i].lba as u64 == here => { self.remove(i); false }
                    _ => false,
                }
            } else {
                let whole = self.intact(&cid, size);
                if !whole { self.corrupt += 1; }
                if whole && self.position(&cid.to_bytes(), domain).is_err() {
                    if self.count == self.limit() { return Err(Error::Full); }
                    self.insert(cid.to_bytes(), domain, here, size);
                }
                true
            };
            if !keep { freed.blocks += 1; }
            return self.settle(keep, here, n, run, freed);
        }
        if let Some(count) = parse_txn(sector).filter(|&c| opened && sealed_sectors((1 + c) * SECTOR) as u64 == n) {
            // A commit applies whole or not at all: its seal and its digest cover every entry (MC-4.10).
            if let Some(entries) = self.commit_entries(count) {
                let entries = &entries[..count];
                let keep = if sweep {
                    entries.iter().any(|(name, len, v)| self.find_in(domain, &name[..*len]).is_ok_and(|i| self.heads[i].versions().iter().any(|k| k.version == v.version)))
                } else {
                    for (name, len, v) in entries { self.keep_version(domain, &name[..*len], *v)?; }
                    true
                };
                if !keep { freed.names += count as u32; }
                return self.settle(keep, here, n, run, freed);
            }
        }
        if let Some((name, size, v)) = parse_name(sector).filter(|_| one) {
            let name = &name[..size];
            // A sweep frees a version the name no longer keeps.
            let keep = if sweep { self.find_in(domain, name).is_ok_and(|i| self.heads[i].versions().iter().any(|k| k.version == v.version)) } else { self.keep_version(domain, name, v)?; true };
            if !keep { freed.names += 1; }
            return self.settle(keep, here, n, run, freed);
        }
        if let Some(mut pin) = parse_pin(sector).filter(|_| one) {
            pin.domain = domain;
            let held = self.pins[..self.pinned].iter().position(|p| p.id == pin.id);
            let keep = if sweep { held.is_some_and(|k| self.pins[k].lba == here) } else {
                if held.is_none() {
                    if self.pinned == self.pins.len() { return Err(Error::Full); }
                    pin.lba = here;
                    self.pins[self.pinned] = pin;
                    self.pinned += 1;
                    self.next_pin = self.next_pin.max(pin.id.wrapping_add(1));
                }
                true
            };
            return self.settle(keep, here, n, run, freed);
        }
        if let Some(m) = parse_free(sector).filter(|&m| one && domain == VOLUME && here + m <= self.dev.sectors()) {
            // A collection stopped in the middle of freeing this: finish it.
            if self.dev.writable() { self.erase(here, m)?; run.get_or_insert(here); } else { self.occupied(run, here, m); }
            return Ok(m);
        }
        // A frame whose seal does not open, or whose record is none of this layout: corrupt. A sweep frees it unless the
        // index names its place, so a get still reports that block corrupt.
        if !sweep || self.index.iter().any(|e| e.used() && e.lba as u64 == here) {
            self.corrupt += 1;
            return self.settle(true, here, n, run, freed);
        }
        freed.blocks += 1;
        self.settle(false, here, n, run, freed)
    }

    // Keeps the `n` sectors of a record at `here` occupied, or frees them.
    fn settle(&mut self, keep: bool, here: u64, n: u64, run: &mut Option<u64>, freed: &mut Collected) -> Result<u64, Error> {
        if keep { self.occupied(run, here, n); } else { self.erase(here, n)?; run.get_or_insert(here); freed.sectors += n; }
        Ok(n)
    }

    // A version found on the medium: the name keeps it among its newest.
    fn keep_version(&mut self, domain: u16, name: &[u8], v: Version) -> Result<(), Error> {
        match self.find_in(domain, name) {
            Ok(i) => self.heads[i].keep(v),
            Err(i) => { if self.names == self.heads.len() { return Err(Error::Full); } self.add(i, Head::new(domain, name, v)); }
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
        self.write_record(lba, &free_marker(n), VOLUME)?;
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

    // Seals the record of `len` bytes at buffer[FRAME..], and the zeros after it to the end of its last sector, in
    // `domain` for its first sector `lba`; returns its sectors, ready to write. `NotFound` for a domain without a key.
    fn seal(&mut self, lba: u64, len: usize, domain: u16) -> Result<usize, Error> {
        let keys = self.keys_for(domain).ok_or(Error::NotFound)?;
        let n = sealed_sectors(len);
        let b = &mut self.buffer[..n * SECTOR];
        b[FRAME + len..].fill(0);
        b[..8].copy_from_slice(FRAME_MAGIC);
        b[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
        b[10..12].copy_from_slice(&domain.to_le_bytes());
        b[12..16].copy_from_slice(&(n as u32).to_le_bytes());
        let aad = aad(b, lba);
        let (nonce, tag) = keys.seal(self.crypto, &aad, &mut b[FRAME..]);
        b[16..28].copy_from_slice(&nonce);
        b[28..FRAME].copy_from_slice(&tag);
        Ok(n)
    }

    // Seals the record of `len` bytes at buffer[FRAME..] in `domain` for `lba`, writes and flushes it.
    fn write_sealed(&mut self, lba: u64, len: usize, domain: u16) -> Result<(), Error> {
        let n = self.seal(lba, len, domain)?;
        // A scrub inside the blank run written now would read the record's middle as damaged: it goes on after it.
        if (lba + 1..lba + n as u64).contains(&self.scrub_at) { self.scrub_at = lba + n as u64; }
        if !self.dev.write(lba, &self.buffer[..n * SECTOR]) || !self.dev.flush() { return Err(Error::Device); }
        Ok(())
    }

    fn write_record(&mut self, lba: u64, record: &[u8], domain: u16) -> Result<(), Error> {
        self.buffer[FRAME..FRAME + record.len()].copy_from_slice(record);
        self.write_sealed(lba, record.len(), domain)
    }

    // Reads the `n` sectors at `lba` and opens the record they seal in `domain`: it then starts the buffer, zeros after
    // it. If the frame does not cover exactly them, is of another domain or does not open, the buffer is zeros and false
    // returned.
    fn open(&mut self, lba: u64, n: usize, domain: u16) -> Result<bool, Error> {
        let keys = self.keys_for(domain);
        let b = &mut self.buffer[..n * SECTOR];
        if !self.dev.read(lba, b) { return Err(Error::Device); }
        let framed = parse_frame(b) == Some(n) && b[10..12] == domain.to_le_bytes();
        let (aad, nonce, tag) = (aad(b, lba), b[16..28].try_into().unwrap(), b[28..FRAME].try_into().unwrap());
        let opened = framed && keys.is_some_and(|k| self.crypto.open(&k.seal, &nonce, &aad, &mut b[FRAME..], &tag));
        if !opened { b.fill(0); return Ok(false); }
        b.copy_within(FRAME.., 0);
        b[n * SECTOR - FRAME..].fill(0);
        Ok(true)
    }

    // Opens the record at `lba`; whether it is the block `cid` of `len` bytes, whole. Its bytes then follow the header.
    fn verify(&mut self, lba: u64, cid: &Cid, len: usize) -> Result<bool, Error> {
        let whole = self.open(lba, record_sectors(len), self.domain)? && self.intact(cid, len);
        if !whole { self.corrupt += 1; }
        Ok(whole)
    }

    // Whether the opened record in the buffer is the block `cid` of `len` bytes: its header names it, its bytes match.
    fn intact(&self, cid: &Cid, len: usize) -> bool {
        let data = &self.buffer[HEADER..HEADER + len];
        parse_header(&self.buffer[..SECTOR]) == Some((*cid, len)) && cid.matches(data) && typed(cid.codec(), data)
    }

    // The slot holding `key` of `domain`, or the empty slot where it would go (the number of slots when none is empty).
    fn position(&self, key: &[u8; cid::BYTES], domain: u16) -> Result<usize, usize> {
        let n = self.index.len();
        if n == 0 { return Err(0); }
        let mut at = home(key, domain, n);
        for _ in 0..n {
            let e = &self.index[at];
            if !e.used() { return Err(at); }
            if e.key == *key && e.domain == domain { return Ok(at); }
            at = if at + 1 == n { 0 } else { at + 1 };
        }
        Err(n)
    }
    // The most blocks the index takes: every slot of a small one, seven eighths of a large one, so a search for a block
    // that is not there ends soon.
    fn limit(&self) -> usize { let n = self.index.len(); if n > 64 { n - n / 8 } else { n } }
    fn contains(&self, cid: &Cid) -> bool { self.position(&cid.to_bytes(), self.domain).is_ok() }
    // Whether a collection keeps the block: a name retains it, or its lease runs.
    fn retained(&self, i: usize) -> bool { self.index[i].live() || self.now < self.index[i].lease.saturating_add(LEASE_NS) }

    // Callers check first that the key is not there and that the index has room (`limit`).
    fn insert(&mut self, key: [u8; cid::BYTES], domain: u16, lba: u64, len: usize) {
        let at = self.position(&key, domain).unwrap_err();
        self.index[at] = Entry { lease: self.now, lba: lba as u32, len: len as u16, flags: USED, key, domain };
        self.count += 1;
        self.bytes += len as u64;
    }

    // Empties a slot and moves back the entries after it that would no longer be found (deletion without tombstones).
    fn remove(&mut self, at: usize) {
        self.bytes -= self.index[at].len as u64;
        self.count -= 1;
        let n = self.index.len();
        let (mut hole, mut next) = (at, (at + 1) % n);
        while next != at && self.index[next].used() {
            let home = home(&self.index[next].key, self.index[next].domain, n);
            // The entry stays if its home lies cyclically after the hole, up to where it is.
            let stays = if hole < next { hole < home && home <= next } else { hole < home || home <= next };
            if !stays { self.index[hole] = self.index[next]; hole = next; }
            next = (next + 1) % n;
        }
        self.index[hole] = Entry::EMPTY;
    }

    pub fn has(&self, cid: &Cid) -> bool { self.contains(cid) }

    // A name of the domain requests work in, as find_in.
    fn find(&self, name: &[u8]) -> Result<usize, usize> { self.find_in(self.domain, name) }

    // The head of `name` in `domain`, or where it would go: heads are sorted by domain, then name.
    fn find_in(&self, domain: u16, name: &[u8]) -> Result<usize, usize> {
        self.heads[..self.names].binary_search_by(|h| (h.domain, h.name()).cmp(&(domain, name)))
    }

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

    /// The current names that start with `prefix` and sort after `after` (empty: from the first), in byte order, with
    /// their versions and roots; a removed name is skipped (300-STO-0018).
    pub fn names<'s>(&'s self, prefix: &[u8], after: &[u8]) -> Result<impl Iterator<Item = (&'s [u8], u64, Cid)> + 's, Error> {
        let part = |s: &[u8]| s.len() <= NAME_MAX && (s.is_empty() || valid_name(s));
        if !part(prefix) || !part(after) { return Err(Error::Invalid); }
        // A domain's names are one run of the heads, sorted by name.
        let all = &self.heads[..self.names];
        let heads = &all[all.partition_point(|h| h.domain < self.domain)..all.partition_point(|h| h.domain <= self.domain)];
        // Names are kept sorted, so those with the prefix are one run.
        let start = heads.partition_point(|h| h.name() < prefix).max(if after.is_empty() { 0 } else { heads.partition_point(|h| h.name() <= after) });
        let prefix_len = prefix.len();
        let mut first = [0u8; NAME_MAX];
        first[..prefix_len].copy_from_slice(prefix);
        Ok(heads[start..].iter().take_while(move |h| h.name().starts_with(&first[..prefix_len]))
            .filter_map(|h| h.latest().root().map(|root| (h.name(), h.latest().version, root))))
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

    // The size of every retained object, for the owners' accounts: read from each root, in its domain, once a mount is
    // done.
    fn size_retained(&mut self) {
        let Some(scratch) = self.scratch.take() else { return };
        let current = self.domain;
        for i in 0..self.names {
            self.domain = self.heads[i].domain;
            for k in 0..self.heads[i].count as usize {
                if let Some(root) = self.heads[i].kept[k].root() { self.heads[i].kept[k].size = dag::size(self, &root, scratch).unwrap_or(0); }
            }
        }
        for i in 0..self.pinned {
            self.domain = self.pins[i].domain;
            let root = self.pins[i].root();
            self.pins[i].size = dag::size(self, &root, scratch).unwrap_or(0);
        }
        self.domain = current;
        self.scratch = Some(scratch);
    }

    // The roots `owner` retains through its names' kept versions and its pins, each with its domain, with their
    // objects' sizes.
    fn owned(&self, owner: u16) -> impl Iterator<Item = (Root, u64)> + '_ {
        self.heads[..self.names].iter().flat_map(|h| { let d = h.domain; h.retains().map(move |v| (d, v)) })
            .filter(move |(_, v)| v.owner == owner).map(|(d, v)| ((v.root, d), v.size))
            .chain(self.pins[..self.pinned].iter().filter(move |p| p.owner == owner).map(|p| ((p.root, p.domain), p.size)))
    }

    // The bytes `owner` retains, each root counted once however many names and pins retain it; with the roots of
    // `extra` added unless it holds them already.
    fn retained_by(&self, owner: u16, extra: &[(Root, u64)]) -> u64 {
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
        if self.retained_by(owner, &[((root.to_bytes(), self.domain), size)]) > self.quota { return Err(Error::Quota); }
        let previous = self.find(name).ok().and_then(|i| self.heads[i].latest().root());
        let v = Version { version: current + 1, owner, root: root.to_bytes(), points: true, size };
        self.claim(lba, 1);
        self.write_record(lba, &name_record(name, &v, previous)[..NAME_RECORD], self.domain)?;
        match self.find(name) { Ok(i) => self.heads[i].keep(v), Err(i) => self.add(i, Head::new(self.domain, name, v)) }
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
        self.write_record(lba, &name_record(name, &v, previous)[..NAME_RECORD], self.domain)?;
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
        let size = (1 + n) * SECTOR;
        let lba = self.room(sealed_sectors(size) as u64)?;
        let mut extra = [(([0u8; cid::BYTES], self.domain), 0u64); COMMIT_MAX];
        for (k, u) in updates.iter().enumerate() {
            if let Some(root) = u.root {
                let size = self.walk(&root, |s, cid| if s.contains(cid) { Ok(()) } else { Err(dag::Error::NotFound) })?;
                extra[k] = ((root.to_bytes(), self.domain), size);
            }
        }
        // A removal adds a zero root of no size: it changes nothing.
        if self.retained_by(owner, &extra[..n]) > self.quota { return Err(Error::Quota); }
        let mut versions = [0u64; COMMIT_MAX];
        let domain = self.domain;
        let record = &mut self.buffer[FRAME..FRAME + size];
        record.fill(0);
        record[..8].copy_from_slice(TXN_MAGIC);
        record[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
        record[10..12].copy_from_slice(&(n as u16).to_le_bytes());
        for (k, u) in updates.iter().enumerate() {
            let head = self.heads[..self.names].binary_search_by(|h| (h.domain, h.name()).cmp(&(domain, u.name))).ok().map(|i| &self.heads[i]);
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
        self.claim(lba, sealed_sectors(size) as u64);
        self.write_sealed(lba, size, domain)?;
        for (k, u) in updates.iter().enumerate() {
            let v = Version { version: versions[k], owner, root: u.root.map_or([0; cid::BYTES], |r| r.to_bytes()), points: u.root.is_some(), size: extra[k].1 };
            self.keep_version(domain, u.name, v)?;
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
        if self.retained_by(owner, &[((root.to_bytes(), self.domain), size)]) > self.quota { return Err(Error::Quota); }
        let pin = Pin { id: self.next_pin, owner, root: root.to_bytes(), lba, size, domain: self.domain };
        self.claim(lba, 1);
        self.write_record(lba, &pin_record(&pin)[..PIN_RECORD], self.domain)?;
        self.next_pin = self.next_pin.wrapping_add(1).max(1);
        self.pins[self.pinned] = pin;
        self.pinned += 1;
        Ok(pin.id)
    }

    /// Ends `owner`'s pin `id`: its record is freed at once; the object goes when nothing else retains it.
    pub fn unpin(&mut self, id: u32, owner: u16) -> Result<(), Error> {
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let k = self.pins[..self.pinned].iter().position(|p| p.id == id && p.domain == self.domain).ok_or(Error::NotFound)?;
        if self.pins[k].owner != owner { return Err(Error::Rights); }
        let lba = self.pins[k].lba;
        self.erase(lba, 1)?;
        self.pins.copy_within(k + 1..self.pinned, k);
        self.pinned -= 1;
        self.release(lba, 1);
        Ok(())
    }

    /// The pins of `owner` in the domain requests work in.
    pub fn pins(&self, owner: u16) -> impl Iterator<Item = &Pin> { self.pins[..self.pinned].iter().filter(move |p| p.owner == owner && p.domain == self.domain) }

    /// Frees what no name retains, no pin holds and no lease protects: unretained blocks, other copies of a block
    /// (corrupt ones too), name versions a name no longer keeps (MC-4.5). Every retained object is walked first, every
    /// node read and checked; if one lacks a block or holds a corrupt node, nothing is freed.
    pub fn collect(&mut self) -> Result<Collected, Error> {
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        for e in self.index.iter_mut() { e.flags &= !LIVE; }
        let current = self.domain;
        let marked = self.mark();
        self.domain = current;
        marked?;
        self.pass(true)
    }

    // Marks every block a name's kept version or a pin retains, each object walked in its own domain.
    fn mark(&mut self) -> Result<(), Error> {
        let mark = |s: &mut Self, cid: &Cid| match s.position(&cid.to_bytes(), s.domain) {
            Ok(k) => { s.index[k].flags |= LIVE; Ok(()) }
            Err(_) => Err(dag::Error::NotFound),
        };
        for i in 0..self.names {
            self.domain = self.heads[i].domain;
            for k in 0..self.heads[i].count as usize {
                let head = &self.heads[i];
                if head.removed() { break; }
                if let Some(root) = head.kept[k].root() { self.walk(&root, mark)?; }
            }
        }
        for i in 0..self.pinned {
            self.domain = self.pins[i].domain;
            let root = self.pins[i].root();
            self.walk(&root, mark)?;
        }
        Ok(())
    }

    /// Stores `data` as a block of type `codec` and returns its CID, once the device has flushed it. A node must decode
    /// first; a block already held is not written again, and its lease starts again. Without room a collection runs.
    pub fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, Error> {
        if data.len() > BLOCK_MAX { return Err(Error::TooLarge); }
        if !typed(codec, data) { return Err(Error::Invalid); }
        let cid = Cid::of(codec, data);
        if let Ok(i) = self.position(&cid.to_bytes(), self.domain) { self.index[i].lease = self.now; return Ok(cid); }
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let n = record_sectors(data.len()) as u64;
        if self.count == self.limit() { self.collect()?; }
        if self.count == self.limit() { return Err(Error::Full); }
        let lba = self.room(n)?;
        let record = &mut self.buffer[FRAME..FRAME + HEADER + data.len()];
        record[..HEADER].copy_from_slice(&header(&cid, data.len()));
        record[HEADER..].copy_from_slice(data);
        // The sectors are taken even if the write fails: they may hold part of it now, and are not written again.
        self.claim(lba, n);
        self.write_sealed(lba, HEADER + data.len(), self.domain)?;
        self.insert(cid.to_bytes(), self.domain, lba, data.len());
        Ok(cid)
    }

    /// Copies the block named by `cid` into `out` and returns its length, only if its bytes match the CID. A corrupt
    /// block leaves the index, so a put of the same bytes stores them again.
    pub fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> {
        let at = self.position(&cid.to_bytes(), self.domain).map_err(|_| Error::NotFound)?;
        let Entry { lba, len, .. } = self.index[at];
        let (lba, len) = (lba as u64, len as usize);
        if out.len() < len { return Err(Error::TooLarge); }
        if !self.verify(lba, cid, len)? { self.remove(at); return Err(Error::Corrupt); }
        out[..len].copy_from_slice(&self.buffer[HEADER..HEADER + len]);
        Ok(len)
    }

    pub fn stats(&self) -> Stats {
        Stats {
            blocks: self.count as u32, bytes: self.bytes, used: self.end, sectors: self.dev.sectors(),
            corrupt: self.corrupt, damaged: self.damaged, capacity: self.limit() as u32, names: self.names as u32,
            free: self.holes[..self.runs].iter().map(|h| h.len).sum(), pins: self.pinned as u32,
        }
    }

    /// Makes a domain for `owner` (a badge), its key random and kept in a record sealed under the volume key; returns
    /// its number. Without a random source none is made: a key derived from anything kept could not be destroyed.
    pub fn domain_create(&mut self, owner: u16) -> Result<u16, Error> {
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let slot = self.domains.iter().position(Option::is_none).ok_or(Error::Full)?;
        let id = self.domains().map(|d| d.id).max().unwrap_or(VOLUME) + 1;
        if id == KEYS { return Err(Error::Full); }
        let mut key = [0u8; 32];
        if !self.crypto.random(&mut key) { return Err(Error::NoRandom); }
        let lba = self.room(1)?;
        self.claim(lba, 1);
        let written = self.write_record(lba, &key_record(id, KEY_LIVE, owner, &key), KEYS);
        let keys = Keys::new(&key);
        key.fill(0);
        written?;
        self.domains[slot] = Some(Domain { id, owner, forgotten: false, records: 0, keys, key_at: lba, tomb_at: 0 });
        Ok(id)
    }

    /// Forgets `owner`'s domain `id` (MC-4.8): a tombstone first, then its key record erased and its key wiped. Its
    /// names, pins and blocks leave the store at once; its records never open again, and the next collection frees
    /// them. Copies a client already read are outside this.
    pub fn domain_forget(&mut self, id: u16, owner: u16) -> Result<(), Error> {
        if !self.dev.writable() { return Err(Error::ReadOnly); }
        let k = self.slot_of(id).filter(|&k| self.domains[k].is_some_and(|d| !d.forgotten)).ok_or(Error::NotFound)?;
        if self.domains[k].is_some_and(|d| d.owner != owner) { return Err(Error::Rights); }
        let lba = self.room(1)?;
        self.claim(lba, 1);
        self.write_record(lba, &key_record(id, KEY_GONE, owner, &[0; 32]), KEYS)?;
        // Forgotten from here whatever follows: a mount that finds the tombstone erases the key record too.
        let d = self.domains[k].as_mut().unwrap();
        (d.forgotten, d.tomb_at, d.keys) = (true, lba, Keys::NONE);
        let key_at = core::mem::take(&mut d.key_at);
        self.drop_domain(id);
        if self.domain == id { self.domain = VOLUME; }
        if key_at != 0 { self.erase(key_at, 1)?; self.release(key_at, 1); }
        Ok(())
    }

    // Takes a forgotten domain's names, pins and blocks out of the store's tables.
    fn drop_domain(&mut self, id: u16) {
        let mut k = 0;
        while k < self.names { if self.heads[k].domain == id { self.heads.copy_within(k + 1..self.names, k); self.names -= 1; } else { k += 1; } }
        let mut k = 0;
        while k < self.pinned { if self.pins[k].domain == id { self.pins.copy_within(k + 1..self.pinned, k); self.pinned -= 1; } else { k += 1; } }
        // A removal shifts later entries back, so the same slot is looked at again.
        let mut i = 0;
        while i < self.index.len() { if self.index[i].used() && self.index[i].domain == id { self.remove(i); } else { i += 1; } }
    }

    /// Works in domain `id` for the requests that follow (VOLUME: the volume's own); `NotFound` for one not live.
    pub fn set_domain(&mut self, id: u16) -> Result<(), Error> {
        if id == KEYS || self.keys_for(id).is_none() { return Err(Error::NotFound); }
        self.domain = id;
        Ok(())
    }

    /// The domains the store keeps, live and forgotten (a forgotten one until a collection has freed its records).
    pub fn domains(&self) -> impl Iterator<Item = &Domain> { self.domains.iter().flatten() }

    /// Reads and checks the records from where the scrub stopped (309-STO-0025), about `budget` sectors of them, as a
    /// mount does: each opened under its domain's key, a block's bytes matched to its CID, any other record's digest
    /// checked; a forgotten domain's records are not opened. Nothing is written. `found` hears of each record that does
    /// not open or is not whole: its sector, and the block the index holds there. The pass that reaches the medium's
    /// end returns what it found, and the next call starts another.
    pub fn scrub(&mut self, budget: u64, found: &mut dyn FnMut(u64, Option<Cid>)) -> Result<Option<Scrubbed>, Error> {
        let sectors = self.dev.sectors();
        if self.scrub_at == 0 { (self.scrub_at, self.scrubbing) = (1, Scrubbed::default()); }
        let mut read = 0;
        while read < budget && self.scrub_at < sectors {
            let lba = self.scrub_at;
            let window = (sectors - lba).min(RECORD_SECTORS as u64) as usize;
            if !self.dev.read(lba, &mut self.buffer[..window * SECTOR]) { return Err(Error::Device); }
            let mut next = lba + window as u64;
            for at in 0..window {
                let here = lba + at as u64;
                let sector = &self.buffer[at * SECTOR..(at + 1) * SECTOR];
                if let Some(n) = parse_frame(sector).filter(|&n| here + n as u64 <= sectors) {
                    let domain = u16::from_le_bytes([sector[10], sector[11]]);
                    let (whole, covers) = self.check(here, n as u64, domain)?;
                    self.scrubbing.records += 1;
                    if !whole {
                        self.scrubbing.corrupt += 1;
                        let block = self.index.iter().find(|e| e.used() && e.lba as u64 == here).and_then(|e| Cid::from_bytes(&e.key).ok());
                        found(here, block);
                    }
                    next = here + covers;
                    break;
                }
                if sector.iter().any(|&b| b != 0) { self.scrubbing.damaged += 1; }
            }
            read += next - lba;
            self.scrub_at = next;
        }
        if self.scrub_at < sectors { return Ok(None); }
        self.scrub_at = 0;
        Ok(Some(self.scrubbing))
    }

    /// Whether a scrub's pass is under way.
    pub fn scrubbing(&self) -> bool { self.scrub_at != 0 }

    /// Drops the pass under way: the next scrub starts at the medium's first record.
    pub fn restart_scrub(&mut self) { self.scrub_at = 0; }

    // Whether the record of `n` sectors at `here`, sealed in `domain`, opens and is whole; and the sectors it covers.
    fn check(&mut self, here: u64, n: u64, domain: u16) -> Result<(bool, u64), Error> {
        if domain != VOLUME && domain != KEYS {
            match self.slot_of(domain).and_then(|k| self.domains[k]) {
                // Its key is destroyed: the next collection frees it.
                Some(d) if d.forgotten => return Ok((true, n)),
                Some(_) => {}
                // No key and no tombstone, as a mount counts it.
                None => return Ok((false, n)),
            }
        }
        if !self.open(here, n as usize, domain)? { return Ok((false, n)); }
        let sector = &self.buffer[..SECTOR];
        let one = n == 1;
        if domain == KEYS { return Ok((one && parse_key(sector).is_some(), n)); }
        if let Some((cid, size)) = parse_header(sector) { return Ok((record_sectors(size) as u64 == n && self.intact(&cid, size), n)); }
        if let Some(count) = parse_txn(sector) { return Ok((sealed_sectors((1 + count) * SECTOR) as u64 == n && self.commit_entries(count).is_some(), n)); }
        // A collection stopped in the middle of freeing this; the next mount finishes it.
        if let Some(m) = parse_free(sector).filter(|&m| one && domain == VOLUME && here + m <= self.dev.sectors()) { return Ok((true, m)); }
        Ok((one && (parse_name(sector).is_some() || parse_pin(sector).is_some()), n))
    }

    #[cfg(test)]
    pub fn device(&mut self) -> &mut D { &mut self.dev }
}

// A root as an owner's account counts it: the CID and the domain it is in.
type Root = ([u8; cid::BYTES], u16);

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
