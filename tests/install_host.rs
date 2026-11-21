//! Host tests of the installer's writer (vfs_server/src/install.rs, 352-UPD-0048): a boot volume made by `mkfs.fat` and
//! mtools, as make_iso.py makes the live one, installed onto a blank disk image. The table is read by libmind's own
//! reader, the ESP and MIND LOG are checked with `fsck.fat -n` and read back with mtools, and `store.txt` names the
//! blank store partition. A disk too small, too large or read-only is refused untouched; a write cut short at any step,
//! or a sector the medium did not keep, leaves no ESP's entry. Skipped if the tools are missing.
#![allow(dead_code)]
extern crate alloc;
#[path = "../vfs_server/src/fat.rs"]
mod fat;
#[path = "../vfs_server/src/install.rs"]
mod install;
#[path = "../libmind/src/partition.rs"]
mod partition;

use fat::{Sectors, SECTOR};
use install::{Layout, Step};
use std::path::{Path, PathBuf};
use std::process::Command;

const MIB: usize = 1 << 20;
const SIGNATURE: u32 = 0x4D49_4E44;
const STAMP: u32 = (45 << 25 | 11 << 21 | 21 << 16) | (10 << 11); // 2025-11-21 10:00

// A disk in memory: writes that fail after `fail_after` sectors, and a sector whose writes it does not keep.
struct Image { data: Vec<u8>, writable: bool, writes: usize, fail_after: Option<usize>, lost: Option<u32> }
impl Image {
    fn blank(mib: usize) -> Self { Image { data: vec![0; mib * MIB], writable: true, writes: 0, fail_after: None, lost: None } }
    fn sector(&self, lba: u32) -> &[u8] { &self.data[lba as usize * SECTOR..(lba as usize + 1) * SECTOR] }
    fn part(&self, (first, count): (u32, u32)) -> &[u8] { &self.data[first as usize * SECTOR..(first + count) as usize * SECTOR] }
}
impl Sectors for Image {
    fn read(&mut self, lba: u32, out: &mut [u8; SECTOR]) -> bool {
        let at = lba as usize * SECTOR;
        match self.data.get(at..at + SECTOR) { Some(s) => { out.copy_from_slice(s); true } None => false }
    }
    fn write(&mut self, lba: u32, data: &[u8; SECTOR]) -> bool {
        if !self.writable || self.fail_after.is_some_and(|n| self.writes >= n) { return false; }
        self.writes += 1;
        if self.lost == Some(lba) { return true; }
        let at = lba as usize * SECTOR;
        match self.data.get_mut(at..at + SECTOR) { Some(s) => { s.copy_from_slice(data); true } None => false }
    }
    fn flush(&mut self) -> bool { true }
    fn sectors(&self) -> u64 { (self.data.len() / SECTOR) as u64 }
    fn writable(&self) -> bool { self.writable }
}

// A disk of `sectors` that holds nothing and must never be written.
struct Huge(u64);
impl Sectors for Huge {
    fn read(&mut self, _: u32, out: &mut [u8; SECTOR]) -> bool { out.fill(0); true }
    fn write(&mut self, _: u32, _: &[u8; SECTOR]) -> bool { panic!("written") }
    fn flush(&mut self) -> bool { true }
    fn sectors(&self) -> u64 { self.0 }
    fn writable(&self) -> bool { true }
}

