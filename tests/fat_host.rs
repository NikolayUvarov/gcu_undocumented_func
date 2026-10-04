//! Host tests of the FAT writer (vfs_server/src/fat.rs) against the standard tools: images made by `mkfs.fat`
//! (FAT12, FAT16, FAT32) and by `format`, changed through the writer — long and Cyrillic names, files across clusters,
//! overwrite, truncate, rename and move, remove, directories — then checked with `fsck.fat -n` and read back with
//! mtools. A random sequence of operations is compared with a model. Skipped if the tools are missing. (mtools reads
//! non-ASCII long names as Latin-1 here, so files with Cyrillic names are fetched by their 8.3 alias and their long
//! names are checked by mounting again.)
#![allow(dead_code)]
extern crate alloc;
#[path = "../vfs_server/src/fat.rs"]
mod fat;

use fat::{Error, Node, Report, Sectors, Volume, SECTOR};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Image { data: Vec<u8>, writable: bool, flushes: usize }
impl Sectors for Image {
    fn read(&mut self, lba: u32, out: &mut [u8; SECTOR]) -> bool {
        let at = lba as usize * SECTOR;
        match self.data.get(at..at + SECTOR) { Some(s) => { out.copy_from_slice(s); true } None => false }
    }
    fn write(&mut self, lba: u32, data: &[u8; SECTOR]) -> bool {
        let at = lba as usize * SECTOR;
        match self.data.get_mut(at..at + SECTOR) { Some(s) if self.writable => { s.copy_from_slice(data); true } _ => false }
    }
    fn flush(&mut self) -> bool { self.flushes += 1; true }
    fn sectors(&self) -> u64 { (self.data.len() / SECTOR) as u64 }
    fn writable(&self) -> bool { self.writable }
}

