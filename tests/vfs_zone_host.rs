//! Host tests of vfs_server's zones (vfs_server/src/zone.rs) and the in-place write of a boot record (351-UPD-0008):
//! what each badge may change below the boot root, the updater's slots and records, and that a record write changes
//! its one sector and nothing else on the volume.
#![allow(dead_code)]
extern crate alloc;
#[path = "../vfs_server/src/fat.rs"]
mod fat;
#[path = "../vfs_server/src/zone.rs"]
mod zone;

// The badges zone.rs reads from libmind (checked against libmind/src/fs.rs below).
mod mind {
    pub mod fs {
        pub const BADGE_USER: u16 = 1;
        pub const BADGE_KEYSTORE: u16 = 2;
        pub const BADGE_NETPOLICY: u16 = 3;
        pub const BADGE_UPDATE: u16 = 4;
        pub const BADGE_READER: u16 = 5;
    }
}

use fat::{Error, Sectors, Volume, SECTOR};
use mind::fs::{BADGE_KEYSTORE, BADGE_NETPOLICY, BADGE_READER, BADGE_UPDATE, BADGE_USER};
use zone::{Zone, RECORD};

const STAMP: u32 = ((2026 - 1980) << 9 | 10 << 5 | 9) << 16 | (12 << 11 | 34 << 5 | 28);
const APP: u16 = 0;

struct Image { data: Vec<u8> }
impl Sectors for Image {
    fn read(&mut self, lba: u32, out: &mut [u8; SECTOR]) -> bool {
        let at = lba as usize * SECTOR;
        match self.data.get(at..at + SECTOR) { Some(s) => { out.copy_from_slice(s); true } None => false }
    }
    fn write(&mut self, lba: u32, data: &[u8; SECTOR]) -> bool {
        let at = lba as usize * SECTOR;
        match self.data.get_mut(at..at + SECTOR) { Some(s) => { s.copy_from_slice(data); true } None => false }
    }
    fn flush(&mut self) -> bool { true }
    fn sectors(&self) -> u64 { (self.data.len() / SECTOR) as u64 }
    fn writable(&self) -> bool { true }
}

// The zone of `path` from the boot root a client with `badge` gets (the user's badge: the writable boot root).
fn zone(path: &str, badge: u16, inactive: Option<&str>) -> Zone {
    let root = if badge == BADGE_USER { Zone::BootRoot } else { Zone::BootReadOnly };
    path.split('/').fold(root, |zone, name| zone.below(name, badge, inactive))
}

#[test]
fn the_badges_are_libminds() {
    let source = include_str!("../libmind/src/fs.rs");
    for (name, value) in [("BADGE_USER", BADGE_USER), ("BADGE_KEYSTORE", BADGE_KEYSTORE), ("BADGE_NETPOLICY", BADGE_NETPOLICY), ("BADGE_UPDATE", BADGE_UPDATE), ("BADGE_READER", BADGE_READER)] {
        assert!(source.contains(&format!("pub const {}: u16 = {};", name, value)), "{} is not {} in libmind", name, value);
    }
}

#[test]
fn the_updater_fills_the_slot_that_did_not_boot_and_writes_records() {
    let b = Some("B");
    assert_eq!(zone("MIND", BADGE_UPDATE, b), Zone::Slots);
    assert_eq!(zone("MIND/B", BADGE_UPDATE, b), Zone::Writable);
    assert_eq!(zone("mind/b/kernel.elf", BADGE_UPDATE, b), Zone::Writable);
    assert_eq!(zone("MIND/B/deeper/x", BADGE_UPDATE, b), Zone::Writable);
    assert_eq!(zone("MIND/BOOT0", BADGE_UPDATE, b), Zone::Record);
    assert_eq!(zone("MIND/boot1", BADGE_UPDATE, b), Zone::Record);
    // The running slot, anything else in MIND, the bootloader, the root's files and data/ stay read-only.
    for path in ["MIND/A", "MIND/A/kernel.elf", "MIND/BOOT2", "MIND/C", "MIND/BOOT0/x", "EFI/BOOT/BOOTX64.EFI", "kernel.elf", "data", "data/x"] {
        assert_eq!(zone(path, BADGE_UPDATE, b), Zone::ReadOnly, "{}", path);
    }
    // Private directories stay hidden from it.
    assert_eq!(zone("system/keystore", BADGE_UPDATE, b), Zone::Hidden);
    // Booted from slot B, the zone is slot A.
    assert_eq!(zone("MIND/A/kernel.elf", BADGE_UPDATE, Some("A")), Zone::Writable);
    assert_eq!(zone("MIND/B/kernel.elf", BADGE_UPDATE, Some("A")), Zone::ReadOnly);
    // Booted from the root, there is no zone.
    for path in ["MIND", "MIND/A", "MIND/B", "MIND/BOOT0"] {
        assert_eq!(zone(path, BADGE_UPDATE, None), Zone::ReadOnly, "{}", path);
    }
}

