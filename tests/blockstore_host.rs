//! Host tests of the block store's layout and logic (blockstore/src/store.rs, issue 300-STO-0002; MC-4.2, 4.8):
//! put and get by CID, blocks found again after a remount, a flipped byte detected and never returned, no sector
//! written twice, defined refusals for a full medium, a full index, a foreign or unknown medium and a read-only one;
//! the clients' rights by badge (libmind/src/blockstore.rs, issue 300-STO-0004); nodes stored only if they decode
//! (issue 301-STO-0002); names published by compare-and-swap once their root's object is complete (302-STO-0001);
//! collection by reachability with leases (303-STO-0001); pins with an owner and a quota, a name's history with explicit
//! links, removing a name (303-STO-0002..0004); the current names listed by prefix, page by page (300-STO-0018); sealed
//! records: nothing of a block, a CID or a name in the clear, a changed or moved record refused, a wrong key refused at
//! mount (308-STO-0021); memory domains, each under its own key, and a domain forgotten by destroying its key, its
//! records then refused and collected (308-STO-0023).
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[path = "../libmind/src/cid.rs"]
mod cid;
#[path = "../libmind/src/dag.rs"]
mod dag;
#[path = "../blockstore/src/store.rs"]
mod store;
#[path = "../libmind/src/blockstore.rs"]
mod rights;

use cid::{Cid, Codec};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use store::{hmac_sha256, record_sectors, sealed_sectors, superblock, Collected, Crypto, Device, Entry, Error, Extent, Head, Pin, Scrubbed, Stats, Store, Update, Usage, BLOCK_MAX, BUFFER, COMMIT_MAX, FRAME, HEADER, HISTORY, KEY_CLEAR, KEY_NONE, LAYOUT, LEASE_NS, NAME_MAX, SECTOR, VOLUME};

/// A stand-in for ChaCha20-Poly1305, since the host tests build without crates: a keystream of splitmix64 seeded by
/// SHA-256(key, nonce), and a tag of two sums in the manner of FNV-1a, seeded by the key, over the nonce, the associated
/// data and the ciphertext, which any one changed byte changes. Not a cipher: enough to show what is sealed and what is
/// refused. The platform's cipher is the RustCrypto crate, tested by its own project.
struct Sealer { random: bool }
static SEALER: Sealer = Sealer { random: true };
static NO_RANDOM: Sealer = Sealer { random: false };
static RANDOM: AtomicU64 = AtomicU64::new(1);
fn keystream(key: &[u8; 32], nonce: &[u8; 12], data: &mut [u8]) {
    let mut h = sha256::Sha256::new();
    h.update(key);
    h.update(nonce);
    let mut x = u64::from_le_bytes(h.finish()[..8].try_into().unwrap());
    for chunk in data.chunks_mut(8) {
        x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        for (b, k) in chunk.iter_mut().zip((z ^ (z >> 31)).to_le_bytes()) { *b ^= k; }
    }
}
fn tag(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &[u8]) -> [u8; 16] {
    let (mut a, mut b) = (u64::from_le_bytes(key[..8].try_into().unwrap()), u64::from_le_bytes(key[8..16].try_into().unwrap()));
    for &byte in nonce.iter().chain(aad).chain(data) {
        a = (a ^ byte as u64).wrapping_mul(0x100_0000_01b3);
        b = (b.rotate_left(7) ^ byte as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    let mut t = [0u8; 16];
    t[..8].copy_from_slice(&a.to_le_bytes());
    t[8..].copy_from_slice(&b.to_le_bytes());
    t
}
impl Crypto for Sealer {
    fn seal(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] {
        keystream(key, nonce, data);
        tag(key, nonce, aad, data)
    }
    fn open(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], t: &[u8; 16]) -> bool {
        if tag(key, nonce, aad, data) != *t { return false; }
        keystream(key, nonce, data);
        true
    }
    fn random(&self, out: &mut [u8]) -> bool {
        if !self.random { return false; }
        for chunk in out.chunks_mut(32) { chunk.copy_from_slice(&sha256::digest(&RANDOM.fetch_add(1, Ordering::Relaxed).to_le_bytes())[..chunk.len()]); }
        true
    }
}

// The format again, written here from docs/storage rather than taken from store.rs: the keys a superblock's volume key
// gives, and a record's associated data.
fn keys(superblock: &[u8]) -> ([u8; 32], [u8; 32]) {
    let volume: [u8; 32] = superblock[48..80].try_into().unwrap();
    (hmac_sha256(&volume, &[b"MIND-STO seal"]), hmac_sha256(&volume, &[b"MIND-STO nonce"]))
}
fn aad(frame: &[u8], lba: u64) -> Vec<u8> { [&frame[..16], &lba.to_le_bytes()[..]].concat() }

/// The record sealed in `bytes` (from its frame on) for sector `lba`, opened under the key in `superblock`: the bytes of
/// its sectors after the frame, or none if no frame starts there or it does not open.
fn unseal(superblock: &[u8], lba: u64, bytes: &[u8]) -> Option<Vec<u8>> {
    if &bytes[..8] != b"MIND-SLD" || bytes[8..10] != LAYOUT.to_le_bytes() { return None; }
    let n = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let mut data = bytes.get(FRAME..n * SECTOR)?.to_vec();
    SEALER.open(&keys(superblock).0, bytes[16..28].try_into().unwrap(), &aad(bytes, lba), &mut data, bytes[28..44].try_into().unwrap()).then_some(data)
}

/// Every record on the medium that opens, by its first sector, and every frame that does not (none).
fn records(medium: &Memory) -> Vec<(u64, Option<Vec<u8>>)> {
    let mut found = Vec::new();
    let mut lba = 1;
    while lba < medium.writes.len() {
        let at = lba * SECTOR;
        let n = u32::from_le_bytes(medium.data[at + 12..at + 16].try_into().unwrap()) as usize;
        if medium.data[at..at + 8] == *b"MIND-SLD" && n >= 1 && lba + n <= medium.writes.len() {
            found.push((lba as u64, unseal(&medium.data[..SECTOR], lba as u64, &medium.data[at..])));
            lba += n;
        } else {
            lba += 1;
        }
    }
    found
}

/// The first sector of each record that opens and starts with `magic`.
fn find(medium: &Memory, magic: &[u8; 8]) -> Vec<u64> {
    records(medium).into_iter().filter(|(_, r)| r.as_ref().is_some_and(|r| r.starts_with(magic))).map(|(lba, _)| lba).collect()
}

/// Seals `record` at `lba` as the store does, under the medium's key: how a test forges a record.
fn plant(medium: &mut Memory, lba: u64, record: &[u8]) { plant_in(medium, lba, record, VOLUME) }

/// The same with the frame's domain `domain`; still under the volume key, as the domains' key records are (0xFFFF).
fn plant_in(medium: &mut Memory, lba: u64, record: &[u8], domain: u16) {
    let n = sealed_sectors(record.len());
    let mut b = vec![0u8; n * SECTOR];
    b[..8].copy_from_slice(b"MIND-SLD");
    b[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
    b[10..12].copy_from_slice(&domain.to_le_bytes());
    b[12..16].copy_from_slice(&(n as u32).to_le_bytes());
    b[FRAME..FRAME + record.len()].copy_from_slice(record);
    let nonce = [lba as u8; 12];
    let t = SEALER.seal(&keys(&medium.data[..SECTOR]).0, &nonce, &aad(&b, lba), &mut b[FRAME..]);
    b[16..28].copy_from_slice(&nonce);
    b[28..44].copy_from_slice(&t);
    let at = lba as usize * SECTOR;
    medium.data[at..at + n * SECTOR].copy_from_slice(&b);
}

/// A medium in memory that counts how often each sector was written.
#[derive(Clone)]
struct Memory { data: Vec<u8>, writes: Vec<u32>, writable: bool, fail_write: bool, flushes: usize, reads: usize, fail_read_at: Option<usize>, written: usize, fail_write_at: Option<usize> }
impl Memory {
    fn new(sectors: usize) -> Self { Self { data: vec![0; sectors * SECTOR], writes: vec![0; sectors], writable: true, fail_write: false, flushes: 0, reads: 0, fail_read_at: None, written: 0, fail_write_at: None } }
    fn flip(&mut self, sector: u64, byte: usize) { self.data[sector as usize * SECTOR + byte] ^= 0x10; }
}
impl Device for &mut Memory {
    fn sectors(&self) -> u64 { self.writes.len() as u64 }
    fn writable(&self) -> bool { self.writable }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool {
        assert!(out.len() % SECTOR == 0 && out.len() <= BUFFER, "reads are whole sectors, at most a record");
        self.reads += 1;
        if self.fail_read_at == Some(self.reads) { return false; }
        let at = lba as usize * SECTOR;
        out.copy_from_slice(&self.data[at..at + out.len()]);
        true
    }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool {
        assert!(self.writable && data.len() % SECTOR == 0 && data.len() <= BUFFER);
        if self.fail_write { return false; }
        self.written += 1;
        if self.fail_write_at == Some(self.written) { return false; }
        let at = lba as usize * SECTOR;
        // A record goes only into blank sectors, so nothing stored is overwritten; only a free marker goes over the
        // record it frees.
        if lba > 0 && data.starts_with(b"MIND-SLD") {
            let marker = unseal(&self.data[..SECTOR], lba, data).is_some_and(|r| r.starts_with(b"MIND-DEL"));
            assert!(marker || self.data[at..at + data.len()].iter().all(|&b| b == 0), "a record written over sectors that are not blank at {lba}");
        }
        self.data[at..at + data.len()].copy_from_slice(data);
        for s in 0..data.len() / SECTOR { self.writes[lba as usize + s] += 1; }
        true
    }
    fn flush(&mut self) -> bool { self.flushes += 1; true }
}

struct Room { index: Vec<Entry>, heads: Vec<Head>, pins: Vec<Pin>, holes: Vec<Extent>, buffer: Box<[u8; BUFFER]>, scratch: Box<[u8; dag::CHUNK]>, now: u64 }
impl Room {
    fn new(capacity: usize) -> Self { Self::with_names(capacity, 8) }
    fn with_names(capacity: usize, names: usize) -> Self {
        Self { index: vec![Entry::EMPTY; capacity], heads: vec![Head::EMPTY; names], pins: vec![Pin::EMPTY; 8], holes: vec![Extent::EMPTY; 64], buffer: Box::new([0; BUFFER]), scratch: Box::new([0; dag::CHUNK]), now: 0 }
    }
    fn at(mut self, now: u64) -> Self { self.now = now; self }
}

fn mount<'a>(medium: &'a mut Memory, room: &'a mut Room) -> Result<Store<'a, &'a mut Memory>, Error> {
    mount_with(medium, room, &SEALER)
}

fn mount_with<'a>(medium: &'a mut Memory, room: &'a mut Room, crypto: &'a dyn Crypto) -> Result<Store<'a, &'a mut Memory>, Error> {
    Store::mount(medium, crypto, &mut room.index, &mut room.heads, &mut room.pins, &mut room.holes, &mut room.buffer, &mut room.scratch, room.now)
}

/// The badge the tests' publications and pins are made with (the shell's on the platform).
const OWNER: u16 = 7;

fn block(seed: usize, len: usize) -> Vec<u8> { (0..len).map(|i| (i * 31 + seed * 7 + 1) as u8).collect() }

fn get(store: &mut Store<&mut Memory>, cid: &Cid) -> Result<Vec<u8>, Error> {
    let mut out = vec![0u8; BLOCK_MAX];
    let len = store.get(cid, &mut out)?;
    out.truncate(len);
    Ok(out)
}

#[test]
fn put_and_get_by_content() {
    let mut medium = Memory::new(256);
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let a = store.put(Codec::Raw, b"hello world").unwrap();
    assert_eq!(a.to_string(), "bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e");
    let empty = store.put(Codec::Raw, b"").unwrap();
    let big = block(1, BLOCK_MAX);
    let b = store.put(Codec::Raw, &big).unwrap();
    assert_eq!(get(&mut store, &a).unwrap(), b"hello world");
    assert_eq!(get(&mut store, &empty).unwrap(), b"");
    assert_eq!(get(&mut store, &b).unwrap(), big);
    assert!(store.has(&a) && store.has(&b) && !store.has(&Cid::raw(b"absent")));
    assert_eq!(get(&mut store, &Cid::raw(b"absent")), Err(Error::NotFound));
    assert_eq!(store.put(Codec::Raw, &block(2, BLOCK_MAX + 1)), Err(Error::TooLarge));
    let mut small = [0u8; 4];
    assert_eq!(store.get(&a, &mut small), Err(Error::TooLarge));
    let used = 1 + record_sectors(11) + record_sectors(0) + record_sectors(BLOCK_MAX);
    assert_eq!(store.stats(), Stats { blocks: 3, bytes: 11 + BLOCK_MAX as u64, used: used as u64, sectors: 256, corrupt: 0, damaged: 0, capacity: 64, names: 0, free: 256 - used as u64, pins: 0 });
}

