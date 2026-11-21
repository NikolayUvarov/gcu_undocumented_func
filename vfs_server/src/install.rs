// The installer's writer (352-UPD-0048): MIND Core from the live system's boot volume onto a disk of this machine, laid out
// as make_usb_image.py --slots lays out a stick: the boot volume as the ESP, MIND LOG, and the block store's partition.
// The ESP's entry is written last, so a disk cut short has no ESP and the firmware does not boot it.
use crate::fat::{self, Sectors, Volume, SECTOR};
use alloc::vec;

pub const ALIGN: u32 = 2048; // 1 MiB
pub const LOG_SECTORS: u32 = 64 << 11; // 64 MiB, as make_usb_image.py's
pub const STORE_SECTORS: u32 = 128 << 11; // 128 MiB, as make_usb_image.py's
pub const ESP_TYPE: u8 = 0xEF;
pub const LOG_TYPE: u8 = 0x0E; // FAT16 LBA, as the stick's
pub const STORE_TYPE: u8 = 0x7F; // libmind's partition::MBR_STORE
pub const LOG_LABEL: &str = "MIND LOG";
pub const MBR_SECTORS: u64 = 1 << 32; // what an MBR addresses: 2 TiB of 512-byte sectors
const RUN: usize = 256; // sectors a copy reads and writes at once
const GPT_SECTORS: u32 = 33; // a GPT's header and entries, at each end of the disk

/// Where each partition goes, as (first sector, sectors).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout { pub esp: (u32, u32), pub log: (u32, u32), pub store: (u32, u32) }

/// The step that failed; the disk then holds no ESP's entry, and is installed again from the start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step { TooSmall, TooLarge, ReadOnly, Source, Clear, Copy, Verify, Log, Store, Named, Table, ReadBack }

/// The partitions on a disk of `sectors` for a boot volume of `volume` sectors; past 2 TiB a GPT is needed (212 step 3).
pub fn layout(sectors: u64, volume: u32) -> Result<Layout, Step> {
    if sectors > MBR_SECTORS { return Err(Step::TooLarge); }
    let up = |n: u64| n.div_ceil(ALIGN as u64) * ALIGN as u64;
    let log = up(ALIGN as u64 + volume as u64);
    let store = up(log + LOG_SECTORS as u64);
    if volume == 0 || store + STORE_SECTORS as u64 > sectors { return Err(Step::TooSmall); }
    Ok(Layout { esp: (ALIGN, volume), log: (log as u32, LOG_SECTORS), store: (store as u32, STORE_SECTORS) })
}

/// The sectors of the FAT volume that starts at sector 0 of `source`, by its boot sector.
pub fn volume_sectors<S: Sectors>(source: &mut S) -> Option<u32> {
    let mut boot = [0u8; SECTOR];
    if !source.read(0, &mut boot) || boot[510..] != [0x55, 0xAA] || u16::from_le_bytes([boot[11], boot[12]]) != SECTOR as u16 { return None; }
    let small = u16::from_le_bytes([boot[19], boot[20]]) as u32;
    let total = if small != 0 { small } else { u32::from_le_bytes([boot[32], boot[33], boot[34], boot[35]]) };
    (total != 0 && total as u64 <= source.sectors()).then_some(total)
}

/// The MBR: the disk's signature, and the entries of `layout`, the ESP's alone when `esp`.
pub fn mbr(signature: u32, layout: &Layout, esp: bool) -> [u8; SECTOR] {
    let mut s = [0u8; SECTOR];
    s[440..444].copy_from_slice(&signature.to_le_bytes());
    let entries = [(esp, ESP_TYPE, layout.esp), (true, LOG_TYPE, layout.log), (true, STORE_TYPE, layout.store)];
    for (slot, &(_, kind, (first, count))) in entries.iter().enumerate().filter(|(_, e)| e.0) {
        let e = 446 + slot * 16;
        s[e + 1..e + 4].copy_from_slice(&[0xFE, 0xFF, 0xFF]); // CHS past what it can say, as the images' entries
        s[e + 4] = kind;
        s[e + 5..e + 8].copy_from_slice(&[0xFE, 0xFF, 0xFF]);
        s[e + 8..e + 12].copy_from_slice(&first.to_le_bytes());
        s[e + 12..e + 16].copy_from_slice(&count.to_le_bytes());
    }
    s[510] = 0x55; s[511] = 0xAA;
    s
}

/// `store.txt` for the store's partition, the third of an MBR disk (307-KRN-0177).
pub fn store_txt(signature: u32) -> alloc::string::String {
    alloc::format!("# The block store's partition, written by the installer (352-UPD-0048): blank until the store's first start.\npartition {:08x}-03\n", signature)
}