fn tools() -> bool {
    let ok = ["mkfs.fat", "fsck.fat", "mtype", "mdir", "mcopy"].iter().all(|t| Command::new("sh").args(["-c", &format!("command -v {}", t)]).output().is_ok_and(|o| o.status.success()));
    if !ok { eprintln!("SKIP: mkfs.fat, fsck.fat and mtools are needed"); }
    ok
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mind-fat-{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("disk.img")
}

fn mkfs(path: &Path, bits: u32, mib: u64) -> Image {
    let file = std::fs::File::create(path).unwrap();
    file.set_len(mib << 20).unwrap();
    let out = Command::new("mkfs.fat").args(["-F", &bits.to_string(), "-n", "TESTVOL", path.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success(), "{:?}", out);
    Image { data: std::fs::read(path).unwrap(), writable: true, flushes: 0 }
}

fn fsck(path: &Path) {
    let out = Command::new("fsck.fat").args(["-n", "-v", path.to_str().unwrap()]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success() && !text.contains("Dirty bit"), "fsck.fat -n failed:\n{}", text);
}

fn mtype(path: &Path, file: &str) -> Vec<u8> {
    let out = Command::new("mtype").env("MTOOLS_SKIP_CHECK", "1").args(["-i", path.to_str().unwrap(), &format!("::/{}", file)]).output().unwrap();
    assert!(out.status.success(), "mtype {}: {}", file, String::from_utf8_lossy(&out.stderr));
    out.stdout
}

fn mdir(path: &Path, dir: &str) -> String {
    let out = Command::new("mdir").env("MTOOLS_SKIP_CHECK", "1").env("LC_ALL", "C.UTF-8").args(["-b", "-i", path.to_str().unwrap(), &format!("::/{}", dir)]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

// The path of 8.3 aliases for `path`, as mtools can open it whatever the long names are.
fn alias(v: &mut Volume<Image>, path: &str) -> String {
    let mut node = v.root();
    let mut out = Vec::new();
    for part in path.split('/') {
        let entry = v.find(&node, part).unwrap();
        let base: String = entry.short[..8].iter().map(|&c| c as char).collect::<String>().trim_end().into();
        let ext: String = entry.short[8..].iter().map(|&c| c as char).collect::<String>().trim_end().into();
        out.push(if ext.is_empty() { base } else { format!("{}.{}", base, ext) });
        node = entry.node;
    }
    out.join("/")
}

fn save(volume: &mut Volume<Image>, path: &Path) { volume.flush().unwrap(); std::fs::write(path, &volume.disk.data).unwrap(); }

fn content(seed: u32, len: usize) -> Vec<u8> { (0..len).map(|i| (i as u32).wrapping_mul(2654435761).wrapping_add(seed).rotate_left(7) as u8).collect() }

const STAMP: u32 = ((2026 - 1980) << 9 | 10 << 5 | 4) << 16 | (12 << 11 | 34 << 5 | 28);

fn exercise(bits: u32, mib: u64) {
    let path = temp(&format!("fat{}", bits));
    let image = mkfs(&path, bits, mib);
    let mut v = Volume::mount(image).ok().expect("mount");
    assert_eq!(v.bits() as u32, bits);
    assert_eq!(v.label(), "TESTVOL");
    let root = v.root();
    let free = v.free_clusters().unwrap();
    // Names: a plain 8.3 name, a long one, Cyrillic, one that needs an alias with ~2.
    let docs = v.create(&root, "Документы", true, STAMP).unwrap();
    let mut plain = v.create(&root, "notes.txt", false, STAMP).unwrap();
    let mut long = v.create(&docs, "A rather long file name.markdown", false, STAMP).unwrap();
    let mut first = v.create(&docs, "Отчёт за октябрь.txt", false, STAMP).unwrap();
    let mut second = v.create(&docs, "Отчёт за ноябрь.txt", false, STAMP).unwrap();
    assert_eq!(v.create(&root, "NOTES.TXT", false, STAMP).unwrap_err(), Error::Exists, "names ignore case");
    assert_eq!(v.create(&root, "bad:name", false, STAMP).unwrap_err(), Error::Name);
    // Data across clusters, an overwrite in the middle, a gap that reads as zeros.
    let per = v.cluster_bytes() as usize;
    let big = content(1, per * 3 + 700);
    v.write(&mut long, 0, &big, STAMP).unwrap();
    v.write(&mut long, (per + 10) as u32, b"PATCH", STAMP).unwrap();
    let mut expected_long = big.clone();
    expected_long[per + 10..per + 15].copy_from_slice(b"PATCH");
    v.write(&mut plain, 0, "Привет, мир\n".as_bytes(), STAMP).unwrap();
    v.write(&mut first, 100, b"tail", STAMP).unwrap();
    let mut expected_first = vec![0u8; 100]; expected_first.extend_from_slice(b"tail");
    v.write(&mut second, 0, &content(2, per * 2), STAMP).unwrap();
    v.truncate(&mut second, 10, STAMP).unwrap();
    let mut back = vec![0u8; expected_long.len() + 10];
    assert_eq!(v.read(&long, 0, &mut back).unwrap(), expected_long.len());
    assert_eq!(&back[..expected_long.len()], &expected_long[..]);
    // Directories: nested, moved, renamed; a directory cannot move into itself.
    let sub = v.create(&docs, "old", true, STAMP).unwrap();
    let deep = v.create(&sub, "deeper", true, STAMP).unwrap();
    assert_eq!(v.rename(&docs, "old", &deep, "loop").unwrap_err(), Error::Invalid);
    v.rename(&docs, "old", &root, "Archive 2026", ).unwrap();
    v.rename(&root, "notes.txt", &root, "Notes.TXT").unwrap(); // a change of case
    v.rename(&docs, "Отчёт за ноябрь.txt", &sub, "ноябрь.txt").unwrap();
    assert_eq!(v.remove(&root, "Archive 2026").unwrap_err(), Error::NotEmpty);
    let temp_file = v.create(&root, "temp.bin", false, STAMP).unwrap();
    let mut temp_node = temp_file;
    v.write(&mut temp_node, 0, &content(3, per * 4), STAMP).unwrap();
    v.remove(&root, "temp.bin").unwrap();
    // Many entries make the directory grow past one cluster.
    for i in 0..(per / 32 + 8) { v.create(&deep, &format!("entry number {:03}", i), false, STAMP).unwrap(); }
    save(&mut v, &path);
    fsck(&path);
    // What the standard tools see.
    assert_eq!(mtype(&path, "Notes.TXT"), "Привет, мир\n".as_bytes());
    let paths = ["Документы/A rather long file name.markdown", "Документы/Отчёт за октябрь.txt", "Archive 2026/ноябрь.txt"].map(|p| alias(&mut v, p));
    assert_eq!(mtype(&path, &paths[0]), expected_long);
    assert_eq!(mtype(&path, &paths[1]), expected_first);
    assert_eq!(mtype(&path, &paths[2]), content(2, 10));
    assert!(mdir(&path, &alias(&mut v, "Документы")).contains("A rather long file name.markdown"));
    let listing = mdir(&path, "Archive 2026/deeper");
    assert!(listing.contains(&format!("entry number {:03}", per / 32 + 7)), "{}", listing);
    assert!(!mdir(&path, "").contains("temp.bin"));
    // Read back through the writer after a fresh mount.
    let image = Image { data: std::fs::read(&path).unwrap(), writable: false, flushes: 0 };
    let mut v = Volume::mount(image).ok().unwrap();
    let root = v.root();
    let names: Vec<String> = v.list(&root).unwrap().into_iter().map(|e| e.name).collect();
    assert!(names.contains(&"Документы".to_string()) && names.contains(&"Notes.TXT".to_string()) && names.contains(&"Archive 2026".to_string()), "{:?}", names);
    let node = v.lookup(&root, "документы/отчёт ЗА октябрь.txt").unwrap();
    assert_eq!(node.modified, STAMP);
    assert_eq!(v.create(&root, "x", false, STAMP).unwrap_err(), Error::ReadOnly);
    let _ = (free, docs);
    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[test]
fn fat12() { if tools() { exercise(12, 2); } }
#[test]
fn fat16() { if tools() { exercise(16, 32); } }
#[test]
fn fat32() { if tools() { exercise(32, 64); } }

#[test]
fn reads_files_made_by_mtools() {
    if !tools() { return; }
    let path = temp("mcopy");
    mkfs(&path, 16, 16);
    let source = path.parent().unwrap().join("A File With A Long Name.txt");
    std::fs::write(&source, "содержимое").unwrap();
    let out = Command::new("mcopy").env("MTOOLS_SKIP_CHECK", "1").args(["-i", path.to_str().unwrap(), source.to_str().unwrap(), "::/"]).output().unwrap();
    assert!(out.status.success());
    let mut v = Volume::mount(Image { data: std::fs::read(&path).unwrap(), writable: false, flushes: 0 }).ok().unwrap();
    let root = v.root();
    let node = v.lookup(&root, "a file with a long NAME.TXT").unwrap();
    let mut data = vec![0u8; 64];
    let n = v.read(&node, 0, &mut data).unwrap();
    assert_eq!(&data[..n], "содержимое".as_bytes());
    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[test]
fn format_makes_a_volume_the_tools_accept() {
    if !tools() { return; }
    for mib in [1u64, 8, 64] {
        let path = temp(&format!("format{}", mib));
        let mut image = Image { data: vec![0u8; (mib << 20) as usize], writable: true, flushes: 0 };
        fat::format(&mut image, "mind ram", STAMP).unwrap();
        let mut v = Volume::mount(image).ok().unwrap();
        assert_eq!(v.label(), "MIND RAM");
        assert_eq!(v.bits(), if mib < 2 { 12 } else { 16 });
        let root = v.root();
        let mut file = v.create(&root, "hello.txt", false, STAMP).unwrap();
        v.write(&mut file, 0, b"hello", STAMP).unwrap();
        save(&mut v, &path);
        fsck(&path);
        assert_eq!(mtype(&path, "hello.txt"), b"hello");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}

#[test]
fn reformat_replaces_the_volume_in_place() {
    // The RAM disk's `format` request (vfs.wit 2.3): files and directories are gone, the label is new, the volume
    // works at once, and (with the tools) fsck.fat accepts the result.
    let mut image = Image { data: vec![0u8; 8 << 20], writable: true, flushes: 0 };
    fat::format(&mut image, "MIND RAM", STAMP).unwrap();
    let mut v = Volume::mount(image).ok().unwrap();
    let root = v.root();
    let dir = v.create(&root, "docs", true, STAMP).unwrap();
    let mut file = v.create(&dir, "note.txt", false, STAMP).unwrap();
    v.write(&mut file, 0, &content(1, 5000), STAMP).unwrap();
    let flushes = v.disk.flushes;
    v.reformat("scratch", STAMP).unwrap();
    assert!(v.disk.flushes > flushes, "the new volume is flushed");
    assert_eq!(v.label(), "SCRATCH");
    let root = v.root();
    assert!(v.list(&root).unwrap().is_empty(), "nothing of the old volume is left");
    assert!(v.lookup(&root, "docs/note.txt").is_err());
    let mut file = v.create(&root, "new.txt", false, STAMP).unwrap();
    v.write(&mut file, 0, b"fresh", STAMP).unwrap();
    assert_eq!(v.check().unwrap().lost, 0);
    if tools() {
        let path = temp("reformat");
        save(&mut v, &path);
        fsck(&path);
        assert_eq!(mtype(&path, "new.txt"), b"fresh");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
    // A read-only medium cannot be formatted.
    let mut image = Image { data: vec![0u8; 1 << 20], writable: true, flushes: 0 };
    fat::format(&mut image, "X", STAMP).unwrap();
    let mut v = Volume::mount(image).ok().unwrap();
    v.disk.writable = false;
    assert_eq!(v.reformat("Y", STAMP), Err(Error::ReadOnly));
}

#[test]
fn random_operations_match_a_model() {
    if !tools() { return; }
    for bits in [12u32, 16, 32] {
        let path = temp(&format!("random{}", bits));
        let image = mkfs(&path, bits, match bits { 32 => 40, 16 => 16, _ => 4 });
        let mut v = Volume::mount(image).ok().unwrap();
        let root = v.root();
        let dirs = ["", "a", "a/b", "c"];
        let mut nodes: BTreeMap<&str, Node> = BTreeMap::new();
        nodes.insert("", root);
        for d in &dirs[1..] {
            let (parent, name) = d.rsplit_once('/').unwrap_or(("", d));
            let node = v.create(&nodes[parent], name, true, STAMP).unwrap();
            nodes.insert(d, node);
        }
        let mut model: BTreeMap<(usize, String), Vec<u8>> = BTreeMap::new();
        let mut seed = 0x1234_5678u32 ^ bits;
        let mut next = || { seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345); seed >> 8 };
        let per = v.cluster_bytes();
        for step in 0..300 {
            let d = next() as usize % dirs.len();
            let name = format!("файл {}.dat", next() % 12);
            let dir = nodes[dirs[d]];
            let key = (d, name.clone());
            match next() % 5 {
                0 | 1 => {
                    let mut node = match v.lookup(&dir, &name) { Ok(n) => n, Err(Error::NotFound) => v.create(&dir, &name, false, STAMP).unwrap(), Err(e) => panic!("{:?}", e) };
                    let data = model.entry(key).or_default();
                    let offset = next() % (data.len() as u32 + per);
                    let chunk = content(step, (next() % (per * 3)) as usize);
                    v.write(&mut node, offset, &chunk, STAMP).unwrap();
                    if data.len() < offset as usize { data.resize(offset as usize, 0); }
                    let end = offset as usize + chunk.len();
                    if data.len() < end { data.resize(end, 0); }
                    data[offset as usize..end].copy_from_slice(&chunk);
                }
                2 => if let Ok(mut node) = v.lookup(&dir, &name) {
                    let size = next() % (per * 4);
                    v.truncate(&mut node, size, STAMP).unwrap();
                    model.get_mut(&key).unwrap().resize(size as usize, 0);
                },
                3 => if v.lookup(&dir, &name).is_ok() { v.remove(&dir, &name).unwrap(); model.remove(&key); },
                _ => if v.lookup(&dir, &name).is_ok() {
                    let t = next() as usize % dirs.len();
                    let new = format!("moved {}.dat", next() % 6);
                    if model.contains_key(&(t, new.clone())) { continue; }
                    v.rename(&dir, &name, &nodes[dirs[t]], &new).unwrap();
                    let data = model.remove(&key).unwrap();
                    model.insert((t, new), data);
                },
            }
        }
        save(&mut v, &path);
        fsck(&path);
        for ((d, name), data) in &model {
            let file = if dirs[*d].is_empty() { name.clone() } else { format!("{}/{}", dirs[*d], name) };
            let short = alias(&mut v, &file);
            assert_eq!(&mtype(&path, &short), data, "{} on FAT{}", file, bits);
            let node = v.lookup(&root, &file).unwrap();
            let mut back = vec![0u8; data.len()];
            assert_eq!(v.read(&node, 0, &mut back).unwrap(), data.len());
            assert_eq!(&back, data);
        }
        let listed: usize = dirs.iter().map(|d| v.list(&nodes[d]).unwrap().iter().filter(|e| !e.node.is_dir()).count()).sum();
        assert_eq!(listed, model.len(), "FAT{}", bits);
        let report = v.check().unwrap();
        assert!(report.clean() && !report.dirty, "FAT{}: {:?}", bits, report);
        assert_eq!((report.files as usize, report.directories), (model.len(), 3));
        assert_eq!(report.free, v.free_clusters().unwrap());
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}

// Sets a FAT entry in every copy of the FAT of a bare (unpartitioned) image.
fn patch_fat(data: &mut [u8], bits: u32, cluster: u32, value: u32) {
    let u16_at = |at: usize| u16::from_le_bytes([data[at], data[at + 1]]) as usize;
    let (reserved, fats) = (u16_at(14), data[16] as usize);
    let fat_size = if u16_at(22) != 0 { u16_at(22) } else { u32::from_le_bytes(data[36..40].try_into().unwrap()) as usize };
    for copy in 0..fats {
        let base = (reserved + copy * fat_size) * 512;
        match bits {
            12 => {
                let at = base + (cluster + cluster / 2) as usize;
                if cluster & 1 == 0 { data[at] = value as u8; data[at + 1] = (data[at + 1] & 0xF0) | ((value >> 8) as u8 & 0x0F); }
                else { data[at] = (data[at] & 0x0F) | (value << 4) as u8; data[at + 1] = (value >> 4) as u8; }
            }
            16 => data[base + cluster as usize * 2..][..2].copy_from_slice(&(value as u16).to_le_bytes()),
            _ => data[base + cluster as usize * 4..][..4].copy_from_slice(&value.to_le_bytes()),
        }
    }
}

// fsck.fat -n finds errors (exit code 1) in the image.
fn fsck_fails(path: &Path, data: &[u8]) -> bool {
    std::fs::write(path, data).unwrap();
    !Command::new("fsck.fat").args(["-n", path.to_str().unwrap()]).output().unwrap().status.success()
}

fn check_of(data: &[u8]) -> Report {
    let mut v = Volume::mount(Image { data: data.to_vec(), writable: false, flushes: 0 }).ok().unwrap();
    v.check().unwrap()
}

#[test]
fn check_finds_what_fsck_finds() {
    if !tools() { return; }
    for (bits, mib) in [(12u32, 2u64), (16, 16), (32, 40)] {
        let path = temp(&format!("check{}", bits));
        let mut v = Volume::mount(mkfs(&path, bits, mib)).ok().unwrap();
        let root = v.root();
        let per = v.cluster_bytes() as usize;
        let dir = v.create(&root, "каталог", true, STAMP).unwrap();
        let mut a = v.create(&root, "a.bin", false, STAMP).unwrap();
        let mut b = v.create(&root, "b.bin", false, STAMP).unwrap();
        let mut c = v.create(&dir, "c.txt", false, STAMP).unwrap();
        v.write(&mut a, 0, &content(1, per * 2 + 5), STAMP).unwrap();
        v.write(&mut b, 0, &content(2, per + 1), STAMP).unwrap();
        v.write(&mut c, 0, b"text", STAMP).unwrap();
        save(&mut v, &path);
        let report = v.check().unwrap();
        assert!(report.clean() && !report.dirty, "FAT{}: {:?}", bits, report);
        assert_eq!((report.files, report.directories, report.lost), (3, 1, 0));
        let total = v.total_bytes() as u32 / per as u32;
        assert_eq!(report.used + report.free, total, "FAT{}: {:?}", bits, report);
        let clean = v.disk.data.clone();
        let (a, b) = (v.lookup(&root, "a.bin").unwrap(), v.lookup(&root, "b.bin").unwrap());
        let a_second = v.fat(a.cluster).unwrap();
        let b_last = v.fat(b.cluster).unwrap();
        // A chain cut short: the rest of it is lost, the size no longer fits.
        let mut data = clean.clone();
        patch_fat(&mut data, bits, a_second, 0);
        let report = check_of(&data);
        assert_eq!((report.bad_chains, report.lost, report.lost_chains, report.sizes), (1, 1, 1, 1), "FAT{}: {:?}", bits, report);
        assert!(report.first.starts_with("a.bin: "), "{}", report.first);
        assert!(fsck_fails(&path, &data), "FAT{}: fsck.fat finds the cut chain", bits);
        // Two chains that join: b's end leads into a.
        let mut data = clean.clone();
        patch_fat(&mut data, bits, b_last, a.cluster);
        let report = check_of(&data);
        assert!(report.cross_linked == 1 && report.first.starts_with("b.bin: "), "FAT{}: {:?}", bits, report);
        assert!(fsck_fails(&path, &data), "FAT{}: fsck.fat finds the cross link", bits);
        // A cluster in use that nothing reaches.
        let mut data = clean.clone();
        let free = (2..total + 2).rev().find(|&cl| v.fat(cl).unwrap() == 0).unwrap();
        patch_fat(&mut data, bits, free, 0x0FFF_FFFF & match bits { 12 => 0xFFF, 16 => 0xFFFF, _ => 0x0FFF_FFFF });
        let report = check_of(&data);
        assert_eq!((report.lost, report.lost_chains, report.bad_chains, report.cross_linked), (1, 1, 0, 0), "FAT{}: {:?}", bits, report);
        assert!(fsck_fails(&path, &data), "FAT{}: fsck.fat finds the lost cluster", bits);
        // A file larger than its chain.
        let mut data = clean.clone();
        let at = b.entry.unwrap();
        let offset = at.lba as usize * 512 + at.offset as usize + 28;
        data[offset..offset + 4].copy_from_slice(&(per as u32 * 5).to_le_bytes());
        let report = check_of(&data);
        assert_eq!((report.sizes, report.bad_chains, report.lost), (1, 0, 0), "FAT{}: {:?}", bits, report);
        assert!(fsck_fails(&path, &data), "FAT{}: fsck.fat finds the size", bits);
        // A short name with a character FAT does not allow.
        let mut data = clean.clone();
        data[at.lba as usize * 512 + at.offset as usize + 1] = b'*';
        let report = check_of(&data);
        assert_eq!(report.bad_entries, 1, "FAT{}: {:?}", bits, report);
        // A volume that was not flushed after a change is dirty (not an error by itself).
        let mut v = Volume::mount(Image { data: clean.clone(), writable: true, flushes: 0 }).ok().unwrap();
        let root = v.root();
        v.create(&root, "new.txt", false, STAMP).unwrap();
        let report = v.check().unwrap();
        assert_eq!((report.dirty, report.clean()), (bits != 12, true), "FAT{}: {:?}", bits, report);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}

#[test]
fn stamps() {
    assert_eq!(fat::stamp(0, 0), (20 << 9 | 1 << 5 | 1) << 16); // 2000-01-01 00:00:00
    assert_eq!(fat::stamp(9773, 12 * 3600 + 34 * 60 + 56), STAMP); // 2026-10-04
    assert!(fat::valid_name("Отчёт.txt") && !fat::valid_name("a/b") && !fat::valid_name("trailing.") && !fat::valid_name(".."));
    assert!(fat::same_name("ДОКУМЕНТЫ", "документы"));
}