#[test]
fn the_same_bytes_are_stored_once() {
    let mut medium = Memory::new(64);
    let mut room = Room::new(8);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let first = store.put(Codec::Raw, &block(3, 1000)).unwrap();
    let used = store.stats().used;
    assert_eq!(store.put(Codec::Raw, &block(3, 1000)), Ok(first));
    assert_eq!((store.stats().blocks, store.stats().used), (1, used));
}

#[test]
fn a_put_returns_after_the_flush() {
    let mut medium = Memory::new(64);
    let mut room = Room::new(8);
    {
        let mut store = mount(&mut medium, &mut room).unwrap();
        store.put(Codec::Raw, b"durable").unwrap();
    }
    // The format and the put each flushed.
    assert_eq!(medium.flushes, 2);
}

#[test]
fn blocks_are_found_again_after_a_remount() {
    let mut medium = Memory::new(512);
    let mut cids = Vec::new();
    let stats;
    {
        let mut room = Room::new(64);
        let mut store = mount(&mut medium, &mut room).unwrap();
        for i in 0..20 { cids.push((store.put(Codec::Raw, &block(i, i * 700)).unwrap(), block(i, i * 700))); }
        stats = store.stats();
    }
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(store.stats(), stats);
    for (cid, data) in &cids { assert_eq!(&get(&mut store, cid).unwrap(), data); }
}

#[test]
fn a_flipped_byte_is_reported_and_never_returned() {
    let mut medium = Memory::new(128);
    let (a, b);
    {
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        a = store.put(Codec::Raw, &block(1, 2000)).unwrap();
        b = store.put(Codec::Raw, &block(2, 2000)).unwrap();
    }
    // A byte in the middle of a's data.
    medium.flip(1 + 2, 100);
    let mut room = Room::new(16);
    {
        let mut store = mount(&mut medium, &mut room).unwrap();
        // Found when mounting: a is not offered, b is.
        assert_eq!(get(&mut store, &a), Err(Error::NotFound));
        assert!(!store.has(&a));
        assert_eq!(get(&mut store, &b).unwrap(), block(2, 2000));
        assert_eq!((store.stats().blocks, store.stats().corrupt), (1, 1));
        // A put of the same bytes stores them again, after the log, without touching the damaged record.
        assert_eq!(store.put(Codec::Raw, &block(1, 2000)), Ok(a));
        assert_eq!(get(&mut store, &a).unwrap(), block(1, 2000));
    }
    assert!(medium.writes.iter().all(|&w| w <= 1), "no sector is written twice");
}

#[test]
fn a_block_damaged_after_mounting_is_refused_when_read() {
    let mut medium = Memory::new(128);
    let mut room = Room::new(16);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let a = store.put(Codec::Raw, &block(5, 600)).unwrap();
    // The last byte of the block, in its second sector.
    let last = FRAME + HEADER + 600 - 1;
    store.device().flip(1 + (last / SECTOR) as u64, last % SECTOR);
    assert_eq!(get(&mut store, &a), Err(Error::Corrupt));
    assert!(!store.has(&a));
    assert_eq!(store.stats().corrupt, 1);
    assert_eq!(get(&mut store, &a), Err(Error::NotFound));
}

#[test]
fn a_damaged_frame_loses_its_record_only() {
    let mut medium = Memory::new(128);
    let mut cids = Vec::new();
    {
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        for i in 0..4 { cids.push(store.put(Codec::Raw, &block(i, 1500)).unwrap()); }
    }
    // The second record's magic: no frame starts there, so its sectors are damaged; the third's nonce: its seal does
    // not open, so it is corrupt.
    medium.flip(1 + record_sectors(1500) as u64, 2);
    medium.flip(1 + 2 * record_sectors(1500) as u64, 20);
    let mut room = Room::new(16);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(get(&mut store, &cids[1]), Err(Error::NotFound));
    assert_eq!(get(&mut store, &cids[2]), Err(Error::NotFound));
    for i in [0, 3] { assert_eq!(get(&mut store, &cids[i]).unwrap(), block(i, 1500)); }
    let stats = store.stats();
    assert_eq!((stats.blocks, stats.damaged, stats.corrupt), (2, record_sectors(1500) as u64, 1));
    // New blocks go after the whole log.
    assert_eq!(stats.used, 1 + 4 * record_sectors(1500) as u64);
    store.put(Codec::Raw, b"after").unwrap();
    assert!(store.device().writes.iter().all(|&w| w <= 1));
}

#[test]
fn a_torn_write_is_a_corrupt_record_and_the_log_goes_on() {
    let mut medium = Memory::new(128);
    let a;
    {
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        store.put(Codec::Raw, &block(1, 3000)).unwrap();
        a = store.put(Codec::Raw, &block(2, 3000)).unwrap();
    }
    // Power lost after the header and the first sector of a's record: its other sectors are blank.
    let start = 1 + record_sectors(3000);
    for s in start + 1..start + record_sectors(3000) { medium.data[s * SECTOR..(s + 1) * SECTOR].fill(0); }
    let mut room = Room::new(16);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(get(&mut store, &a), Err(Error::NotFound));
    assert_eq!((store.stats().blocks, store.stats().corrupt), (1, 1));
    assert_eq!(store.stats().used, (1 + 2 * record_sectors(3000)) as u64);
    assert_eq!(store.put(Codec::Raw, &block(2, 3000)), Ok(a));
    assert_eq!(get(&mut store, &a).unwrap(), block(2, 3000));
}

#[test]
fn a_full_medium_or_index_refuses_a_put() {
    let mut medium = Memory::new(1 + 2 * record_sectors(4000) + 3);
    let mut room = Room::new(8);
    let mut store = mount(&mut medium, &mut room).unwrap();
    store.put(Codec::Raw, &block(1, 4000)).unwrap();
    store.put(Codec::Raw, &block(2, 4000)).unwrap();
    let before = store.stats();
    assert_eq!(store.put(Codec::Raw, &block(3, 4000)), Err(Error::Full));
    assert_eq!(store.stats(), before);
    // What still fits is taken.
    store.put(Codec::Raw, &block(4, SECTOR * 3 - FRAME - HEADER)).unwrap();
    assert_eq!(store.put(Codec::Raw, b"x"), Err(Error::Full));
    assert_eq!(get(&mut store, &Cid::raw(&block(1, 4000))).unwrap(), block(1, 4000));

    let mut medium = Memory::new(64);
    let mut room = Room::new(2);
    let mut store = mount(&mut medium, &mut room).unwrap();
    store.put(Codec::Raw, b"a").unwrap();
    store.put(Codec::Raw, b"b").unwrap();
    assert_eq!(store.put(Codec::Raw, b"c"), Err(Error::Full));
    assert_eq!(store.put(Codec::Raw, b"a").map(|c| c == Cid::raw(b"a")), Ok(true));
    drop(store);
    // A store with more blocks than the index holds is not mounted with some of them missing.
    let mut room = Room::new(1);
    assert_eq!(mount(&mut medium, &mut room).err(), Some(Error::Full));
}

#[test]
fn a_foreign_or_unknown_medium_is_left_alone() {
    // A FAT boot sector in sector 0.
    let mut medium = Memory::new(64);
    medium.data[..3].copy_from_slice(&[0xeb, 0x3c, 0x90]);
    medium.data[510..512].copy_from_slice(&[0x55, 0xaa]);
    let before = medium.data.clone();
    let mut room = Room::new(4);
    assert_eq!(mount(&mut medium, &mut room).err(), Some(Error::Foreign));
    assert_eq!(medium.data, before);

    // Sector 0 blank, a file system's superblock further on (ext4 keeps its first 1024 bytes zero).
    let mut medium = Memory::new(64);
    medium.data[1024 + 56..1024 + 58].copy_from_slice(&[0x53, 0xef]);
    let before = medium.data.clone();
    assert_eq!(mount(&mut medium, &mut Room::new(4)).err(), Some(Error::Foreign));
    assert_eq!(medium.data, before);

    // A store whose superblock is damaged, or of another layout version.
    let mut medium = Memory::new(64);
    drop(mount(&mut medium, &mut Room::new(4)).unwrap());
    let mut damaged = medium.clone();
    damaged.flip(0, 30);
    assert_eq!(mount(&mut damaged, &mut Room::new(4)).err(), Some(Error::Foreign));
    // A store of layout 2, before records were sealed: refused by its version, never converted.
    let mut other = medium.clone();
    other.data[8] = 2;
    let check = sha256::digest(&other.data[..16]);
    other.data[16..48].copy_from_slice(&check);
    let before = other.data.clone();
    assert_eq!(mount(&mut other, &mut Room::new(4)).err(), Some(Error::Layout(2)));
    assert_eq!(other.data, before);
}

#[test]
fn a_read_only_medium_is_read_but_not_written() {
    let mut medium = Memory::new(64);
    medium.writable = false;
    assert_eq!(mount(&mut medium, &mut Room::new(4)).err(), Some(Error::ReadOnly));
    medium.writable = true;
    let a;
    {
        let mut room = Room::new(4);
        let mut store = mount(&mut medium, &mut room).unwrap();
        a = store.put(Codec::Raw, b"kept").unwrap();
    }
    medium.writable = false;
    let mut room = Room::new(4);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(get(&mut store, &a).unwrap(), b"kept");
    assert_eq!(store.put(Codec::Raw, b"new"), Err(Error::ReadOnly));
    assert_eq!(store.put(Codec::Raw, b"kept"), Ok(a));
}

#[test]
fn a_failed_write_is_not_offered_and_its_sectors_are_not_reused() {
    let mut medium = Memory::new(64);
    let mut room = Room::new(4);
    let mut store = mount(&mut medium, &mut room).unwrap();
    store.device().fail_write = true;
    assert_eq!(store.put(Codec::Raw, &block(1, 700)), Err(Error::Device));
    assert!(!store.has(&Cid::raw(&block(1, 700))));
    store.device().fail_write = false;
    let used = store.stats().used;
    assert_eq!(used, 1 + record_sectors(700) as u64);
    let a = store.put(Codec::Raw, &block(1, 700)).unwrap();
    assert_eq!(store.stats().used, used + record_sectors(700) as u64);
    assert_eq!(get(&mut store, &a).unwrap(), block(1, 700));
}

#[test]
fn random_puts_gets_and_remounts_match_a_model() {
    let mut medium = Memory::new(2048);
    let mut model: HashMap<Cid, Vec<u8>> = HashMap::new();
    let mut seed = 0x2545f4914f6cdd1du64;
    let mut next = |bound: usize| { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % bound as u64) as usize };
    for round in 0..8 {
        let mut room = Room::new(256);
        let mut store = mount(&mut medium, &mut room).unwrap();
        assert_eq!(store.stats().blocks as usize, model.len(), "round {round}");
        for _ in 0..40 {
            match next(4) {
                0 | 1 => {
                    let data = block(next(60), [0, 1, 511, 512, HEADER, 4096, BLOCK_MAX][next(7)].min(next(BLOCK_MAX + 1)));
                    match store.put(Codec::Raw, &data) {
                        Ok(cid) => { assert!(cid.matches(&data)); model.insert(cid, data); }
                        Err(e) => assert_eq!(e, Error::Full),
                    }
                }
                2 if !model.is_empty() => {
                    let cid = *model.keys().nth(next(model.len())).unwrap();
                    assert_eq!(get(&mut store, &cid).unwrap(), model[&cid]);
                }
                _ => {
                    let cid = Cid::raw(&block(1000 + next(10), 10));
                    assert_eq!(store.has(&cid), model.contains_key(&cid));
                }
            }
        }
        assert_eq!(store.stats().bytes, model.values().map(|d| d.len() as u64).sum::<u64>());
    }
    assert!(medium.writes.iter().all(|&w| w <= 1), "no sector is written twice");
}

#[test]
fn rights_come_from_the_badge() {
    use rights::{allowed, Operation::*, BADGE_GET, BADGE_PUBLISH, BADGE_PUT};
    // (badge, put, get, has, stat, publish, resolve)
    for (badge, put, get, has, stat, publish, resolve) in [
        (0, false, false, false, false, false, false),
        (BADGE_GET, false, true, true, true, false, true),
        (BADGE_PUT, true, false, false, true, false, false),
        (BADGE_PUBLISH, false, false, false, true, true, false),
        (BADGE_GET | BADGE_PUT, true, true, true, true, false, true),
        (BADGE_GET | BADGE_PUT | BADGE_PUBLISH, true, true, true, true, true, true),
        // Bits of rights this version does not know grant nothing.
        (8, false, false, false, false, false, false),
        (0xfff8, false, false, false, false, false, false),
        (0xffff, true, true, true, true, true, true),
    ] {
        let got = [allowed(badge, Put), allowed(badge, Get), allowed(badge, Has), allowed(badge, Stat), allowed(badge, Publish), allowed(badge, Resolve)];
        assert_eq!(got, [put, get, has, stat, publish, resolve], "badge {badge:#x}");
        // A collection frees only what nothing retains, as a put that finds no room does: the put right. A whole scrub
        // reads the medium on the caller's behalf, as a collection does; its last report is any client's.
        assert_eq!([allowed(badge, Collect), allowed(badge, Scrub)], [put; 2], "badge {badge:#x}");
        assert_eq!(allowed(badge, Scrubbed), stat, "badge {badge:#x}");
        // Removing a name and pins are retention, as publishing is; history is reading; pins and usage are any client's.
        assert_eq!([allowed(badge, Unpublish), allowed(badge, Pin), allowed(badge, Unpin), allowed(badge, Commit)], [publish; 4], "badge {badge:#x}");
        assert_eq!([allowed(badge, History), allowed(badge, Snapshot), allowed(badge, Names)], [get; 3], "badge {badge:#x}");
        assert_eq!([allowed(badge, Pins), allowed(badge, Usage)], [stat; 2], "badge {badge:#x}");
    }
}

