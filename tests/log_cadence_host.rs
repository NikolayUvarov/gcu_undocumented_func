//! Host tests of when the system log is saved (vfs_server/src/cadence.rs, 211-KRN-0223): every record in the boot's
//! first BOOT_MS, then a warning at once and ordinary records after ORDINARY_MS or once MANY wait, each looked at once;
//! and the FAT writer's `flush_open`, one flush a save with the volume left dirty until `flush` (vfs_server/src/fat.rs),
//! checked by fsck.fat.
#![allow(dead_code)]
extern crate alloc;
#[path = "../vfs_server/src/cadence.rs"]
mod cadence;
#[path = "../vfs_server/src/fat.rs"]
mod fat;

use cadence::{Cadence, BOOT_MS, MANY, ORDINARY_MS, SAVE_MS};
use fat::{Sectors, Volume, SECTOR};
use std::process::Command;

#[test]
fn the_boot_is_saved_every_look_then_a_warning_at_once_and_ordinary_records_wait() {
    let mut c = Cadence::new();
    // The boot's first BOOT_MS: any record at the next look.
    assert!(c.wanted(0, SAVE_MS, [(0, 1)]));
    c.saved(SAVE_MS);
    assert!(c.wanted(1, BOOT_MS - 1, [(1, 0)]));
    let t = BOOT_MS;
    c.saved(t);
    // After it, info records not before ORDINARY_MS since the last save.
    assert!(!c.wanted(2, t + SAVE_MS, [(2, 1), (3, 1)]));
    assert!(!c.wanted(2, t + 2 * SAVE_MS, [(4, 0)]));
    assert!(c.wanted(2, t + ORDINARY_MS, []));
    let t = t + ORDINARY_MS;
    c.saved(t);
    // Saved through 4; a warning among new records is saved at once, however soon.
    assert!(!c.wanted(5, t + SAVE_MS, []));
    assert!(c.wanted(5, t + SAVE_MS, [(5, 1), (6, 2)]));
    let t = t + SAVE_MS;
    c.saved(t);
    // Nothing new: nothing to save, even long after.
    assert!(!c.wanted(7, t + 10 * ORDINARY_MS, []));
    // An error too.
    assert!(c.wanted(7, t + 10 * ORDINARY_MS, [(7, 3)]));
}

#[test]
fn records_already_looked_at_are_not_read_again_and_many_are_saved() {
    let mut c = Cadence::new();
    let t = BOOT_MS;
    c.saved(t);
    assert!(!c.wanted(0, t + 1, (0..10).map(|s| (s, 1))));
    assert_eq!(c.seen(0), 10);
    // A record read again is skipped; the next look starts where the last one stopped.
    assert!(!c.wanted(0, t + 2, [(9, 3), (10, 1)]));
    assert_eq!(c.seen(0), 11);
    // MANY waiting: saved without waiting for ORDINARY_MS.
    assert!(c.wanted(0, t + 3, (11..MANY).map(|s| (s, 1))));
    // After a save that went past `seen`, the look starts at `next`.
    assert_eq!(c.seen(MANY + 5), MANY + 5);
}

struct Image { data: Vec<u8>, flushes: usize }
impl Sectors for Image {
    fn read(&mut self, lba: u32, out: &mut [u8; SECTOR]) -> bool { let at = lba as usize * SECTOR; match self.data.get(at..at + SECTOR) { Some(s) => { out.copy_from_slice(s); true } None => false } }
    fn write(&mut self, lba: u32, data: &[u8; SECTOR]) -> bool { let at = lba as usize * SECTOR; match self.data.get_mut(at..at + SECTOR) { Some(s) => { s.copy_from_slice(data); true } None => false } }
    fn flush(&mut self) -> bool { self.flushes += 1; true }
    fn sectors(&self) -> u64 { (self.data.len() / SECTOR) as u64 }
    fn writable(&self) -> bool { true }
}

fn fsck(data: &[u8]) -> (bool, String) {
    let path = std::env::temp_dir().join(format!("mind-log-cadence-{}.img", std::process::id()));
    std::fs::write(&path, data).unwrap();
    let out = Command::new("fsck.fat").args(["-n", path.to_str().unwrap()]).output().unwrap();
    let _ = std::fs::remove_file(&path);
    (out.status.success(), String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr))
}

#[test]
fn a_save_flushes_once_and_leaves_the_volume_dirty_until_a_flush() {
    if !Command::new("sh").args(["-c", "command -v mkfs.fat && command -v fsck.fat"]).output().is_ok_and(|o| o.status.success()) { eprintln!("SKIP: mkfs.fat and fsck.fat are needed"); return; }
    let path = std::env::temp_dir().join(format!("mind-log-cadence-mkfs-{}.img", std::process::id()));
    std::fs::File::create(&path).unwrap().set_len(16 << 20).unwrap();
    assert!(Command::new("mkfs.fat").args(["-F", "16", "-n", "MIND LOG", path.to_str().unwrap()]).output().unwrap().status.success());
    let image = Image { data: std::fs::read(&path).unwrap(), flushes: 0 };
    let _ = std::fs::remove_file(&path);
    let mut v = Volume::mount(image).ok().unwrap();
    let root = v.root();
    let mut node = v.create(&root, "boot0001.log", false, 0).ok().unwrap();
    v.write(&mut node, 0, b"THE SYSTEM LOG OF ONE BOOT\n", 0).ok().unwrap();
    v.flush().ok().unwrap();
    assert!(fsck(&v.disk.data).0, "clean after the journal's start");
    // The first save marks the volume dirty (its own flush) and flushes; each later one flushes once and no more.
    let line = [b'x'; 79];
    for save in 0..20 {
        let before = v.disk.flushes;
        let end = node.size;
        let mut text = line.to_vec(); text.push(b'\n');
        v.write(&mut node, end, &text, 0).ok().unwrap();
        v.flush_open().ok().unwrap();
        assert_eq!(v.disk.flushes - before, if save == 0 { 2 } else { 1 }, "save {save}");
    }
    let (ok, text) = fsck(&v.disk.data);
    assert!(text.contains("Dirty bit") || !ok, "left dirty between saves: {text}");
    // A flush marks it clean, and every byte is there.
    v.flush().ok().unwrap();
    let (ok, text) = fsck(&v.disk.data);
    assert!(ok && !text.contains("Dirty bit"), "{text}");
    assert_eq!(node.size, 27 + 20 * 80);
}