/// A window of `disk`: `count` sectors from `first`, as a disk of its own, for `fat::format`.
struct Window<'a, T: Sectors> { disk: &'a mut T, first: u32, count: u32 }
impl<T: Sectors> Sectors for Window<'_, T> {
    fn read(&mut self, lba: u32, out: &mut [u8; SECTOR]) -> bool { lba < self.count && self.disk.read(self.first + lba, out) }
    fn write(&mut self, lba: u32, data: &[u8; SECTOR]) -> bool { lba < self.count && self.disk.write(self.first + lba, data) }
    fn flush(&mut self) -> bool { self.disk.flush() }
    fn sectors(&self) -> u64 { self.count as u64 }
    fn writable(&self) -> bool { self.disk.writable() }
}

fn put_hidden(boot: &mut [u8], first: u32) { boot[28..32].copy_from_slice(&first.to_le_bytes()); }

fn zero<T: Sectors>(target: &mut T, first: u32, count: u32) -> bool {
    let blank = [0u8; SECTOR];
    (first..first + count).all(|lba| target.write(lba, &blank))
}

/// Writes the system onto `target` from the FAT volume at sector 0 of `source` (the boot volume the loader verified):
/// a table without the ESP, the volume sector for sector with its hidden sectors set, its copy read back and compared,
/// MIND LOG formatted, the store's partition blank and named in the new ESP's `store.txt`, then the ESP's entry.
/// `signature` is the new disk's (not 0), `stamp` the FAT date and time of what it makes.
pub fn install<T: Sectors, S: Sectors>(target: &mut T, source: &mut S, signature: u32, stamp: u32) -> Result<Layout, Step> {
    if !target.writable() { return Err(Step::ReadOnly); }
    let volume = volume_sectors(source).ok_or(Step::Source)?;
    let layout = layout(target.sectors(), volume)?;
    // No table names an ESP from here on, and no GPT is left at either end of the disk.
    let last = (target.sectors() - GPT_SECTORS as u64) as u32;
    if !target.write(0, &mbr(signature, &layout, false)) || !zero(target, 1, GPT_SECTORS) || !zero(target, last, GPT_SECTORS) || !target.flush() { return Err(Step::Clear); }
    let (esp, _) = layout.esp;
    let mut run = vec![0u8; RUN * SECTOR];
    let mut back = vec![0u8; RUN * SECTOR];
    let mut at = 0;
    while at < volume {
        let n = (volume - at).min(RUN as u32) as usize;
        if !source.read_run(at, &mut run[..n * SECTOR]) { return Err(Step::Source); }
        if at == 0 { put_hidden(&mut run[..SECTOR], esp); }
        if !target.write_run(esp + at, &run[..n * SECTOR]) { return Err(Step::Copy); }
        at += n as u32;
    }
    if !target.flush() { return Err(Step::Copy); }
    // Read back from the medium, past any cache, and compared with the source's sectors.
    target.discard();
    at = 0;
    while at < volume {
        let n = (volume - at).min(RUN as u32) as usize;
        if !source.read_run(at, &mut run[..n * SECTOR]) { return Err(Step::Source); }
        if at == 0 { put_hidden(&mut run[..SECTOR], esp); }
        if !target.read_run(esp + at, &mut back[..n * SECTOR]) || run[..n * SECTOR] != back[..n * SECTOR] { return Err(Step::Verify); }
        at += n as u32;
    }
    let (log, count) = layout.log;
    let mut window = Window { disk: &mut *target, first: log, count };
    if fat::format(&mut window, LOG_LABEL, stamp).is_err() { return Err(Step::Log); }
    let mut boot = [0u8; SECTOR];
    if !target.read(log, &mut boot) { return Err(Step::Log); }
    put_hidden(&mut boot, log);
    if !target.write(log, &boot) { return Err(Step::Log); }
    // Blank, so the store makes itself there at its first start, by its own layout (307-KRN-0177: named, it may be blank).
    if !zero(target, layout.store.0, ALIGN) { return Err(Step::Store); }
    {
        let Ok(mut volume) = Volume::mount_at(&mut *target, esp) else { return Err(Step::Named) };
        let root = volume.root();
        let text = store_txt(signature);
        let named = match volume.lookup(&root, "store.txt") {
            Ok(mut node) => volume.truncate(&mut node, 0, stamp).and_then(|_| volume.write(&mut node, 0, text.as_bytes(), stamp)),
            Err(_) => volume.create(&root, "store.txt", false, stamp).and_then(|mut node| volume.write(&mut node, 0, text.as_bytes(), stamp)),
        };
        if named.ok() != Some(text.len()) || volume.flush().is_err() { return Err(Step::Named); }
    }
    if !target.flush() || !target.write(0, &mbr(signature, &layout, true)) || !target.flush() { return Err(Step::Table); }
    target.discard();
    let mut first = [0u8; SECTOR];
    if !target.read(0, &mut first) || first != mbr(signature, &layout, true) { return Err(Step::ReadBack); }
    Ok(layout)
}