#[test]
fn nodes_are_stored_only_if_they_decode() {
    let mut medium = Memory::new(128);
    let node_cid;
    {
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        let chunk = store.put(Codec::Raw, b"abc").unwrap();
        let mut node = [0u8; dag::NODE_MAX];
        let len = dag::encode(3, &[chunk], &mut node);
        node_cid = store.put(Codec::DagCbor, &node[..len]).unwrap();
        assert_eq!(node_cid, Cid::of(Codec::DagCbor, &node[..len]));
        assert_eq!(get(&mut store, &node_cid).unwrap(), &node[..len]);
        // The same bytes as raw content are another block.
        let raw = store.put(Codec::Raw, &node[..len]).unwrap();
        assert_ne!(raw, node_cid);
        // Bytes that are not a node of the schema, or not its canonical encoding, are not stored as one.
        let used = store.stats().used;
        assert_eq!(store.put(Codec::DagCbor, b"abc"), Err(Error::Invalid));
        let mut longer = node[..len].to_vec();
        longer.push(0);
        assert_eq!(store.put(Codec::DagCbor, &longer), Err(Error::Invalid));
        assert_eq!(store.stats().used, used);
    }
    let mut room = Room::new(16);
    let store = mount(&mut medium, &mut room).unwrap();
    assert!(store.has(&node_cid));
}

#[test]
fn a_record_typed_as_a_node_that_does_not_decode_is_corrupt() {
    // A forged medium: a record whose CID says dag-cbor over bytes that match the digest but are no node.
    let mut medium = Memory::new(64);
    {
        let mut room = Room::new(4);
        let mut store = mount(&mut medium, &mut room).unwrap();
        store.put(Codec::Raw, b"not a node").unwrap();
    }
    let mut record = records(&medium)[0].1.clone().unwrap();
    record[17] = 0x71;
    let check = sha256::digest(&record[..52]);
    record[52..84].copy_from_slice(&check);
    plant(&mut medium, 1, &record);
    let mut room = Room::new(4);
    let store = mount(&mut medium, &mut room).unwrap();
    assert_eq!((store.stats().blocks, store.stats().corrupt), (0, 1));
    assert!(!store.has(&Cid::of(Codec::DagCbor, b"not a node")));
}

#[test]
fn a_name_changes_only_from_the_version_expected() {
    let mut medium = Memory::new(256);
    let mut room = Room::new(16);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let (a, b) = (store.put(Codec::Raw, b"first").unwrap(), store.put(Codec::Raw, b"second").unwrap());
    assert_eq!(store.resolve(b"notes"), Err(Error::NotFound));
    assert_eq!(store.publish(b"notes", 0, &a, OWNER), Ok(1));
    assert_eq!(store.resolve(b"notes"), Ok((1, a)));
    // Two writers that both read version 1: the first wins, the second learns it and nothing of it is published.
    assert_eq!(store.publish(b"notes", 1, &b, OWNER), Ok(2));
    let used = store.stats().used;
    assert_eq!(store.publish(b"notes", 1, &a, OWNER), Err(Error::Conflict));
    assert_eq!(store.publish(b"notes", 0, &a, OWNER), Err(Error::Conflict));
    assert_eq!((store.resolve(b"notes"), store.stats().used), (Ok((2, b)), used));
    // Another name is independent; the same root may be named twice.
    assert_eq!(store.publish(b"docs/a.txt", 0, &b, OWNER), Ok(1));
    assert_eq!(store.stats().names, 2);
}

#[test]
fn a_root_is_published_only_with_every_block_stored() {
    let mut medium = Memory::new(4096);
    let mut room = Room::new(1024);
    let mut store = mount(&mut medium, &mut room).unwrap();
    // An object of 40 KiB written through the store as dag's blocks, then read back through it.
    let data: Vec<u8> = (0..40_000).map(|i| (i % 241) as u8).collect();
    let mut builder = Box::new(dag::Builder::new());
    builder.write(&mut store, &data).unwrap();
    let (root, size) = builder.finish(&mut store).unwrap();
    assert_eq!((root.codec(), size), (Codec::DagCbor, 40_000));
    assert_eq!(store.publish(b"object", 0, &root, OWNER), Ok(1));
    let (_, named) = store.resolve(b"object").unwrap();
    let mut out = vec![0u8; data.len()];
    let mut done = 0;
    while done < data.len() { done += dag::read_at(&mut store, &named, done as u64, &mut out[done..], &mut Box::new([0u8; dag::CHUNK])).unwrap(); }
    assert_eq!(out, data);
    // A root whose blocks are not all stored: here a node over a chunk never put.
    let mut node = [0u8; dag::NODE_MAX];
    let missing = Cid::raw(&[7u8; dag::CHUNK]);
    let len = dag::encode(dag::CHUNK as u64 + 1, &[missing, Cid::raw(b"x")], &mut node);
    let partial = store.put(Codec::DagCbor, &node[..len]).unwrap();
    let used = store.stats().used;
    assert_eq!(store.publish(b"partial", 0, &partial, OWNER), Err(Error::Incomplete));
    assert_eq!(store.publish(b"partial", 0, &Cid::raw(b"never stored"), OWNER), Err(Error::Incomplete));
    // A node out of shape is no root.
    let len = dag::encode(5, &[Cid::raw(b"x")], &mut node);
    let small = store.put(Codec::DagCbor, &node[..len]).unwrap();
    assert_eq!(store.publish(b"partial", 0, &small, OWNER), Err(Error::Invalid));
    assert_eq!((store.stats().used, store.resolve(b"partial")), (used + record_sectors(len) as u64, Err(Error::NotFound)));
}

#[test]
fn names_are_found_again_after_a_remount() {
    let mut medium = Memory::new(256);
    let (a, b);
    {
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        a = store.put(Codec::Raw, b"a").unwrap();
        b = store.put(Codec::Raw, b"b").unwrap();
        for (i, root) in [a, b, a, b, a].iter().enumerate() { assert_eq!(store.publish(b"head", i as u64, root, OWNER), Ok(i as u64 + 1)); }
        store.publish(b"other", 0, &b, OWNER).unwrap();
    }
    let mut room = Room::new(16);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!((store.resolve(b"head"), store.resolve(b"other")), (Ok((5, a)), Ok((1, b))));
    assert_eq!(store.publish(b"head", 5, &b, OWNER), Ok(6));
    assert!(store.device().writes.iter().all(|&w| w <= 1), "no sector is written twice");
}

#[test]
fn a_damaged_name_record_is_reported_and_the_version_before_stands() {
    let mut medium = Memory::new(64);
    let (a, b);
    {
        let mut room = Room::new(8);
        let mut store = mount(&mut medium, &mut room).unwrap();
        a = store.put(Codec::Raw, b"a").unwrap();
        b = store.put(Codec::Raw, b"b").unwrap();
        store.publish(b"head", 0, &a, OWNER).unwrap();
        store.publish(b"head", 1, &b, OWNER).unwrap();
    }
    // The last record is version 2's: two block records of one sector each, then versions 1 and 2.
    assert_eq!(find(&medium, b"MIND-REF"), vec![3, 4]);
    medium.flip(4, 300);
    let mut room = Room::new(8);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(store.resolve(b"head"), Ok((1, a)));
    assert_eq!((store.stats().corrupt, store.stats().damaged), (1, 0));
    // The next publication still goes after the damaged sector and takes the next version of what is current.
    assert_eq!(store.publish(b"head", 1, &b, OWNER), Ok(2));
}

#[test]
fn names_are_checked_and_bounded() {
    let mut medium = Memory::new(64);
    let mut room = Room::with_names(8, 2);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let a = store.put(Codec::Raw, b"a").unwrap();
    let long = [b'n'; NAME_MAX + 1];
    for name in [&b""[..], &long[..], b"with space", b"a\\b", "имя".as_bytes(), b"a:b"] {
        assert_eq!(store.publish(name, 0, &a, OWNER), Err(Error::Invalid), "{name:?}");
        assert_eq!(store.resolve(name), Err(Error::Invalid));
    }
    assert_eq!(store.publish(&long[..NAME_MAX], 0, &a, OWNER), Ok(1));
    assert_eq!(store.publish(b"A-z_0.9/x", 0, &a, OWNER), Ok(1));
    assert_eq!(store.publish(b"third", 0, &a, OWNER), Err(Error::Full));
    assert_eq!(store.publish(b"A-z_0.9/x", 1, &a, OWNER), Ok(2));
    drop(store);
    medium.writable = false;
    let mut room = Room::with_names(8, 2);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(store.publish(b"A-z_0.9/x", 2, &a, OWNER), Err(Error::ReadOnly));
    // A medium with more names than the table holds is not mounted with some missing.
    medium.writable = true;
    assert_eq!(mount(&mut medium, &mut Room::with_names(8, 1)).err(), Some(Error::Full));
}

#[test]
fn a_block_cannot_plant_a_record_for_a_scan_after_damage() {
    // A record's second sector starts SECTOR - FRAME - HEADER bytes into the block's data: a client puts there the
    // frame of a record it would like a scan to find once the record's own frame is damaged.
    let at = SECTOR - FRAME - HEADER;
    let mut medium = Memory::new(128);
    let a;
    {
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        let mut data = vec![0u8; at + 2 * SECTOR];
        for offset in [at, at + SECTOR] {
            data[offset..offset + 8].copy_from_slice(b"MIND-SLD");
            data[offset + 8..offset + 10].copy_from_slice(&LAYOUT.to_le_bytes());
            data[offset + 12] = 1;
        }
        a = store.put(Codec::Raw, &data).unwrap();
        store.publish(b"after", 0, &a, OWNER).unwrap();
    }
    // The bytes are sealed: on the medium nothing of them starts a sector.
    let n = record_sectors(at + 2 * SECTOR);
    assert!((2..1 + n).all(|s| medium.data[s * SECTOR..s * SECTOR + 8] != *b"MIND-SLD"));
    medium.flip(1, 0);
    let mut room = Room::new(16);
    let store = mount(&mut medium, &mut room).unwrap();
    let stats = store.stats();
    assert_eq!((stats.blocks, stats.names, stats.corrupt, stats.damaged), (0, 1, 0, n as u64));
}

// An object of `len` bytes written into the store; its root.
fn object(store: &mut Store<&mut Memory>, seed: usize, len: usize) -> Cid {
    let mut builder = Box::new(dag::Builder::new());
    builder.write(store, &block(seed, len)).unwrap();
    builder.finish(store).unwrap().0
}

fn read_object(store: &mut Store<&mut Memory>, root: &Cid) -> Vec<u8> {
    let mut buffer = Box::new([0u8; dag::CHUNK]);
    let size = dag::size(store, root, &mut buffer).unwrap() as usize;
    let mut out = vec![0u8; size];
    let mut done = 0;
    while done < size { done += dag::read_at(store, root, done as u64, &mut out[done..], &mut buffer).unwrap(); }
    out
}

#[test]
fn a_collection_frees_what_no_name_retains_once_its_lease_ends() {
    let mut medium = Memory::new(1024);
    let mut room = Room::new(256);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let (a, b) = (store.put(Codec::Raw, &block(1, 3000)).unwrap(), store.put(Codec::Raw, b"b").unwrap());
    let kept = object(&mut store, 2, 40_000);
    assert_eq!(store.publish(b"keep", 0, &kept, OWNER), Ok(1));
    let before = store.stats();
    // Leases still run: nothing goes.
    store.set_time(LEASE_NS - 1);
    assert_eq!(store.collect(), Ok(Collected { blocks: 0, names: 0, sectors: 0, free: before.free }));
    // They have ended: what no name retains goes, the object stays whole.
    store.set_time(LEASE_NS);
    let freed = (record_sectors(3000) + record_sectors(1)) as u64;
    assert_eq!(store.collect(), Ok(Collected { blocks: 2, names: 0, sectors: freed, free: before.free + freed }));
    assert!(!store.has(&a) && !store.has(&b));
    assert_eq!(read_object(&mut store, &kept), block(2, 40_000));
    assert_eq!(store.stats().blocks, before.blocks - 2);
    // A new version keeps the first in the name's history: nothing goes.
    let next = object(&mut store, 3, 20_000);
    assert_eq!(store.publish(b"keep", 1, &next, OWNER), Ok(2));
    store.set_time(2 * LEASE_NS);
    assert_eq!((store.collect().unwrap().names, store.collect().unwrap().blocks), (0, 0));
    assert_eq!(read_object(&mut store, &kept), block(2, 40_000));
    // HISTORY versions later the first falls out: its record and the blocks only it held go.
    let mut last = next;
    for v in 2..=HISTORY as u64 {
        last = store.put(Codec::Raw, &block(10 + v as usize, 100)).unwrap();
        assert_eq!(store.publish(b"keep", v, &last, OWNER), Ok(v + 1));
    }
    store.set_time(4 * LEASE_NS);
    let collected = store.collect().unwrap();
    // The 40 KB object's node and its chunks: its first two chunks are the same bytes (block() repeats every 256), one block.
    assert_eq!((collected.names, collected.blocks), (1, 3));
    assert_eq!(store.resolve(b"keep"), Ok((HISTORY as u64 + 1, last)));
    assert_eq!(store.history(b"keep").unwrap().iter().map(|v| v.version).collect::<Vec<_>>(), (2..=HISTORY as u64 + 1).rev().collect::<Vec<_>>());
    assert_eq!(read_object(&mut store, &next), block(3, 20_000));
    assert!(!store.has(&kept));
    drop(store);
    // A mount finds the same.
    let mut room = Room::new(256).at(4 * LEASE_NS);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(store.resolve(b"keep"), Ok((HISTORY as u64 + 1, last)));
    assert_eq!(store.history(b"keep").unwrap().len(), HISTORY);
    assert_eq!((store.stats().corrupt, store.stats().damaged), (0, 0));
    assert_eq!(read_object(&mut store, &next), block(3, 20_000));
}

