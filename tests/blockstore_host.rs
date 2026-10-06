//! Host tests of the block store's layout and logic (blockstore/src/store.rs, issue 300-STO-0002; MC-4.2, 4.8):
//! put and get by CID, blocks found again after a remount, a flipped byte detected and never returned, no sector
//! written twice, defined refusals for a full medium, a full index, a foreign or unknown medium and a read-only one;
//! the clients' rights by badge (libmind/src/blockstore.rs, issue 300-STO-0004).
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[path = "../libmind/src/cid.rs"]
mod cid;
#[path = "../blockstore/src/store.rs"]
mod store;
#[path = "../libmind/src/blockstore.rs"]
mod rights;

use cid::Cid;
use std::collections::HashMap;
use store::{record_sectors, Device, Entry, Error, Stats, Store, BLOCK_MAX, BUFFER, HEADER, SECTOR};

/// A medium in memory that counts how often each sector was written.
#[derive(Clone)]
struct Memory { data: Vec<u8>, writes: Vec<u32>, writable: bool, fail_write: bool, flushes: usize }
impl Memory {
    fn new(sectors: usize) -> Self { Self { data: vec![0; sectors * SECTOR], writes: vec![0; sectors], writable: true, fail_write: false, flushes: 0 } }
    fn flip(&mut self, sector: u64, byte: usize) { self.data[sector as usize * SECTOR + byte] ^= 0x10; }
}
impl Device for &mut Memory {
    fn sectors(&self) -> u64 { self.writes.len() as u64 }
    fn writable(&self) -> bool { self.writable }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool {
        assert!(out.len() % SECTOR == 0 && out.len() <= BUFFER, "reads are whole sectors, at most a record");
        let at = lba as usize * SECTOR;
        out.copy_from_slice(&self.data[at..at + out.len()]);
        true
    }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool {
        assert!(self.writable && data.len() % SECTOR == 0 && data.len() <= BUFFER);
        if self.fail_write { return false; }
        let at = lba as usize * SECTOR;
        self.data[at..at + data.len()].copy_from_slice(data);
        for s in 0..data.len() / SECTOR { self.writes[lba as usize + s] += 1; }
        true
    }
    fn flush(&mut self) -> bool { self.flushes += 1; true }
}

struct Room { index: Vec<Entry>, buffer: Box<[u8; BUFFER]> }
impl Room { fn new(capacity: usize) -> Self { Self { index: vec![Entry::EMPTY; capacity], buffer: Box::new([0; BUFFER]) } } }

fn mount<'a>(medium: &'a mut Memory, room: &'a mut Room) -> Result<Store<'a, &'a mut Memory>, Error> {
    Store::mount(medium, &mut room.index, &mut room.buffer)
}

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
    let a = store.put(b"hello world").unwrap();
    assert_eq!(a.to_string(), "bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e");
    let empty = store.put(b"").unwrap();
    let big = block(1, BLOCK_MAX);
    let b = store.put(&big).unwrap();
    assert_eq!(get(&mut store, &a).unwrap(), b"hello world");
    assert_eq!(get(&mut store, &empty).unwrap(), b"");
    assert_eq!(get(&mut store, &b).unwrap(), big);
    assert!(store.has(&a) && store.has(&b) && !store.has(&Cid::raw(b"absent")));
    assert_eq!(get(&mut store, &Cid::raw(b"absent")), Err(Error::NotFound));
    assert_eq!(store.put(&block(2, BLOCK_MAX + 1)), Err(Error::TooLarge));
    let mut small = [0u8; 4];
    assert_eq!(store.get(&a, &mut small), Err(Error::TooLarge));
    let used = 1 + record_sectors(11) + record_sectors(0) + record_sectors(BLOCK_MAX);
    assert_eq!(store.stats(), Stats { blocks: 3, bytes: 11 + BLOCK_MAX as u64, used: used as u64, sectors: 256, corrupt: 0, damaged: 0, capacity: 64 });
}

#[test]
fn the_same_bytes_are_stored_once() {
    let mut medium = Memory::new(64);
    let mut room = Room::new(8);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let first = store.put(&block(3, 1000)).unwrap();
    let used = store.stats().used;
    assert_eq!(store.put(&block(3, 1000)), Ok(first));
    assert_eq!((store.stats().blocks, store.stats().used), (1, used));
}

#[test]
fn a_put_returns_after_the_flush() {
    let mut medium = Memory::new(64);
    let mut room = Room::new(8);
    {
        let mut store = mount(&mut medium, &mut room).unwrap();
        store.put(b"durable").unwrap();
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
        for i in 0..20 { cids.push((store.put(&block(i, i * 700)).unwrap(), block(i, i * 700))); }
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
        a = store.put(&block(1, 2000)).unwrap();
        b = store.put(&block(2, 2000)).unwrap();
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
        assert_eq!(store.put(&block(1, 2000)), Ok(a));
        assert_eq!(get(&mut store, &a).unwrap(), block(1, 2000));
    }
    assert!(medium.writes.iter().all(|&w| w <= 1), "no sector is written twice");
}

#[test]
fn a_block_damaged_after_mounting_is_refused_when_read() {
    let mut medium = Memory::new(128);
    let mut room = Room::new(16);
    let mut store = mount(&mut medium, &mut room).unwrap();
    let a = store.put(&block(5, 600)).unwrap();
    // The last byte of the block, in its second sector.
    let last = HEADER + 600 - 1;
    store.device().flip(1 + (last / SECTOR) as u64, last % SECTOR);
    assert_eq!(get(&mut store, &a), Err(Error::Corrupt));
    assert!(!store.has(&a));
    assert_eq!(store.stats().corrupt, 1);
    assert_eq!(get(&mut store, &a), Err(Error::NotFound));
}