#[test]
fn other_badges_see_the_slots_read_only() {
    for badge in [APP, BADGE_USER, BADGE_KEYSTORE, BADGE_NETPOLICY, BADGE_READER, 0x100] {
        for path in ["MIND", "MIND/B", "MIND/B/kernel.elf", "MIND/BOOT0"] {
            assert_eq!(zone(path, badge, Some("B")), Zone::ReadOnly, "{} {}", badge, path);
        }
    }
    // What was there before: the user writes data/, a service its own private directory only.
    assert_eq!(zone("data/notes", BADGE_USER, Some("B")), Zone::Writable);
    assert_eq!(zone("data", APP, Some("B")), Zone::ReadOnly);
    // A reader (init's badge for services' and programs' clients) sees what an unbadged client sees.
    for path in ["data", "EFI/BOOT", "kernel.elf"] { assert_eq!(zone(path, BADGE_READER, Some("B")), Zone::ReadOnly, "{}", path); }
    for path in ["system/keystore", "system/netpolicy"] { assert_eq!(zone(path, BADGE_READER, None), Zone::Hidden, "{}", path); }
    assert_eq!(zone("system/keystore/x", BADGE_KEYSTORE, None), Zone::Writable);
    assert_eq!(zone("system/netpolicy", BADGE_KEYSTORE, None), Zone::Hidden);
    assert_eq!(zone("system/netpolicy", BADGE_NETPOLICY, None), Zone::Writable);
}

#[test]
fn a_record_is_written_in_its_one_sector() {
    let mut image = Image { data: vec![0u8; 8 << 20] };
    fat::format(&mut image, "MIND", STAMP).unwrap();
    let mut v = Volume::mount(image).ok().unwrap();
    let root = v.root();
    let mind = v.create(&root, "MIND", true, STAMP).unwrap();
    let mut record = v.create(&mind, "BOOT0", false, STAMP).unwrap();
    v.write(&mut record, 0, &[0u8; RECORD as usize], STAMP).unwrap();
    v.flush().unwrap();
    let before = v.disk.data.clone();
    let node = v.find(&mind, "BOOT0").unwrap().node;
    let new = [0x5Au8; RECORD as usize];
    v.overwrite(&node, 0, &new).unwrap();
    v.flush().unwrap();
    // One sector differs: the record's; the directory entry, the FAT and its clean flag are as they were.
    let changed: Vec<usize> = (0..before.len() / SECTOR).filter(|&s| before[s * SECTOR..][..SECTOR] != v.disk.data[s * SECTOR..][..SECTOR]).collect();
    assert_eq!(changed.len(), 1, "{:?}", changed);
    assert_eq!(v.disk.data[changed[0] * SECTOR..][..SECTOR], new);
    let after = v.find(&mind, "BOOT0").unwrap().node;
    assert_eq!((after.size, after.modified, after.cluster), (node.size, node.modified, node.cluster));
    let mut back = [0u8; RECORD as usize];
    assert_eq!(v.read(&after, 0, &mut back), Ok(RECORD as usize));
    assert_eq!(back, new);
    // Never past its end, never a directory.
    assert_eq!(v.overwrite(&node, 1, &new), Err(Error::Invalid));
    assert_eq!(v.overwrite(&node, 0, &[0u8; RECORD as usize + 1]), Err(Error::Invalid));
    assert_eq!(v.overwrite(&mind, 0, &new), Err(Error::IsDirectory));
}