#[test]
fn a_put_starts_the_lease_again() {
    let mut medium = Memory::new(64);
    let mut room = Room::new(16);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let a = store.put(Codec::Raw, b"a").unwrap();
    store.set_time(LEASE_NS - 1);
    assert_eq!(store.put(Codec::Raw, b"a"), Ok(a));
    store.set_time(LEASE_NS + 5);
    assert_eq!(store.collect().unwrap().blocks, 0);
    store.set_time(2 * LEASE_NS - 1);
    assert_eq!(store.collect().unwrap().blocks, 1);
    assert!(!store.has(&a));
}

#[test]
fn freed_room_is_written_again_when_the_medium_is_full() {
    let mut medium = Memory::new(1 + 6 * record_sectors(4000));
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let kept = store.put(Codec::Raw, &block(0, 4000)).unwrap();
    store.publish(b"kept", 0, &kept, OWNER).unwrap();
    for i in 1..5 { store.put(Codec::Raw, &block(i, 4000)).unwrap(); }
    // Full while every lease runs: a collection frees nothing, the put is refused.
    assert_eq!(store.put(Codec::Raw, &block(9, 4000)), Err(Error::Full));
    // Later the put finds the room a collection frees, between records that stay.
    store.set_time(LEASE_NS);
    let fresh = store.put(Codec::Raw, &block(9, 4000)).unwrap();
    assert_eq!(store.stats().blocks, 2);
    for i in 10..13 { store.put(Codec::Raw, &block(i, 4000)).unwrap(); }
    drop(store);
    let mut room = Room::new(64).at(LEASE_NS);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(get(&mut store, &kept).unwrap(), block(0, 4000));
    assert_eq!(get(&mut store, &fresh).unwrap(), block(9, 4000));
    assert_eq!((store.stats().blocks, store.stats().corrupt, store.stats().damaged), (5, 0, 0));
}

#[test]
fn nothing_is_collected_while_a_name_lacks_a_block() {
    let mut medium = Memory::new(512);
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let loose = store.put(Codec::Raw, b"loose").unwrap();
    let root = object(&mut store, 4, 40_000);
    store.publish(b"obj", 0, &root, OWNER).unwrap();
    // A chunk of the object found corrupt leaves the index.
    let chunk = Cid::raw(&block(4, 40_000)[16384..32768]);
    let lba = records(store.device()).into_iter().find(|(_, r)| r.as_ref().is_some_and(|r| r.starts_with(b"MIND-BLK") && r[16..52] == chunk.to_bytes())).unwrap().0;
    store.device().flip(lba + 3, 7);
    assert_eq!(get(&mut store, &chunk), Err(Error::Corrupt));
    store.set_time(LEASE_NS);
    let before = store.stats();
    assert_eq!(store.collect(), Err(Error::Incomplete));
    assert!(store.has(&loose));
    assert_eq!(store.stats(), before);
    // The chunk put again: the collection goes ahead, and frees the corrupt copy too.
    store.put(Codec::Raw, &block(4, 40_000)[16384..32768]).unwrap();
    let collected = store.collect().unwrap();
    assert_eq!(collected.blocks, 2);
    assert_eq!((store.has(&loose), store.stats().corrupt), (false, 0));
    assert_eq!(read_object(&mut store, &root), block(4, 40_000));
}