fn tools() -> bool {
    let ok = ["mkfs.fat", "fsck.fat", "mtype", "mdir", "mcopy", "minfo"].iter().all(|t| Command::new("sh").args(["-c", &format!("command -v {}", t)]).output().is_ok_and(|o| o.status.success()));
    if !ok { eprintln!("SKIP: mkfs.fat, fsck.fat and mtools are needed"); }
    ok
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mind-install-{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn mtools(program: &str, image: &Path, args: &[&str]) -> Vec<u8> {
    let out = Command::new(program).arg("-i").arg(image).args(args).env("MTOOLS_SKIP_CHECK", "1").output().unwrap();
    assert!(out.status.success(), "{} {:?}: {}", program, args, String::from_utf8_lossy(&out.stderr));
    out.stdout
}

fn kernel() -> Vec<u8> { (0..300_000u32).map(|i| i.wrapping_mul(2654435761).rotate_left(5) as u8).collect() }

// The live volume as make_iso.py makes it: FAT16 of `mib`, hidden sectors 2048, labelled MIND CORE, with slot A's files.
fn source(dir: &Path, mib: usize) -> Image {
    let path = dir.join("volume.img");
    std::fs::write(&path, vec![0u8; mib * MIB]).unwrap();
    let out = Command::new("mkfs.fat").args(["--invariant", "-F", "16", "-i", "4d494e44", "-n", "MIND CORE", "-h", "2048"]).arg(&path).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let files = dir.join("files");
    std::fs::create_dir_all(files.join("EFI/BOOT")).unwrap();
    std::fs::create_dir_all(files.join("MIND/A")).unwrap();
    std::fs::write(files.join("EFI/BOOT/BOOTX64.EFI"), b"MZ loader").unwrap();
    std::fs::write(files.join("MIND/A/kernel.elf"), kernel()).unwrap();
    std::fs::write(files.join("MIND/BOOT0"), b"slot A, confirmed\n").unwrap();
    for name in ["EFI", "MIND"] { mtools("mcopy", &path, &["-s", "-m", files.join(name).to_str().unwrap(), "::"]); }
    let mut image = Image { data: std::fs::read(&path).unwrap(), writable: false, writes: 0, fail_after: None, lost: None };
    image.data.truncate(mib * MIB);
    image
}

fn fsck(path: &Path) {
    let out = Command::new("fsck.fat").arg("-n").arg(path).output().unwrap();
    assert!(out.status.success(), "fsck.fat -n {}: {}{}", path.display(), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

fn table(disk: &Image) -> Vec<(u8, u64, u64)> {
    let sectors = disk.sectors();
    let mut read = |n: u64, out: &mut [u8; partition::SECTOR]| disk.data.get(n as usize * SECTOR..(n as usize + 1) * SECTOR).map(|s| out.copy_from_slice(s)).is_some();
    partition::read_table(sectors, &mut read).partitions().map(|p| (match p.kind { partition::Kind::Mbr(k) => k, _ => 0 }, p.first, p.count)).collect()
}

fn has_esp(disk: &Image) -> bool { (0..4).any(|i| disk.data[446 + i * 16 + 4] == install::ESP_TYPE) }

#[test]
fn installs_a_disk_the_tools_accept() {
    if !tools() { return; }
    let dir = temp("accept");
    let mut volume = source(&dir, 40);
    let mut disk = Image::blank(240);
    let layout = install::install(&mut disk, &mut volume, SIGNATURE, STAMP).unwrap();
    assert_eq!(layout, Layout { esp: (2048, 40 << 11), log: (2048 + (40 << 11), 64 << 11), store: (2048 + (104 << 11), 128 << 11) });
    // The table, as libmind reads it, and the signature.
    assert_eq!(table(&disk), vec![(0xEF, 2048, 40 << 11), (0x0E, layout.log.0 as u64, 64 << 11), (0x7F, layout.store.0 as u64, 128 << 11)]);
    assert_eq!(&disk.data[440..444], &SIGNATURE.to_le_bytes());
    // The ESP: the files of the live volume, fsck clean, hidden sectors at its start, and store.txt naming partition 3.
    let esp = dir.join("esp.img");
    std::fs::write(&esp, disk.part(layout.esp)).unwrap();
    fsck(&esp);
    assert_eq!(u32::from_le_bytes(disk.sector(2048)[28..32].try_into().unwrap()), 2048);
    assert_eq!(mtools("mtype", &esp, &["::MIND/A/kernel.elf"]), kernel());
    assert_eq!(mtools("mtype", &esp, &["::EFI/BOOT/BOOTX64.EFI"]), b"MZ loader");
    let named = String::from_utf8(mtools("mtype", &esp, &["::store.txt"])).unwrap();
    assert_eq!(partition::named(named.as_bytes()), Ok(Some(partition::Id::Mbr { signature: SIGNATURE, number: 3 })));
    // MIND LOG: fsck clean, its label, its hidden sectors.
    let log = dir.join("log.img");
    std::fs::write(&log, disk.part(layout.log)).unwrap();
    fsck(&log);
    assert!(String::from_utf8_lossy(&mtools("minfo", &log, &["::"])).contains("MIND LOG"));
    assert_eq!(u32::from_le_bytes(disk.sector(layout.log.0)[28..32].try_into().unwrap()), layout.log.0);
    // The store's partition: blank, so not taken by its type alone, but named.
    assert!(disk.sector(layout.store.0).iter().all(|&b| b == 0));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn installs_again_over_an_installed_disk() {
    if !tools() { return; }
    let dir = temp("again");
    let mut volume = source(&dir, 40);
    let mut disk = Image::blank(240);
    install::install(&mut disk, &mut volume, SIGNATURE, STAMP).unwrap();
    let layout = install::install(&mut disk, &mut volume, 0x1234_5678, STAMP).unwrap();
    let esp = dir.join("esp.img");
    std::fs::write(&esp, disk.part(layout.esp)).unwrap();
    fsck(&esp);
    let named = String::from_utf8(mtools("mtype", &esp, &["::store.txt"])).unwrap();
    assert_eq!(partition::named(named.as_bytes()), Ok(Some(partition::Id::Mbr { signature: 0x1234_5678, number: 3 })));
    assert_eq!(String::from_utf8_lossy(&mtools("mdir", &esp, &["-b", "::"])).matches("store.txt").count(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_disk_that_cannot_hold_it_is_left_as_it_was() {
    if !tools() { return; }
    let dir = temp("refused");
    let mut volume = source(&dir, 40);
    let mut small = Image::blank(232);
    assert_eq!(install::install(&mut small, &mut volume, SIGNATURE, STAMP), Err(Step::TooSmall));
    assert_eq!(small.writes, 0);
    let mut locked = Image::blank(240);
    locked.writable = false;
    assert_eq!(install::install(&mut locked, &mut volume, SIGNATURE, STAMP), Err(Step::ReadOnly));
    assert_eq!(install::install(&mut Huge(install::MBR_SECTORS + 1), &mut volume, SIGNATURE, STAMP), Err(Step::TooLarge));
    let mut not_fat = Image::blank(8);
    assert_eq!(install::install(&mut Image::blank(240), &mut not_fat, SIGNATURE, STAMP), Err(Step::Source));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_write_cut_short_leaves_no_esp() {
    if !tools() { return; }
    let dir = temp("cut");
    let mut volume = source(&dir, 40);
    // Writes of a whole run: the table first, then the GPT's places, the volume, MIND LOG, the store, store.txt, the table.
    let mut whole = Image::blank(240);
    install::install(&mut whole, &mut volume, SIGNATURE, STAMP).unwrap();
    let total = whole.writes;
    let copy_end = 1 + 2 * 33 + (40 << 11);
    let cuts = [0, 1, 40, copy_end / 2, copy_end, copy_end + 100, total - 2048 - 10, total - 3, total - 1];
    let mut steps = Vec::new();
    for cut in cuts {
        // Over an installed disk too, whose old ESP's entry must be gone once the first write is done.
        for start in [Image::blank(240), Image { data: whole.data.clone(), ..Image::blank(0) }] {
            let mut disk = Image { fail_after: Some(cut), writes: 0, ..start };
            let step = install::install(&mut disk, &mut volume, 0x0BAD_0001, STAMP).unwrap_err();
            steps.push(step);
            if cut > 0 { assert!(!has_esp(&disk), "cut after {} writes ({:?}): an ESP's entry is left", cut, step); }
        }
    }
    for step in [Step::Clear, Step::Copy, Step::Log, Step::Named, Step::Table] { assert!(steps.contains(&step), "no cut fell in {:?}: {:?}", step, steps); }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_sector_the_medium_did_not_keep_is_found() {
    if !tools() { return; }
    let dir = temp("lost");
    let mut volume = source(&dir, 40);
    let mut disk = Image { lost: Some(2048), ..Image::blank(240) };
    assert_eq!(install::install(&mut disk, &mut volume, SIGNATURE, STAMP), Err(Step::Verify));
    assert!(!has_esp(&disk));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_layout_leaves_room_for_each_partition() {
    let l = install::layout(1 << 22, 89 << 11).unwrap();
    assert_eq!(l.esp, (2048, 89 << 11));
    assert!(l.log.0 >= l.esp.0 + l.esp.1 && l.log.0 % 2048 == 0);
    assert!(l.store.0 >= l.log.0 + l.log.1 && l.store.0 % 2048 == 0);
    assert_eq!(install::layout(1 << 22, 0), Err(Step::TooSmall));
    assert_eq!(install::layout(install::MBR_SECTORS + 1, 89 << 11), Err(Step::TooLarge));
    assert!(install::layout(install::MBR_SECTORS, 89 << 11).is_ok());
}