#[test]
fn a_damaged_header_loses_its_record_only() {
    let mut medium = Memory::new(128);
    let mut cids = Vec::new();
    {
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        for i in 0..4 { cids.push(store.put(&block(i, 1500)).unwrap()); }
    }
    // The second record's header: its CID.
    medium.flip(1 + record_sectors(1500) as u64, 20);
    let mut room = Room::new(16);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(get(&mut store, &cids[1]), Err(Error::NotFound));
    for i in [0, 2, 3] { assert_eq!(get(&mut store, &cids[i]).unwrap(), block(i, 1500)); }
    let stats = store.stats();
    assert_eq!((stats.blocks, stats.damaged), (3, record_sectors(1500) as u64));
    // New blocks go after the whole log.
    assert_eq!(stats.used, 1 + 4 * record_sectors(1500) as u64);
    store.put(b"after").unwrap();
    assert!(store.device().writes.iter().all(|&w| w <= 1));
}

#[test]
fn a_torn_write_is_a_corrupt_record_and_the_log_goes_on() {
    let mut medium = Memory::new(128);
    let a;
    {
        let mut room = Room::new(16);
        let mut store = mount(&mut medium, &mut room).unwrap();
        store.put(&block(1, 3000)).unwrap();
        a = store.put(&block(2, 3000)).unwrap();
    }
    // Power lost after the header and the first sector of a's record: its other sectors are blank.
    let start = 1 + record_sectors(3000);
    for s in start + 1..start + record_sectors(3000) { medium.data[s * SECTOR..(s + 1) * SECTOR].fill(0); }
    let mut room = Room::new(16);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(get(&mut store, &a), Err(Error::NotFound));
    assert_eq!((store.stats().blocks, store.stats().corrupt), (1, 1));
    assert_eq!(store.stats().used, (1 + 2 * record_sectors(3000)) as u64);
    assert_eq!(store.put(&block(2, 3000)), Ok(a));
    assert_eq!(get(&mut store, &a).unwrap(), block(2, 3000));
}

#[test]
fn a_full_medium_or_index_refuses_a_put() {
    let mut medium = Memory::new(1 + 2 * record_sectors(4000) + 3);
    let mut room = Room::new(8);
    let mut store = mount(&mut medium, &mut room).unwrap();
    store.put(&block(1, 4000)).unwrap();
    store.put(&block(2, 4000)).unwrap();
    let before = store.stats();
    assert_eq!(store.put(&block(3, 4000)), Err(Error::Full));
    assert_eq!(store.stats(), before);
    // What still fits is taken.
    store.put(&block(4, SECTOR * 3 - HEADER)).unwrap();
    assert_eq!(store.put(b"x"), Err(Error::Full));
    assert_eq!(get(&mut store, &Cid::raw(&block(1, 4000))).unwrap(), block(1, 4000));

    let mut medium = Memory::new(64);
    let mut room = Room::new(2);
    let mut store = mount(&mut medium, &mut room).unwrap();
    store.put(b"a").unwrap();
    store.put(b"b").unwrap();
    assert_eq!(store.put(b"c"), Err(Error::Full));
    assert_eq!(store.put(b"a").map(|c| c == Cid::raw(b"a")), Ok(true));
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
    let mut other = medium.clone();
    other.data[8] = 2;
    let check = sha256::digest(&other.data[..16]);
    other.data[16..48].copy_from_slice(&check);
    assert_eq!(mount(&mut other, &mut Room::new(4)).err(), Some(Error::Layout));
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
        a = store.put(b"kept").unwrap();
    }
    medium.writable = false;
    let mut room = Room::new(4);
    let mut store = mount(&mut medium, &mut room).unwrap();
    assert_eq!(get(&mut store, &a).unwrap(), b"kept");
    assert_eq!(store.put(b"new"), Err(Error::ReadOnly));
    assert_eq!(store.put(b"kept"), Ok(a));
}

#[test]
fn a_failed_write_is_not_offered_and_its_sectors_are_not_reused() {
    let mut medium = Memory::new(64);
    let mut room = Room::new(4);
    let mut store = mount(&mut medium, &mut room).unwrap();
    store.device().fail_write = true;
    assert_eq!(store.put(&block(1, 700)), Err(Error::Device));
    assert!(!store.has(&Cid::raw(&block(1, 700))));
    store.device().fail_write = false;
    let used = store.stats().used;
    assert_eq!(used, 1 + record_sectors(700) as u64);
    let a = store.put(&block(1, 700)).unwrap();
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
                    match store.put(&data) {
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
    use rights::{allowed, Operation::*, BADGE_GET, BADGE_PUT};
    // (badge, put, get, has, stat)
    for (badge, put, get, has, stat) in [
        (0, false, false, false, false),
        (BADGE_GET, false, true, true, true),
        (BADGE_PUT, true, false, false, true),
        (BADGE_GET | BADGE_PUT, true, true, true, true),
        // Bits of rights this version does not know grant nothing.
        (4, false, false, false, false),
        (0xfffc, false, false, false, false),
        (0xffff, true, true, true, true),
    ] {
        assert_eq!([allowed(badge, Put), allowed(badge, Get), allowed(badge, Has), allowed(badge, Stat)], [put, get, has, stat], "badge {badge:#x}");
    }
}