#[test]
fn a_collection_stopped_in_the_middle_is_finished_by_the_next_mount() {
    let mut medium = Memory::new(64);
    let x;
    {
        let mut room = Room::new(8);
        let mut store = mount(&mut medium, &mut room).unwrap();
        store.put(Codec::Raw, b"first").unwrap();
        x = store.put(Codec::Raw, &block(5, 2000)).unwrap();
        store.put(Codec::Raw, b"last").unwrap();
    }
    // Stopped after the marker over x's record: its data is still there.
    let n = record_sectors(2000) as u32;
    let at = 2 * SECTOR;
    let mut marker = [0u8; 48];
    marker[..8].copy_from_slice(b"MIND-DEL");
    marker[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
    marker[12..16].copy_from_slice(&n.to_le_bytes());
    let check = sha256::digest(&marker[..16]);
    marker[16..48].copy_from_slice(&check);
    plant(&mut medium, 2, &marker);
    let mut room = Room::new(8);
    let store = mount(&mut medium, &mut room).unwrap();
    assert!(!store.has(&x));
    assert_eq!((store.stats().blocks, store.stats().corrupt, store.stats().damaged), (2, 0, 0));
    drop(store);
    assert!(medium.data[at..at + n as usize * SECTOR].iter().all(|&b| b == 0), "the freed sectors are blank");
}

#[test]
fn random_puts_names_collections_and_remounts_match_a_model() {
    let mut medium = Memory::new(768);
    let mut seed = 0x9e3779b97f4a7c15u64;
    let mut next = |bound: u64| { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed % bound };
    // Every block put: its bytes and when its lease began; names and the block each points at.
    let mut blocks: HashMap<Cid, (Vec<u8>, u64)> = HashMap::new();
    // Each name's kept versions, newest first.
    let mut names: HashMap<Vec<u8>, Vec<(u64, Cid)>> = HashMap::new();
    let mut now = 0u64;
    for round in 0..6 {
        let mut room = Room::new(128).at(now);
        let mut store = mount(&mut medium, &mut room).unwrap();
        for (_, lease) in blocks.values_mut() { *lease = now; }
        for _ in 0..60 {
            now += next(LEASE_NS / 8);
            store.set_time(now);
            let retained = |blocks: &HashMap<Cid, (Vec<u8>, u64)>, names: &HashMap<Vec<u8>, Vec<(u64, Cid)>>, cid: &Cid| {
                names.values().flatten().any(|(_, r)| r == cid) || now < blocks[cid].1 + LEASE_NS
            };
            match next(6) {
                0 | 1 => {
                    let data = block(next(40) as usize, [1, 600, 3000, 9000][next(4) as usize]);
                    match store.put(Codec::Raw, &data) {
                        Ok(cid) => { blocks.insert(cid, (data, now)); }
                        Err(e) => assert_eq!(e, Error::Full, "round {round}"),
                    }
                }
                2 if !blocks.is_empty() => {
                    let cid = *blocks.keys().nth(next(blocks.len() as u64) as usize).unwrap();
                    let name = vec![b'n', b'0' + next(3) as u8];
                    let version = names.get(&name).map_or(0, |h| h[0].0);
                    match store.publish(&name, version, &cid, OWNER) {
                        Ok(v) => {
                            assert_eq!(v, version + 1);
                            let kept = names.entry(name).or_default();
                            kept.insert(0, (v, cid));
                            kept.truncate(HISTORY);
                        }
                        Err(Error::Incomplete) => assert!(!store.has(&cid)),
                        Err(e) => assert_eq!(e, Error::Full),
                    }
                }
                3 => {
                    store.collect().unwrap();
                    for cid in blocks.keys() { assert_eq!(store.has(cid), retained(&blocks, &names, cid), "round {round}"); }
                }
                // Two names at once (304-STO-0007): both change or neither; sometimes from a stale version.
                4 if blocks.len() >= 2 => {
                    let first = next(3) as u8;
                    let pair = [vec![b'n', b'0' + first], vec![b'n', b'0' + (first + 1 + next(2) as u8) % 3]];
                    let roots = [0, 1].map(|_| *blocks.keys().nth(next(blocks.len() as u64) as usize).unwrap());
                    let current = [0, 1].map(|k| names.get(&pair[k]).map_or(0, |h| h[0].0));
                    let stale = next(4) == 0;
                    let updates = [0, 1].map(|k| Update { name: &pair[k], expected: current[k] + (stale && k == 0) as u64, root: Some(roots[k]) });
                    match store.commit(&updates, OWNER) {
                        Ok(v) => {
                            assert!(!stale);
                            assert_eq!(v[..2], [current[0] + 1, current[1] + 1]);
                            for k in 0..2 {
                                let kept = names.entry(pair[k].clone()).or_default();
                                kept.insert(0, (v[k], roots[k]));
                                kept.truncate(HISTORY);
                            }
                        }
                        Err(Error::Conflict) => assert!(stale),
                        Err(Error::Incomplete) => assert!(!store.has(&roots[0]) || !store.has(&roots[1])),
                        Err(e) => assert_eq!(e, Error::Full),
                    }
                }
                _ => {}
            }
            // What is retained is always there and whole, whatever collections the puts started.
            let keys: Vec<Cid> = blocks.keys().copied().collect();
            for cid in keys.iter().filter(|c| retained(&blocks, &names, c)) {
                assert_eq!(get(&mut store, cid).unwrap(), blocks[cid].0, "round {round}");
            }
            blocks.retain(|cid, _| store.has(cid));
            for (name, kept) in &names {
                assert_eq!(store.resolve(name), Ok(kept[0]));
                assert_eq!(store.history(name).unwrap().iter().map(|v| (v.version, v.root().unwrap())).collect::<Vec<_>>(), *kept);
            }
        }
        assert_eq!((store.stats().corrupt, store.stats().damaged), (0, 0));
    }
}

#[test]
fn leases_start_again_when_a_mount_is_done() {
    let mut medium = Memory::new(64);
    let a;
    {
        let mut room = Room::new(8);
        let mut store = mount(&mut medium, &mut room).unwrap();
        a = store.put(Codec::Raw, b"a").unwrap();
    }
    // The mount began at 0 and took longer than a lease: the leases run from when it is done.
    let mut room = Room::new(8);
    let mut store = mount(&mut medium, &mut room).unwrap();
    store.renew(3 * LEASE_NS);
    store.set_time(4 * LEASE_NS - 1);
    assert_eq!(store.collect().unwrap().blocks, 0);
    store.set_time(4 * LEASE_NS);
    assert_eq!(store.collect().unwrap().blocks, 1);
    assert!(!store.has(&a));
}

#[test]
fn a_removed_name_retains_nothing_and_keeps_its_version() {
    let mut medium = Memory::new(512);
    let (a, b);
    {
        let mut room = Room::new(64);
        let mut store = mount(&mut medium, &mut room).unwrap();
        a = object(&mut store, 6, 30_000);
        assert_eq!(store.publish(b"doc", 0, &a, OWNER), Ok(1));
        assert_eq!(store.unpublish(b"doc", 0, OWNER), Err(Error::Conflict));
        assert_eq!(store.unpublish(b"doc", 1, OWNER), Ok(2));
        assert_eq!(store.resolve(b"doc"), Err(Error::NotFound));
        assert_eq!(store.unpublish(b"doc", 2, OWNER), Err(Error::NotFound));
        assert_eq!(store.unpublish(b"none", 0, OWNER), Err(Error::NotFound));
        let history = store.history(b"doc").unwrap();
        assert_eq!((history.len(), history[0].version, history[0].root()), (1, 2, None));
        // The data goes only when a collection finds nothing else retains it (deleting a reference is not deleting data).
        assert!(store.has(&a));
        store.set_time(LEASE_NS);
        let collected = store.collect().unwrap();
        assert_eq!(collected.names, 1);
        assert!(!store.has(&a));
        // Publishing from the removal's version creates the name again; a stale version cannot.
        b = store.put(Codec::Raw, b"again").unwrap();
        assert_eq!(store.publish(b"doc", 0, &b, OWNER), Err(Error::Conflict));
        assert_eq!(store.publish(b"doc", 2, &b, OWNER), Ok(3));
        assert_eq!(store.usage(OWNER).names, 1);
    }
    let mut room = Room::new(64).at(LEASE_NS);
    let store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(store.resolve(b"doc"), Ok((3, b)));
    assert_eq!(store.history(b"doc").unwrap().iter().map(|v| (v.version, v.root())).collect::<Vec<_>>(), vec![(3, Some(b)), (2, None)]);
}

#[test]
fn a_pin_retains_its_object_until_its_owner_unpins_it() {
    let mut medium = Memory::new(512);
    let (root, id);
    {
        let mut room = Room::new(64);
        let mut store = mount(&mut medium, &mut room).unwrap();
        root = object(&mut store, 7, 30_000);
        assert_eq!(store.pin(&Cid::raw(b"never stored"), OWNER), Err(Error::Incomplete));
        id = store.pin(&root, OWNER).unwrap();
        assert_eq!(store.usage(OWNER), Usage { retained: 30_000, quota: 512 * 512 / 4 * 3, names: 0, pins: 1 });
        assert_eq!(store.usage(OWNER + 1).retained, 0);
        store.set_time(LEASE_NS);
        assert_eq!(store.collect().unwrap().blocks, 0);
        assert_eq!(store.unpin(id, OWNER + 1), Err(Error::Rights));
        assert_eq!(store.unpin(id + 1, OWNER), Err(Error::NotFound));
    }
    // A pin survives a mount, and its ids go on.
    let mut room = Room::new(64).at(LEASE_NS);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(store.pins(OWNER).map(|p| (p.id, p.root(), p.size)).collect::<Vec<_>>(), vec![(id, root, 30_000)]);
    let second = store.pin(&root, OWNER).unwrap();
    assert!(second > id);
    // Two pins of one object count it once.
    assert_eq!(store.usage(OWNER).retained, 30_000);
    assert_eq!(store.unpin(id, OWNER), Ok(()));
    assert_eq!(store.unpin(second, OWNER), Ok(()));
    assert_eq!(store.stats().pins, 0);
    store.set_time(2 * LEASE_NS);
    assert!(store.collect().unwrap().blocks > 0);
    assert!(!store.has(&root));
    drop(store);
    let mut room = Room::new(64).at(2 * LEASE_NS);
    let store = mount(&mut medium, &mut room).unwrap();
    assert_eq!((store.stats().pins, store.stats().damaged), (0, 0));
}

#[test]
fn an_owner_retains_no_more_than_its_quota() {
    // A medium of 128 sectors: each owner may retain 48 KiB.
    let mut medium = Memory::new(128);
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let quota = 128 * 512 / 4 * 3;
    let big = object(&mut store, 8, 30_000);
    let other = object(&mut store, 9, 20_000);
    assert_eq!(store.publish(b"big", 0, &big, OWNER), Ok(1));
    // The same object under a second name and a pin costs nothing more.
    assert_eq!(store.publish(b"same", 0, &big, OWNER), Ok(1));
    store.pin(&big, OWNER).unwrap();
    assert_eq!(store.usage(OWNER).retained, 30_000);
    // Another 20 000 bytes would pass 49 152.
    assert!(30_000 + 20_000 > quota);
    assert_eq!(store.publish(b"other", 0, &other, OWNER), Err(Error::Quota));
    assert_eq!(store.pin(&other, OWNER), Err(Error::Quota));
    assert_eq!(store.resolve(b"other"), Err(Error::NotFound));
    // Another owner has a quota of its own.
    assert_eq!(store.publish(b"other", 0, &other, OWNER + 1), Ok(1));
    assert_eq!(store.usage(OWNER + 1).retained, 20_000);
}

#[test]
fn each_version_links_the_one_before() {
    // MC-4.5: the history's links are explicit on the medium, from each version's record to the root before it.
    let mut medium = Memory::new(256);
    let (a, b);
    {
        let mut room = Room::new(64);
        let mut store = mount(&mut medium, &mut room).unwrap();
        a = store.put(Codec::Raw, b"first").unwrap();
        b = store.put(Codec::Raw, b"second").unwrap();
        assert_eq!(store.publish(b"doc", 0, &a, OWNER), Ok(1));
        assert_eq!(store.publish(b"doc", 1, &b, OWNER + 1), Ok(2));
        assert_eq!(store.unpublish(b"doc", 2, OWNER), Ok(3));
    }
    // Each name record: version, root (none for the removal), the root before it (none for the first), owner.
    let mut records: Vec<(u64, Vec<u8>, Vec<u8>, u16)> = records(&medium).into_iter().filter_map(|(_, r)| r).filter(|s| s.starts_with(b"MIND-REF"))
        .map(|s| (u64::from_le_bytes(s[12..20].try_into().unwrap()), s[20..56].to_vec(), s[56..92].to_vec(), u16::from_le_bytes([s[158], s[159]])))
        .collect();
    records.sort();
    let none = vec![0u8; 36];
    assert_eq!(records, vec![
        (1, a.to_bytes().to_vec(), none.clone(), OWNER),
        (2, b.to_bytes().to_vec(), a.to_bytes().to_vec(), OWNER + 1),
        (3, none, b.to_bytes().to_vec(), OWNER),
    ]);
    let mut room = Room::new(64);
    let store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(store.history(b"doc").unwrap().iter().map(|v| (v.version, v.owner)).collect::<Vec<_>>(), vec![(3, OWNER)]);
}

// Two names and a third, each with a stored object; the store after `commit(a, b)` from their first versions.
fn two_names(store: &mut Store<&mut Memory>) -> [Cid; 4] {
    let roots = [object(store, 21, 20_000), object(store, 22, 100), object(store, 23, 30_000), object(store, 24, 50)];
    assert_eq!(store.publish(b"a", 0, &roots[0], OWNER), Ok(1));
    assert_eq!(store.publish(b"b", 0, &roots[1], OWNER), Ok(1));
    roots
}

#[test]
fn a_commit_changes_every_name_or_none() {
    let mut medium = Memory::new(512);
    let mut room = Room::with_names(64, 16);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let [a1, b1, a2, b2] = two_names(&mut store);
    let free = store.stats().free;
    // Refusals write nothing and change no name: a stale version, a root not stored, the same name twice, too many.
    let refused: [(&[Update], Error); 6] = [
        (&[Update { name: b"a", expected: 1, root: Some(a2) }, Update { name: b"b", expected: 2, root: Some(b2) }], Error::Conflict),
        (&[Update { name: b"a", expected: 1, root: Some(a2) }, Update { name: b"c", expected: 0, root: Some(Cid::raw(b"never stored")) }], Error::Incomplete),
        (&[Update { name: b"a", expected: 1, root: Some(a2) }, Update { name: b"a", expected: 1, root: Some(b2) }], Error::Invalid),
        (&[Update { name: b"a", expected: 1, root: Some(a2) }, Update { name: b"gone", expected: 0, root: None }], Error::NotFound),
        (&[Update { name: b"a:b", expected: 0, root: Some(a2) }], Error::Invalid),
        (&[], Error::Invalid),
    ];
    for (updates, error) in refused {
        assert_eq!(store.commit(updates, OWNER).map(|_| ()), Err(error));
        assert_eq!((store.resolve(b"a"), store.resolve(b"b"), store.resolve(b"c")), (Ok((1, a1)), Ok((1, b1)), Err(Error::NotFound)));
        assert_eq!(store.stats().free, free);
    }
    let many: Vec<Vec<u8>> = (0..=COMMIT_MAX).map(|k| format!("n{k}").into_bytes()).collect();
    let many: Vec<Update> = many.iter().map(|name| Update { name, expected: 0, root: Some(b2) }).collect();
    assert_eq!(store.commit(&many, OWNER).map(|_| ()), Err(Error::Invalid));
    assert_eq!(store.commit(&many[..COMMIT_MAX], OWNER).map(|v| v[COMMIT_MAX - 1]), Ok(1));
    // Both from their current versions, one name new and one removed: every change at once.
    let updates = [Update { name: b"a", expected: 1, root: Some(a2) }, Update { name: b"b", expected: 1, root: None }, Update { name: b"c", expected: 0, root: Some(b2) }];
    assert_eq!(store.commit(&updates, OWNER).map(|v| v[..3].to_vec()), Ok(vec![2, 2, 1]));
    assert_eq!((store.resolve(b"a"), store.resolve(b"b"), store.resolve(b"c")), (Ok((2, a2)), Err(Error::NotFound), Ok((1, b2))));
    assert_eq!([b"a", b"b", b"c", b"z"].map(|n| store.snapshot(n).unwrap()), [(2, Some(a2)), (2, None), (1, Some(b2)), (0, None)]);
    // Its versions link the roots before them, as a publication's do.
    assert_eq!(store.history(b"a").unwrap().iter().map(|v| (v.version, v.root())).collect::<Vec<_>>(), vec![(2, Some(a2)), (1, Some(a1))]);
    drop(store);
    // A mount finds the commit whole.
    let mut room = Room::with_names(64, 16);
    let store = mount(&mut medium, &mut room).unwrap();
    assert_eq!([b"a", b"b", b"c"].map(|n| store.snapshot(n).unwrap()), [(2, Some(a2)), (2, None), (1, Some(b2))]);
    assert_eq!((store.stats().corrupt, store.stats().damaged), (0, 0));
}

// Every current name with `prefix`, asked for 16 at a time after the last one, as a client pages through them.
fn paged(store: &Store<&mut Memory>, prefix: &[u8]) -> Result<(Vec<(String, u64)>, usize), Error> {
    let (mut all, mut calls) = (Vec::<(String, u64)>::new(), 0);
    loop {
        let after = all.last().map(|(n, _)| n.clone().into_bytes()).unwrap_or_default();
        let page: Vec<(String, u64)> = store.names(prefix, &after)?.take(16).map(|(n, v, _)| (String::from_utf8(n.to_vec()).unwrap(), v)).collect();
        calls += 1;
        let last = page.len() < 16;
        all.extend(page);
        if last { return Ok((all, calls)); }
    }
}

#[test]
fn names_are_listed_by_prefix_in_pages() {
    let mut medium = Memory::new(512);
    let mut room = Room::with_names(64, 64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let root = object(&mut store, 31, 100);
    // Published out of order: the list comes in byte order whatever the order of publishing.
    let names: Vec<String> = (0..20).rev().map(|k| format!("list/n{k:02}")).chain(["copy", "list", "lists", "list/n05/x", "z"].map(String::from)).collect();
    for name in &names { assert_eq!(store.publish(name.as_bytes(), 0, &root, OWNER), Ok(1)); }
    assert_eq!(store.publish(b"copy", 1, &root, OWNER), Ok(2));
    assert_eq!(store.unpublish(b"z", 1, OWNER), Ok(2));
    let mut listed: Vec<String> = (0..20).map(|k| format!("list/n{k:02}")).collect();
    listed.insert(6, "list/n05/x".into());
    let (all, calls) = paged(&store, b"list/").unwrap();
    assert_eq!(all.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(), listed);
    assert_eq!(calls, 2);
    // Everything: the removed name is not listed, a name's current version is.
    let (all, calls) = paged(&store, b"").unwrap();
    assert_eq!(all.len(), 24);
    assert_eq!((all[0].clone(), all[1].0.as_str(), all[23].0.as_str(), calls), (("copy".into(), 2), "list", "lists", 2));
    // A page after a name that is not stored starts at the next one; one past the run of the prefix is empty.
    let after: Vec<&[u8]> = store.names(b"list/n1", b"list/n12a").unwrap().map(|(n, _, _)| n).collect();
    assert_eq!(after, [&b"list/n13"[..], b"list/n14", b"list/n15", b"list/n16", b"list/n17", b"list/n18", b"list/n19"]);
    assert_eq!(store.names(b"list/n1", b"list/n19").unwrap().count(), 0);
    assert_eq!(store.names(b"list/n1", b"a").unwrap().count(), 10);
    assert_eq!(store.names(b"q", b"").unwrap().count(), 0);
    // Fewer than 16 come in one call.
    assert_eq!(paged(&store, b"list/n0").map(|(a, c)| (a.len(), c)), Ok((11, 1)));
    // A prefix or a start with a byte no name has; one of 64 bytes is a name's whole length.
    assert_eq!(store.names(b"a:b", b"").map(|n| n.count()).err(), Some(Error::Invalid));
    assert_eq!(store.names(b"", b"a b").map(|n| n.count()).err(), Some(Error::Invalid));
    assert_eq!(store.names(&[b'a'; NAME_MAX], b"").map(|n| n.count()), Ok(0));
    assert_eq!(store.names(&[b'a'; NAME_MAX + 1], b"").map(|n| n.count()).err(), Some(Error::Invalid));
    drop(store);
    // A mount lists the same names.
    let mut room = Room::with_names(64, 64);
    let store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(paged(&store, b"list/").unwrap().0.into_iter().map(|(n, _)| n).collect::<Vec<_>>(), listed);
}

#[test]
fn a_commit_past_the_quota_changes_nothing() {
    let mut medium = Memory::new(128);
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let (big, other) = (object(&mut store, 8, 30_000), object(&mut store, 9, 20_000));
    let updates = [Update { name: b"big", expected: 0, root: Some(big) }, Update { name: b"other", expected: 0, root: Some(other) }];
    assert_eq!(store.commit(&updates, OWNER).map(|_| ()), Err(Error::Quota));
    assert_eq!((store.resolve(b"big"), store.resolve(b"other")), (Err(Error::NotFound), Err(Error::NotFound)));
    // The same object under two names counts once.
    let updates = [Update { name: b"big", expected: 0, root: Some(big) }, Update { name: b"same", expected: 0, root: Some(big) }];
    assert!(store.commit(&updates, OWNER).is_ok());
    assert_eq!(store.usage(OWNER).retained, 30_000);
}

#[test]
fn a_damaged_or_torn_commit_changes_no_name() {
    // A byte of any sector of the record, or a write that stopped before its last sector: a mount applies none of it.
    for case in 0..5 {
        let mut medium = Memory::new(512);
        let (a1, b1, at);
        {
            let mut room = Room::new(64);
            let mut store = mount(&mut medium, &mut room).unwrap();
            let [x1, y1, a2, b2] = two_names(&mut store);
            (a1, b1) = (x1, y1);
            let updates = [Update { name: b"a", expected: 1, root: Some(a2) }, Update { name: b"b", expected: 1, root: Some(b2) }];
            store.commit(&updates, OWNER).unwrap();
        }
        at = find(&medium, b"MIND-TXN")[0];
        // Two entries: a header and two entry sectors, sealed in four.
        let n = sealed_sectors(3 * SECTOR) as u64;
        match case {
            0 => medium.flip(at, 20),
            1 => medium.flip(at + 1, 30),
            2 => medium.flip(at + 2, 100),
            3 => medium.data[(at + 3) as usize * SECTOR..(at + 4) as usize * SECTOR].fill(0),
            _ => medium.flip(at, 0),
        }
        let mut room = Room::new(64);
        let store = mount(&mut medium, &mut room).unwrap();
        assert_eq!((store.resolve(b"a"), store.resolve(b"b")), (Ok((1, a1)), Ok((1, b1))), "case {case}");
        // A record that does not open is corrupt; one whose frame is damaged leaves its sectors damaged.
        assert_eq!((store.stats().corrupt, store.stats().damaged), if case == 4 { (0, n) } else { (1, 0) }, "case {case}");
    }
}

#[test]
fn a_commit_record_goes_once_no_name_keeps_its_versions() {
    let mut medium = Memory::new(512);
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let [_, _, a2, b2] = two_names(&mut store);
    let updates = [Update { name: b"a", expected: 1, root: Some(a2) }, Update { name: b"b", expected: 1, root: Some(b2) }];
    store.commit(&updates, OWNER).unwrap();
    let txn = |m: &Memory| find(m, b"MIND-TXN").len();
    // a moves on HISTORY versions; b still keeps version 2: the record stays.
    for v in 2..2 + HISTORY as u64 { assert_eq!(store.publish(b"a", v, &a2, OWNER), Ok(v + 1)); }
    store.set_time(LEASE_NS);
    store.collect().unwrap();
    assert_eq!(store.history(b"b").unwrap()[0].version, 2);
    drop(store);
    assert_eq!(txn(&medium), 1);
    let mut room = Room::new(64).at(LEASE_NS);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!((store.snapshot(b"a").unwrap().0, store.snapshot(b"b").unwrap()), (2 + HISTORY as u64, (2, Some(b2))));
    // Once b moves on too, the record goes with the versions no name keeps.
    for v in 2..2 + HISTORY as u64 { assert_eq!(store.publish(b"b", v, &b2, OWNER), Ok(v + 1)); }
    store.set_time(2 * LEASE_NS);
    assert!(store.collect().unwrap().names >= 2);
    drop(store);
    assert_eq!(txn(&medium), 0);
    let mut room = Room::new(64).at(2 * LEASE_NS);
    let store = mount(&mut medium, &mut room).unwrap();
    assert_eq!((store.snapshot(b"a").unwrap().0, store.snapshot(b"b").unwrap().0, store.stats().damaged), (2 + HISTORY as u64, 2 + HISTORY as u64, 0));
}


#[test]
fn a_collection_cut_by_a_failure_leaves_no_erased_block_acknowledged() {
    // Audit A05 (175-STO-0012): a sweep that fails after erasing a record must not leave it indexed, or a put of the
    // same bytes is acknowledged without a write. Each read and each write of a collection fails in turn; then every
    // block is put again on the same instance, and each must read back whole.
    let blocks: Vec<Vec<u8>> = (0..5).map(|i| block(i, 300 + 2000 * i)).collect();
    // Three blocks whose leases run out, then two put later whose leases still run at the collection; `fail` sets the
    // device's failing read or write for the collection and gives back how many reads and writes it made.
    let run = |fail: (Option<usize>, Option<usize>)| -> (bool, usize, usize) {
        let mut medium = Memory::new(160);
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        let mut cids: Vec<Cid> = blocks[..3].iter().map(|b| store.put(Codec::Raw, b).unwrap()).collect();
        store.set_time(LEASE_NS / 2);
        cids.extend(blocks[3..].iter().map(|b| store.put(Codec::Raw, b).unwrap()));
        store.set_time(LEASE_NS + 1);
        let device = store.device();
        (device.reads, device.written, device.fail_read_at, device.fail_write_at) = (0, 0, fail.0, fail.1);
        let cut = store.collect().is_err();
        let device = store.device();
        let counts = (device.reads, device.written);
        (device.fail_read_at, device.fail_write_at) = (None, None);
        for (b, cid) in blocks.iter().zip(&cids) {
            assert_eq!(store.put(Codec::Raw, b), Ok(*cid), "{:?}", fail);
            assert_eq!(get(&mut store, cid).as_deref(), Ok(&b[..]), "{:?}: a put acknowledged what is not there", fail);
        }
        (cut, counts.0, counts.1)
    };
    let (cut, reads, writes) = run((None, None));
    assert!(!cut && reads > 1 && writes > 3, "{} reads, {} writes", reads, writes);
    for k in 1..=reads { assert!(run((Some(k), None)).0, "read {} of {}", k, reads); }
    for k in 1..=writes { assert!(run((None, Some(k))).0, "write {} of {}", k, writes); }
}

#[test]
fn the_index_grows_with_the_medium() {
    // 251-STO-0013: a hash table of 56-byte slots; inserts and lookups do not shift the index, so 100 000 blocks go in,
    // are found again after a remount, and a collection that frees half of them leaves the rest found.
    assert_eq!(std::mem::size_of::<Entry>(), 56);
    assert_eq!((store::slots_for(1_000), store::slots_for(80_000), store::slots_for(1 << 40)), (4681, 11_428, 1 << 20));
    // The least is 4096 blocks, as the static index held before.
    let mut medium = Memory::new(64);
    let mut room = Room::new(store::slots_for(64));
    assert_eq!(mount(&mut medium, &mut room).unwrap().stats().capacity, 4096);
    let count = 100_000;
    let mut medium = Memory::new(count + 64);
    let mut room = Room::new(store::slots_for(count as u64 * 10));
    let blocks: Vec<Vec<u8>> = (0..count).map(|i| (i as u32).to_le_bytes().to_vec()).collect();
    let started = std::time::Instant::now();
    let cids: Vec<Cid> = {
        let mut store = mount(&mut medium, &mut room).unwrap();
        let cids: Vec<Cid> = blocks.iter().map(|b| store.put(Codec::Raw, b).unwrap()).collect();
        assert_eq!(store.stats().blocks as usize, count);
        cids
    };
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(store.stats().blocks as usize, count);
    assert!(cids.iter().all(|c| store.has(c)));
    assert!(!store.has(&Cid::raw(b"never stored")));
    // Half are put again later, so their leases still run at the collection.
    store.set_time(LEASE_NS / 2);
    for b in blocks.iter().step_by(2) { store.put(Codec::Raw, b).unwrap(); }
    store.set_time(LEASE_NS + 1);
    assert_eq!(store.collect().unwrap().blocks as usize, count / 2);
    for (i, (b, c)) in blocks.iter().zip(&cids).enumerate() {
        assert_eq!(store.has(c), i % 2 == 0, "block {}", i);
        if i % 2 == 0 && i % 997 == 0 { assert_eq!(get(&mut store, c).unwrap(), *b); }
    }
    assert!(started.elapsed().as_secs() < 60, "{:?}", started.elapsed());
}

#[test]
fn a_medium_past_two_tebibytes_is_refused() {
    // An index entry names a sector below 2^32.
    struct Huge;
    impl Device for Huge {
        fn sectors(&self) -> u64 { (1 << 32) + 1 }
        fn writable(&self) -> bool { true }
        fn read(&mut self, _: u64, _: &mut [u8]) -> bool { panic!("read before the size is checked") }
        fn write(&mut self, _: u64, _: &[u8]) -> bool { false }
        fn flush(&mut self) -> bool { true }
    }
    let mut room = Room::new(4);
    let mounted = Store::mount(Huge, &SEALER, &mut room.index, &mut room.heads, &mut room.pins, &mut room.holes, &mut room.buffer, &mut room.scratch, 0);
    assert_eq!(mounted.err(), Some(Error::TooLarge));
}

#[test]
fn removals_keep_every_other_block_found() {
    // Deletion without tombstones moves later entries back; a small index and many rounds of puts and collections make
    // runs that wrap around its end. A model says what must be found after each round.
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; rng };
    let mut medium = Memory::new(4096);
    let mut room = Room::new(48);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let mut held: HashMap<Cid, (Vec<u8>, u64)> = HashMap::new();
    let mut now = 0;
    for round in 0..200 {
        for _ in 0..(next() % 12) {
            let data = (next() % 300).to_le_bytes().repeat(1 + (next() % 3) as usize);
            match store.put(Codec::Raw, &data) {
                Ok(cid) => { held.insert(cid, (data, now)); }
                Err(Error::Full) => {}
                Err(e) => panic!("round {}: {:?}", round, e),
            }
        }
        now += LEASE_NS / 3;
        store.set_time(now);
        if round % 3 == 0 {
            store.collect().unwrap();
            held.retain(|_, (_, at)| now < *at + LEASE_NS);
        }
        for (cid, (data, _)) in &held { assert_eq!(get(&mut store, cid).as_ref(), Ok(data), "round {}", round); }
        assert_eq!(store.stats().blocks as usize, held.len(), "round {}", round);
    }
}

/// A medium in a file, for objects larger than memory.
struct FileMedium { file: std::fs::File, sectors: u64 }
impl Device for FileMedium {
    fn sectors(&self) -> u64 { self.sectors }
    fn writable(&self) -> bool { true }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool { use std::os::unix::fs::FileExt; self.file.read_exact_at(out, lba * SECTOR as u64).is_ok() }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool { use std::os::unix::fs::FileExt; self.file.write_all_at(data, lba * SECTOR as u64).is_ok() }
    fn flush(&mut self) -> bool { true }
}

// The resident set's peak of this process (Linux), in KiB.
fn peak_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l| l.starts_with("VmHWM:")).and_then(|l| l.split_whitespace().nth(1)?.parse().ok())).unwrap_or(0)
}

#[test]
#[ignore = "writes a 3 GiB file and takes minutes: run with --ignored, built with -O (MIND_STORE_FILE names the file)"]
fn a_three_gibibyte_object_fits() {
    // 251-STO-0010: an object of 3 GiB goes into a store on a file through dag::Builder, a chunk at a time, is found
    // whole after a remount and reads back in pieces; neither the writer nor the store holds it. The index is sized
    // for the object's blocks; the peak resident set is printed with it.
    const SIZE: u64 = 3 << 30;
    let path = std::env::var("MIND_STORE_FILE").unwrap_or_else(|_| std::env::temp_dir().join("mind-store-3g.img").to_string_lossy().into_owned());
    // An owner may retain three quarters of the medium (MC-4.11), so the medium is half again the object (a sparse file).
    let sectors = (SIZE + SIZE / 2) / SECTOR as u64;
    // No two chunks alike (splitmix64 of each 8-byte word), as a model's files are, so every block is stored once.
    let piece = |at: u64| -> u8 {
        let mut z = (at / 8).wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> (at % 8 * 8)) as u8
    };
    let blocks = SIZE.div_ceil(dag::CHUNK as u64) + SIZE.div_ceil(dag::CHUNK as u64 * 256) + 16;
    let mut room = Room::new((blocks as usize) * 8 / 7 + 64);
    let (root, written) = {
        let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path).unwrap();
        file.set_len(sectors * SECTOR as u64).unwrap();
        let mut store = Store::mount(FileMedium { file, sectors }, &SEALER, &mut room.index, &mut room.heads, &mut room.pins, &mut room.holes, &mut room.buffer, &mut room.scratch, 0).unwrap();
        let mut builder = Box::new(dag::Builder::new());
        let mut buffer = vec![0u8; 1 << 20];
        let mut at = 0u64;
        while at < SIZE {
            for (i, b) in buffer.iter_mut().enumerate() { *b = piece(at + i as u64); }
            builder.write(&mut store, &buffer).unwrap();
            at += buffer.len() as u64;
        }
        let (root, size) = builder.finish(&mut store).unwrap();
        assert_eq!(size, SIZE);
        store.publish(b"models/three-gib", 0, &root, OWNER).unwrap();
        (root, store.stats())
    };
    let file = std::fs::OpenOptions::new().read(true).write(true).open(&path).unwrap();
    let mut store = Store::mount(FileMedium { file, sectors }, &SEALER, &mut room.index, &mut room.heads, &mut room.pins, &mut room.holes, &mut room.buffer, &mut room.scratch, 0).unwrap();
    assert_eq!(store.resolve(b"models/three-gib").map(|(_, cid)| cid), Ok(root));
    let mut chunk = [0u8; dag::CHUNK];
    let mut out = vec![0u8; 100_000];
    for at in [0, 1 << 20, SIZE / 2 + 12345, SIZE - out.len() as u64] {
        assert_eq!(dag::read_at(&mut store, &root, at, &mut out, &mut chunk), Ok(out.len()));
        assert!(out.iter().enumerate().all(|(i, &b)| b == piece(at + i as u64)), "at {}", at);
    }
    let index_kib = room.index.len() * std::mem::size_of::<Entry>() / 1024;
    println!("3 GiB object: {} blocks, {} sectors of {}; index {} slots, {} KiB; peak resident set {} KiB", written.blocks, written.used, sectors, room.index.len(), index_kib, peak_kib());
    assert!(written.blocks as u64 >= SIZE / dag::CHUNK as u64, "the chunks were not all distinct");
    assert!(peak_kib() < (SIZE >> 10) / 8, "the process held {} KiB", peak_kib());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_partition_holding_only_the_superblock_mounts_and_gets_a_key() {
    // 307-STO-0020, 308-STO-0021: what scripts/make_usb_image.py writes first in the store's partition (store_superblock):
    // the magic, the layout, the sector size, key source 0 (none yet), the SHA-256 of those 16 bytes, zeros where the key
    // and its check go, and the SHA-256 of the 140 bytes before it.
    let mut first = Vec::from(&b"MIND-STO"[..]);
    first.extend_from_slice(&LAYOUT.to_le_bytes());
    first.extend_from_slice(&(SECTOR as u16).to_le_bytes());
    first.extend_from_slice(&[KEY_NONE, 0, 0, 0]);
    first.extend_from_slice(&sha256::digest(&first[..16]));
    first.resize(140, 0);
    first.extend_from_slice(&sha256::digest(&first[..140]));
    first.resize(SECTOR, 0);
    assert_eq!(&superblock(None)[..], &first[..]);
    // Read-only, it cannot get its key, and is not mounted.
    let mut medium = Memory::new(256);
    medium.data[..SECTOR].copy_from_slice(&first);
    medium.writable = false;
    assert_eq!(mount(&mut medium, &mut Room::new(64)).err(), Some(Error::ReadOnly));
    medium.writable = true;
    let mut room = Room::new(64);
    let store = mount(&mut medium, &mut room).unwrap();
    let stats = store.stats();
    assert_eq!((stats.blocks, stats.names, stats.used, stats.corrupt, stats.damaged), (0, 0, 1, 0, 0));
    assert_eq!(store.made(), Some(true));
    drop(store);
    // The superblock alone was written: it now holds the key, and the next mount makes none.
    assert_eq!((medium.written, medium.writes[0], medium.data[12]), (1, 1, KEY_CLEAR));
    let mut room = Room::new(64);
    assert_eq!(mount(&mut medium, &mut room).unwrap().made(), None);
}

#[test]
fn nothing_of_a_block_a_cid_or_a_name_is_in_the_clear() {
    let mut medium = Memory::new(256);
    let secret: Vec<u8> = b"a sentence nobody should read off the disk ".repeat(20);
    let (raw, root);
    {
        let mut room = Room::new(64);
        let mut store = mount(&mut medium, &mut room).unwrap();
        raw = store.put(Codec::Raw, &secret).unwrap();
        root = object(&mut store, 9, 20_000);
        store.publish(b"diary/2026-10-28", 0, &raw, OWNER).unwrap();
        store.pin(&root, OWNER).unwrap();
        let updates = [Update { name: b"diary/today", expected: 0, root: Some(root) }, Update { name: b"diary/2026-10-28", expected: 1, root: None }];
        store.commit(&updates, OWNER).unwrap();
    }
    let medium_has = |needle: &[u8]| medium.data.windows(needle.len()).any(|w| w == needle);
    for needle in [&secret[..16], &raw.to_bytes()[..], &root.to_bytes()[..], &raw.to_bytes()[4..20], b"diary", b"MIND-BLK", b"MIND-REF", b"MIND-PIN", b"MIND-TXN", b"MIND-DEL"] {
        assert!(!medium_has(needle), "{:?} is on the medium in the clear", String::from_utf8_lossy(needle));
    }
    assert!(medium_has(&block(9, 20_000)[..16]) == false);
    // Every record opens, and the store reads them all back.
    assert!(records(&medium).iter().all(|(_, r)| r.is_some()));
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!((get(&mut store, &raw).unwrap(), store.resolve(b"diary/today")), (secret, Ok((1, root))));
    assert_eq!(read_object(&mut store, &root), block(9, 20_000));
    assert_eq!((store.stats().corrupt, store.stats().damaged, store.stats().pins), (0, 0, 1));
}

#[test]
fn a_changed_or_moved_record_does_not_open() {
    let mut medium = Memory::new(128);
    let (a, b);
    {
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        a = store.put(Codec::Raw, b"first").unwrap();
        b = store.put(Codec::Raw, b"second").unwrap();
    }
    assert_eq!(find(&medium, b"MIND-BLK"), vec![1, 2]);
    // One byte of each part of a frame or of the sealed bytes: the record no longer opens.
    for byte in [8, 10, 13, 16, 30, FRAME, FRAME + 100, SECTOR - 1] {
        let mut changed = medium.clone();
        changed.flip(1, byte);
        assert_eq!(unseal(&changed.data[..SECTOR], 1, &changed.data[SECTOR..]), None, "byte {byte}");
        let mut room = Room::new(16);
        let mut store = mount(&mut changed, &mut room).unwrap();
        assert_eq!((get(&mut store, &a), get(&mut store, &b).is_ok()), (Err(Error::NotFound), true), "byte {byte}");
        assert_eq!(store.stats().corrupt + store.stats().damaged as u32, 1, "byte {byte}");
    }
    // A's record copied over b's place, whole and unchanged: it does not open there, and neither block is offered twice.
    let mut moved = medium.clone();
    moved.data.copy_within(SECTOR..2 * SECTOR, 2 * SECTOR);
    let mut room = Room::new(16);
    let mut store = mount(&mut moved, &mut room).unwrap();
    assert_eq!((get(&mut store, &a).unwrap(), get(&mut store, &b)), (b"first".to_vec(), Err(Error::NotFound)));
    assert_eq!(store.stats().corrupt, 1);
}

#[test]
fn a_wrong_key_is_refused_at_mount() {
    let mut medium = Memory::new(64);
    {
        let mut room = Room::new(8);
        let mut store = mount(&mut medium, &mut room).unwrap();
        store.put(Codec::Raw, b"kept").unwrap();
    }
    // Another key in the superblock, its digests made whole: it does not open the check, and nothing is read or written.
    let mut other = medium.clone();
    other.flip(0, 50);
    let check = sha256::digest(&other.data[..140]);
    other.data[140..172].copy_from_slice(&check);
    let before = other.data.clone();
    assert_eq!(mount(&mut other, &mut Room::new(8)).err(), Some(Error::Key));
    assert_eq!(other.data, before);
    // The check changed instead, or a key source this build does not know.
    let mut other = medium.clone();
    other.flip(0, 100);
    let check = sha256::digest(&other.data[..140]);
    other.data[140..172].copy_from_slice(&check);
    assert_eq!(mount(&mut other, &mut Room::new(8)).err(), Some(Error::Key));
    let mut other = medium.clone();
    other.data[12] = 7;
    let check = sha256::digest(&other.data[..16]);
    other.data[16..48].copy_from_slice(&check);
    let check = sha256::digest(&other.data[..140]);
    other.data[140..172].copy_from_slice(&check);
    assert_eq!(mount(&mut other, &mut Room::new(8)).err(), Some(Error::Key));
    // The key changed without its digest: a damaged superblock, left alone.
    let mut other = medium.clone();
    other.flip(0, 50);
    assert_eq!(mount(&mut other, &mut Room::new(8)).err(), Some(Error::Foreign));
}

#[test]
fn without_a_random_source_the_key_is_derived_and_said_so() {
    let mut medium = Memory::new(64);
    let mut room = Room::new(8);
    let mut store = mount_with(&mut medium, &mut room, &NO_RANDOM).unwrap();
    assert_eq!(store.made(), Some(false));
    let a = store.put(Codec::Raw, b"kept").unwrap();
    drop(store);
    let mut room = Room::new(8);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!((store.made(), get(&mut store, &a).unwrap()), (None, b"kept".to_vec()));
}

#[test]
fn hmac_sha256_matches_rfc_4231() {
    // Test cases 1 and 2: a key shorter than the block is padded with zeros, so padding it to 32 bytes changes nothing.
    let mut key = [0u8; 32];
    key[..20].fill(0x0b);
    let hex = |d: [u8; 32]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(hex(hmac_sha256(&key, &[b"Hi ", b"There"])), "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(b"Jefe");
    assert_eq!(hex(hmac_sha256(&key, &[b"what do ya want for nothing?"])), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
}

// The frame's domain of the record at `lba`.
fn frame_domain(medium: &Memory, lba: u64) -> u16 { let at = lba as usize * SECTOR; u16::from_le_bytes([medium.data[at + 10], medium.data[at + 11]]) }

// Every frame on the medium, by its first sector, with its domain.
fn frames(medium: &Memory) -> Vec<(u64, u16)> { records(medium).into_iter().map(|(lba, _)| (lba, frame_domain(medium, lba))).collect() }

#[test]
fn two_domains_open_only_under_their_own_keys() {
    let mut medium = Memory::new(512);
    let data = block(4, 3000);
    let (a, b, cid);
    {
        let mut room = Room::new(64);
        let mut store = mount(&mut medium, &mut room).unwrap();
        (a, b) = (store.domain_create(OWNER).unwrap(), store.domain_create(OWNER).unwrap());
        assert!(a != VOLUME && b != VOLUME && a != b);
        // The same bytes in the volume and in each domain are three records; a name in one is not in the others.
        cid = store.put(Codec::Raw, &data).unwrap();
        for (domain, version) in [(a, 1), (b, 1)] {
            store.set_domain(domain).unwrap();
            assert!(!store.has(&cid), "a domain sees no other domain's block");
            assert_eq!(store.put(Codec::Raw, &data), Ok(cid));
            assert_eq!(store.publish(b"x", 0, &cid, OWNER), Ok(version));
        }
        store.set_domain(b).unwrap();
        assert_eq!(store.publish(b"only-b", 0, &cid, OWNER), Ok(1));
        store.set_domain(a).unwrap();
        assert_eq!(store.resolve(b"only-b"), Err(Error::NotFound));
        store.set_domain(VOLUME).unwrap();
        assert_eq!(store.resolve(b"x"), Err(Error::NotFound));
        assert_eq!(store.stats().blocks, 3);
    }
    // Each domain's records are framed as its own, and none of them opens under the volume key.
    let domains: Vec<u16> = frames(&medium).iter().map(|f| f.1).collect();
    assert_eq!((domains.iter().filter(|&&d| d == a).count(), domains.iter().filter(|&&d| d == b).count()), (2, 3));
    assert!(records(&medium).iter().all(|(lba, r)| r.is_some() == [VOLUME, 0xFFFF].contains(&frame_domain(&medium, *lba))));
    // A remount opens every one of them under its own key.
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!((store.stats().blocks, store.stats().names, store.stats().corrupt), (3, 3, 0));
    assert_eq!(store.domains().map(|d| (d.id, d.owner, d.forgotten)).collect::<Vec<_>>(), vec![(a, OWNER, false), (b, OWNER, false)]);
    for domain in [VOLUME, a, b] {
        store.set_domain(domain).unwrap();
        assert_eq!(get(&mut store, &cid).unwrap(), data);
    }
    assert_eq!(store.resolve(b"only-b"), Ok((1, cid)));
}

#[test]
fn a_forgotten_domain_is_refused_then_collected() {
    let mut medium = Memory::new(512);
    let (a, b, kept);
    {
        let mut room = Room::new(64);
        let mut store = mount(&mut medium, &mut room).unwrap();
        (a, b) = (store.domain_create(OWNER).unwrap(), store.domain_create(OWNER).unwrap());
        store.set_domain(a).unwrap();
        let root = object(&mut store, 5, 40_000);
        store.publish(b"diary", 0, &root, OWNER).unwrap();
        store.pin(&root, OWNER).unwrap();
        store.set_domain(b).unwrap();
        kept = object(&mut store, 6, 20_000);
        store.publish(b"diary", 0, &kept, OWNER).unwrap();
        assert_eq!(store.domain_forget(a, OWNER + 1), Err(Error::Rights));
        let before = store.stats();
        store.domain_forget(a, OWNER).unwrap();
        // Gone at once: its names, pins and blocks, and any call in it.
        assert_eq!(store.set_domain(a), Err(Error::NotFound));
        assert_eq!(store.domain_forget(a, OWNER), Err(Error::NotFound));
        assert!(store.stats().blocks < before.blocks && store.stats().names == before.names - 1 && store.stats().pins == 0);
        assert_eq!(store.resolve(b"diary").map(|(_, r)| r), Ok(kept));
        assert_eq!(store.domains().map(|d| (d.id, d.forgotten)).collect::<Vec<_>>(), vec![(a, true), (b, false)]);
    }
    // On the medium its key record is gone; its tombstone and its records stay until a collection.
    let keys: Vec<u64> = frames(&medium).iter().filter(|f| f.1 == 0xFFFF).map(|f| f.0).collect();
    let opened: Vec<Vec<u8>> = keys.iter().map(|&lba| records(&medium).into_iter().find(|r| r.0 == lba).unwrap().1.unwrap()).collect();
    assert_eq!(opened.iter().filter(|r| r[10..12] == a.to_le_bytes()).map(|r| u16::from_le_bytes([r[12], r[13]])).collect::<Vec<_>>(), vec![2], "the tombstone alone");
    let a_records = frames(&medium).iter().filter(|f| f.1 == a).count();
    assert!(a_records > 3);
    let mut room = Room::new(64).at(LEASE_NS);
    {
        let mut store = mount(&mut medium, &mut room).unwrap();
        let forgotten = store.domains().find(|d| d.id == a).copied().unwrap();
        assert!(forgotten.forgotten && forgotten.records as usize == a_records);
        assert_eq!((store.stats().corrupt, store.stats().damaged), (0, 0), "a forgotten domain's records are neither corrupt nor damaged");
        assert_eq!(store.set_domain(a), Err(Error::NotFound));
        // The next collection frees them and the tombstone; the other domain is whole.
        let collected = store.collect().unwrap();
        assert!(collected.blocks as usize >= a_records);
        assert_eq!(store.domains().map(|d| d.id).collect::<Vec<_>>(), vec![b]);
        store.set_domain(b).unwrap();
        assert_eq!(read_object(&mut store, &kept), block(6, 20_000));
    }
    assert!(frames(&medium).iter().all(|f| f.1 != a), "nothing of the forgotten domain is left");
    let mut room = Room::new(64).at(LEASE_NS);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!((store.stats().corrupt, store.stats().damaged, store.domains().count()), (0, 0, 1));
    store.set_domain(b).unwrap();
    assert_eq!(store.resolve(b"diary").map(|(_, r)| r), Ok(kept));
}

#[test]
fn a_forget_stopped_before_its_key_record_was_erased_is_finished_by_the_next_mount() {
    let mut medium = Memory::new(256);
    let (a, end);
    {
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        a = store.domain_create(OWNER).unwrap();
        store.set_domain(a).unwrap();
        store.put(Codec::Raw, b"in a").unwrap();
        end = store.stats().used;
    }
    // A tombstone written, and the stop before the key record was erased.
    let mut tomb = vec![0u8; 84];
    tomb[..8].copy_from_slice(b"MIND-KEY");
    tomb[8..10].copy_from_slice(&LAYOUT.to_le_bytes());
    tomb[10..12].copy_from_slice(&a.to_le_bytes());
    tomb[12..14].copy_from_slice(&2u16.to_le_bytes());
    tomb[14..16].copy_from_slice(&OWNER.to_le_bytes());
    let check = sha256::digest(&tomb[..52]);
    tomb[52..].copy_from_slice(&check);
    plant_in(&mut medium, end, &tomb, 0xFFFF);
    let mut room = Room::new(16);
    let store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(store.domains().map(|d| (d.id, d.forgotten)).collect::<Vec<_>>(), vec![(a, true)]);
    drop(store);
    let live = records(&medium).into_iter().filter_map(|(_, r)| r).filter(|r| r.starts_with(b"MIND-KEY") && r[12..14] == 1u16.to_le_bytes()).count();
    assert_eq!(live, 0, "the mount erased the key record");
}

#[test]
fn a_domain_needs_a_random_key_and_an_owner() {
    let mut medium = Memory::new(128);
    {
        let mut room = Room::new(8);
        let mut store = mount_with(&mut medium, &mut room, &NO_RANDOM).unwrap();
        assert_eq!(store.domain_create(OWNER), Err(Error::NoRandom));
        assert_eq!(store.domains().count(), 0);
    }
    let mut room = Room::new(8);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let a = store.domain_create(OWNER).unwrap();
    assert_eq!(store.set_domain(a + 1), Err(Error::NotFound));
    assert_eq!(store.set_domain(0xFFFF), Err(Error::NotFound), "the key records' own frame domain is no domain");
    assert_eq!(store.domain_forget(a, OWNER + 1), Err(Error::Rights));
    drop(store);
    medium.writable = false;
    let mut room = Room::new(8);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!((store.domain_create(OWNER), store.domain_forget(a, OWNER)), (Err(Error::ReadOnly), Err(Error::ReadOnly)));
}

#[test]
fn a_damaged_key_record_keeps_its_domains_records() {
    let mut medium = Memory::new(256);
    let a;
    {
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        a = store.domain_create(OWNER).unwrap();
        store.set_domain(a).unwrap();
        let cid = store.put(Codec::Raw, &block(2, 1000)).unwrap();
        store.publish(b"kept", 0, &cid, OWNER).unwrap();
    }
    let key = frames(&medium).iter().find(|f| f.1 == 0xFFFF).unwrap().0;
    medium.flip(key, 200);
    let records_of_a = frames(&medium).iter().filter(|f| f.1 == a).count();
    let mut room = Room::new(16).at(2 * LEASE_NS);
    let mut store = mount(&mut medium, &mut room).unwrap();
    // Without its key the domain is unknown: its records are counted corrupt, and no collection frees them, as the
    // key record may only be damaged.
    assert_eq!(store.stats().corrupt as usize, 1 + records_of_a);
    assert_eq!(store.set_domain(a), Err(Error::NotFound));
    store.collect().unwrap();
    drop(store);
    assert_eq!(frames(&medium).iter().filter(|f| f.1 == a).count(), records_of_a);
}


#[test]
fn a_domains_records_before_its_key_record_are_found() {
    // Volume blocks fill the first sectors and go at a collection; the domain's key record comes after them, and its
    // blocks then go into the room before it. 10 wait for the key record at a mount; 70 are more than wait, so the
    // mount scans once more.
    for count in [10usize, 70] {
        let mut medium = Memory::new(512);
        let (a, cids);
        {
            let mut room = Room::new(256);
            let mut store = mount(&mut medium, &mut room).unwrap();
            for i in 0..count { store.put(Codec::Raw, &(i as u32).to_le_bytes()).unwrap(); }
            a = store.domain_create(OWNER).unwrap();
            store.set_time(LEASE_NS);
            store.collect().unwrap();
            store.set_domain(a).unwrap();
            cids = (0..count).map(|i| store.put(Codec::Raw, &(1000 + i as u32).to_le_bytes()).unwrap()).collect::<Vec<_>>();
            store.publish(b"last", 0, &cids[count - 1], OWNER).unwrap();
        }
        let key = frames(&medium).iter().find(|f| f.1 == 0xFFFF).unwrap().0;
        assert!(frames(&medium).iter().filter(|f| f.1 == a && f.0 < key).count() >= count, "{count}: the domain's blocks lie before its key record");
        let mut room = Room::new(256);
        let mut store = mount(&mut medium, &mut room).unwrap();
        assert_eq!((store.stats().blocks as usize, store.stats().corrupt), (count, 0), "{count}");
        store.set_domain(a).unwrap();
        assert!(cids.iter().all(|c| store.has(c)), "{count}");
        assert_eq!(store.resolve(b"last"), Ok((1, cids[count - 1])));
    }
}

// What a scrub of `store`'s medium finds in steps of `budget` sectors: its report, the records it reported (sector and
// block) and the calls it took.
fn scrub_all(store: &mut Store<&mut Memory>, budget: u64) -> (Scrubbed, Vec<(u64, Option<Cid>)>, usize) {
    let (mut found, mut calls) = (Vec::new(), 0);
    loop {
        calls += 1;
        if let Some(report) = store.scrub(budget, &mut |lba, cid| found.push((lba, cid))).unwrap() { return (report, found, calls); }
    }
}

// The domains' key records on `medium`: the sector of each and the domain its record names.
fn key_records(medium: &Memory) -> Vec<(u64, u16)> {
    records(medium).into_iter().filter(|(lba, r)| frame_domain(medium, *lba) == 0xFFFF && r.is_some()).map(|(lba, r)| (lba, u16::from_le_bytes([r.as_ref().unwrap()[10], r.as_ref().unwrap()[11]]))).collect()
}

#[test]
fn a_scrub_finds_damage_after_mounting_and_writes_nothing() {
    let mut medium = Memory::new(512);
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let cids: Vec<Cid> = (0..4).map(|i| store.put(Codec::Raw, &block(i, 1500)).unwrap()).collect();
    store.publish(b"kept", 0, &cids[3], OWNER).unwrap();
    let a = store.domain_create(OWNER).unwrap();
    store.set_domain(a).unwrap();
    store.put(Codec::Raw, &block(9, 700)).unwrap();
    store.set_domain(VOLUME).unwrap();
    let all = records(store.device()).len() as u32;
    assert_eq!(scrub_all(&mut store, u64::MAX), (Scrubbed { records: all, corrupt: 0, damaged: 0 }, vec![], 1));
    // After mounting: a byte of the second block's data, of the name record and of the key record changes, and a
    // sector after the log is no longer blank.
    let (name, key) = (find(store.device(), b"MIND-REF")[0], key_records(store.device())[0].0);
    let (second, used) = (1 + record_sectors(1500) as u64, store.stats().used);
    let m = store.device();
    m.flip(second + 1, 100);
    m.flip(name, 100);
    m.flip(key, 100);
    m.data[(used as usize + 3) * SECTOR] = 0xAB;
    let (writes, flushes) = (m.writes.clone(), m.flushes);
    let (report, mut found, _) = scrub_all(&mut store, u64::MAX);
    assert_eq!(report, Scrubbed { records: all, corrupt: 3, damaged: 1 });
    found.sort();
    let mut expected = vec![(second, Some(cids[1])), (name, None), (key, None)];
    expected.sort();
    assert_eq!(found, expected, "each reported with its sector, a block with its CID");
    // In steps of a sector it takes many calls, and finds the same.
    let (stepped, mut again, calls) = scrub_all(&mut store, 1);
    again.sort();
    assert_eq!((stepped, again), (report, expected));
    assert!(calls > 10);
    let m = store.device();
    assert_eq!((&m.writes, m.flushes), (&writes, flushes), "a scrub writes nothing");
}

#[test]
fn a_scrub_counts_a_domains_records_as_a_mount_does() {
    let mut medium = Memory::new(512);
    let (a, b);
    {
        let mut room = Room::new(64);
        let mut store = mount(&mut medium, &mut room).unwrap();
        (a, b) = (store.domain_create(OWNER).unwrap(), store.domain_create(OWNER).unwrap());
        for d in [a, b] { store.set_domain(d).unwrap(); object(&mut store, d as usize, 20_000); }
        store.domain_forget(a, OWNER).unwrap();
        // A forgotten domain's records no longer open, and are not corrupt: the next collection frees them.
        let all = records(store.device()).len() as u32;
        assert!(frames(store.device()).iter().filter(|f| f.1 == a).count() > 1);
        assert_eq!(scrub_all(&mut store, 64).0, Scrubbed { records: all, corrupt: 0, damaged: 0 });
    }
    // Without b's key record its domain is unknown: a mount and a scrub count its records corrupt, and the key record.
    let key = key_records(&medium).into_iter().find(|k| k.1 == b).unwrap().0;
    medium.flip(key, 200);
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let corrupt = store.stats().corrupt;
    assert_eq!(corrupt as usize, 1 + frames(store.device()).iter().filter(|f| f.1 == b).count());
    assert_eq!(scrub_all(&mut store, u64::MAX).0.corrupt, corrupt);
}

#[test]
fn a_record_written_around_the_scrub_is_not_read_as_damaged() {
    let mut medium = Memory::new(256);
    let mut room = Room::new(64);
    let mut store = mount(&mut medium, &mut room).unwrap();
    // The first step reads a window of blank sectors and stops after it, inside the run the next puts go into.
    assert_eq!(store.scrub(1, &mut |_, _| panic!("nothing is corrupt")).unwrap(), None);
    assert!(store.scrubbing());
    for i in 0..6 { store.put(Codec::Raw, &block(i, 3000)).unwrap(); }
    // One record covers the scrub's place: the scrub goes on after it, reading none of it as damaged.
    let (report, found, _) = scrub_all(&mut store, u64::MAX);
    assert_eq!((report.corrupt, report.damaged, found), (0, 0, vec![]));
    assert!(report.records < 6);
    assert!(!store.scrubbing());
    // A pass dropped in the middle: the next starts at the first record.
    assert_eq!(store.scrub(1, &mut |_, _| panic!("nothing is corrupt")).unwrap(), None);
    store.restart_scrub();
    assert_eq!(scrub_all(&mut store, u64::MAX).0, Scrubbed { records: 6, corrupt: 0, damaged: 0 });
}
