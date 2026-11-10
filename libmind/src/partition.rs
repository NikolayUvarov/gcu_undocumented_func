//! Partition tables, MBR and GPT, read through any sector source, and the block store's own partition (307-KRN-0177):
//! the store's type and its superblock both, or the owner's word in `store.txt`; never a partition because it is blank.
//! Builds on the host for tests (tests/partition_host.rs).

pub const SECTOR: usize = 512;
/// MBR type of the store's partition: 0x7F, reserved for local and experimental use (make_usb_image.py's partition 3).
pub const MBR_STORE: u8 = 0x7F;
/// GPT type of the store's partition, 27d2c2a1-7133-4d3d-a6ed-1de599cabdb7, in GPT's byte order.
pub const GPT_STORE: [u8; 16] = [0xA1, 0xC2, 0xD2, 0x27, 0x33, 0x71, 0x3D, 0x4D, 0xA6, 0xED, 0x1D, 0xE5, 0x99, 0xCA, 0xBD, 0xB7];
/// The first bytes of the store's superblock (blockstore/src/store.rs).
pub const STORE_MAGIC: &[u8; 8] = b"MIND-STO";
/// Partitions kept per disk; a table with more keeps the first ones and counts the rest.
pub const KEPT: usize = 16;
/// GPT entries read at most; the usual array holds 128.
const GPT_ENTRIES_MAX: u32 = 256;
/// The text of a GUID or a PARTUUID, at most.
pub const ID_TEXT: usize = 36;

/// How a disk is divided: no table (blank, one FAT volume over the whole disk, or unreadable), MBR or GPT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme { None, Mbr, Gpt }

/// A partition's type: the MBR's byte or GPT's type GUID (in GPT's byte order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Mbr(u8), Gpt([u8; 16]) }

/// What the owner names a partition by, as Linux's PARTUUID: GPT's unique partition GUID, or an MBR disk's signature and
/// the partition's number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id { Gpt([u8; 16]), Mbr { signature: u32, number: u8 } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Partition {
    /// 1 to 4 in an MBR, the entry's place from 1 in GPT.
    pub number: u8,
    pub kind: Kind,
    pub first: u64,
    pub count: u64,
    pub id: Id,
    label: [u8; 36],
    label_len: u8,
}

impl Partition {
    /// GPT's name, its non-ASCII characters as `?`; empty in an MBR.
    pub fn label(&self) -> &str { core::str::from_utf8(&self.label[..self.label_len as usize]).unwrap_or("") }
    /// The store's type, MBR or GPT.
    pub fn store_type(&self) -> bool { matches!(self.kind, Kind::Mbr(MBR_STORE)) || self.kind == Kind::Gpt(GPT_STORE) }
}

/// A disk's table: its scheme, the partitions kept, how many more there were, and what was wrong with it, if anything.
#[derive(Clone, Copy, Debug)]
pub struct Table {
    pub scheme: Scheme,
    parts: [Option<Partition>; KEPT],
    pub left_out: usize,
    pub problem: Option<&'static str>,
}

impl Table {
    const fn new(scheme: Scheme) -> Self { Table { scheme, parts: [None; KEPT], left_out: 0, problem: None } }
    pub fn partitions(&self) -> impl Iterator<Item = &Partition> { self.parts.iter().flatten() }
    fn keep(&mut self, partition: Partition) {
        match self.parts.iter_mut().find(|p| p.is_none()) { Some(free) => *free = Some(partition), None => self.left_out += 1 }
    }
    fn with(mut self, problem: &'static str) -> Self { self.problem = Some(problem); self }
}

fn u16_at(s: &[u8], at: usize) -> u16 { u16::from_le_bytes([s[at], s[at + 1]]) }
fn u32_at(s: &[u8], at: usize) -> u32 { u32::from_le_bytes(s[at..at + 4].try_into().unwrap()) }
fn u64_at(s: &[u8], at: usize) -> u64 { u64::from_le_bytes(s[at..at + 8].try_into().unwrap()) }

// CRC-32 (IEEE 802.3), as GPT's header and entries carry it; `crc` is the value so far, 0 at the start.
fn crc32(crc: u32, data: &[u8]) -> u32 {
    let mut crc = !crc;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 { crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg()); }
    }
    !crc
}

// One FAT volume over the whole disk: a jump, 512-byte sectors, a power of two per cluster, one or two FATs.
fn fat_volume(s: &[u8; SECTOR]) -> bool {
    matches!(s[0], 0xEB | 0xE9) && u16_at(s, 11) == 512 && s[13].is_power_of_two() && matches!(s[16], 1 | 2)
}

/// The table of a disk of `sectors`, read with `read` (sector number, buffer; false when it could not be read).
pub fn read_table(sectors: u64, read: &mut dyn FnMut(u64, &mut [u8; SECTOR]) -> bool) -> Table {
    let mut first = [0u8; SECTOR];
    if !read(0, &mut first) { return Table::new(Scheme::None).with("SECTOR 0 COULD NOT BE READ"); }
    if first[510] != 0x55 || first[511] != 0xAA || fat_volume(&first) { return Table::new(Scheme::None); }
    if (0..4).any(|i| first[446 + i * 16 + 4] == 0xEE) { return gpt(sectors, read); }
    let mut table = Table::new(Scheme::Mbr);
    let signature = u32_at(&first, 440);
    for i in 0..4 {
        let entry = &first[446 + i * 16..446 + (i + 1) * 16];
        let (kind, start, count) = (entry[4], u32_at(entry, 8) as u64, u32_at(entry, 12) as u64);
        if kind == 0 { continue; }
        if start == 0 || count == 0 || start + count > sectors { table.problem = Some("A PARTITION OUTSIDE THE DISK, LEFT OUT"); continue; }
        let number = i as u8 + 1;
        table.keep(Partition { number, kind: Kind::Mbr(kind), first: start, count, id: Id::Mbr { signature, number }, label: [0; 36], label_len: 0 });
    }
    table
}

fn gpt(sectors: u64, read: &mut dyn FnMut(u64, &mut [u8; SECTOR]) -> bool) -> Table {
    let table = Table::new(Scheme::Gpt);
    let mut header = [0u8; SECTOR];
    if !read(1, &mut header) { return table.with("GPT HEADER COULD NOT BE READ"); }
    let size = u32_at(&header, 12) as usize;
    if &header[..8] != b"EFI PART" || !(92..=SECTOR).contains(&size) || u64_at(&header, 24) != 1 { return table.with("NO GPT HEADER BEHIND THE PROTECTIVE MBR"); }
    let stated = u32_at(&header, 16);
    let mut zeroed = header;
    zeroed[16..20].fill(0);
    if crc32(0, &zeroed[..size]) != stated { return table.with("GPT HEADER CRC WRONG"); }
    let (at, entries, entry_size, entries_crc) = (u64_at(&header, 72), u32_at(&header, 80), u32_at(&header, 84) as usize, u32_at(&header, 88));
    if !(128..=SECTOR).contains(&entry_size) || !entry_size.is_power_of_two() { return table.with("GPT ENTRY SIZE NOT 128 TO 512"); }
    if entries > GPT_ENTRIES_MAX { return table.with("GPT HAS MORE THAN 256 ENTRIES"); }
    let per = SECTOR / entry_size;
    let span = (entries as u64).div_ceil(per as u64);
    if at < 2 || at.saturating_add(span) > sectors { return table.with("GPT ENTRIES OUTSIDE THE DISK"); }
    // The entries' CRC first, over all of them, so that nothing is taken from an array that does not match it.
    let mut crc = 0;
    let mut sector = [0u8; SECTOR];
    for k in 0..span {
        if !read(at + k, &mut sector) { return table.with("GPT ENTRIES COULD NOT BE READ"); }
        let here = (entries as usize - k as usize * per).min(per);
        crc = crc32(crc, &sector[..here * entry_size]);
    }
    if crc != entries_crc { return table.with("GPT ENTRIES CRC WRONG"); }
    let mut table = table;
    for k in 0..span {
        if !read(at + k, &mut sector) { return table.with("GPT ENTRIES COULD NOT BE READ"); }
        for j in 0..(entries as usize - k as usize * per).min(per) {
            let entry = &sector[j * entry_size..j * entry_size + 128];
            let kind: [u8; 16] = entry[..16].try_into().unwrap();
            if kind == [0; 16] { continue; }
            let (start, last) = (u64_at(entry, 32), u64_at(entry, 40));
            if start < 2 || last < start || last >= sectors { table.problem = Some("A PARTITION OUTSIDE THE DISK, LEFT OUT"); continue; }
            let mut label = [0u8; 36];
            let mut len = 0;
            for c in (0..36).map(|i| u16_at(entry, 56 + i * 2)).take_while(|&c| c != 0) {
                label[len] = if (0x20..0x7F).contains(&c) { c as u8 } else { b'?' };
                len += 1;
            }
            let number = (k as usize * per + j + 1).min(u8::MAX as usize) as u8;
            let id = Id::Gpt(entry[16..32].try_into().unwrap());
            table.keep(Partition { number, kind: Kind::Gpt(kind), first: start, count: last - start + 1, id, label, label_len: len as u8 });
        }
    }
    table
}

/// Why a partition is the store's: its type and superblock both, the owner's word, or both of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why { Found, Named, FoundAndNamed }

/// Whether `partition`, whose first sector is `first`, is a candidate for the store: the store's type with the store's
/// superblock (no label is matched), or the partition `named` in `store.txt`, which may be blank.
pub fn candidate(partition: &Partition, first: &[u8; SECTOR], named: Option<Id>) -> Option<Why> {
    let found = partition.store_type() && first.starts_with(STORE_MAGIC);
    match (found, named == Some(partition.id)) {
        (true, true) => Some(Why::FoundAndNamed),
        (true, false) => Some(Why::Found),
        (false, true) => Some(Why::Named),
        (false, false) => None,
    }
}

fn hex(c: u8) -> Option<u8> { (c as char).to_digit(16).map(|d| d as u8) }

fn hex_bytes(text: &[u8], out: &mut [u8]) -> bool {
    if text.len() != out.len() * 2 { return false; }
    for (byte, pair) in out.iter_mut().zip(text.chunks(2)) {
        let (Some(high), Some(low)) = (hex(pair[0]), hex(pair[1])) else { return false };
        *byte = high << 4 | low;
    }
    true
}

/// A PARTUUID's text: GPT's `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`, or an MBR's `SSSSSSSS-NN` (the disk's signature and
/// the partition's number, in hex), in either case of letters.
pub fn parse_id(text: &[u8]) -> Option<Id> {
    let mut parts: [&[u8]; 5] = [&[]; 5];
    let mut n = 0;
    for part in text.split(|&c| c == b'-') {
        if n == 5 { return None; }
        parts[n] = part;
        n += 1;
    }
    match n {
        2 => {
            let (mut signature, mut number) = ([0u8; 4], [0u8; 1]);
            if !hex_bytes(parts[0], &mut signature) || !hex_bytes(parts[1], &mut number) || number[0] == 0 { return None; }
            Some(Id::Mbr { signature: u32::from_be_bytes(signature), number: number[0] })
        }
        5 => {
            let mut fields = [[0u8; 6]; 5];
            for (i, size) in [4, 2, 2, 2, 6].into_iter().enumerate() { if !hex_bytes(parts[i], &mut fields[i][..size]) { return None; } }
            // The first three fields are stored little-endian, the last two as written.
            let mut guid = [0u8; 16];
            for i in 0..4 { guid[i] = fields[0][3 - i]; }
            guid[4..6].copy_from_slice(&[fields[1][1], fields[1][0]]);
            guid[6..8].copy_from_slice(&[fields[2][1], fields[2][0]]);
            guid[8..10].copy_from_slice(&fields[3][..2]);
            guid[10..16].copy_from_slice(&fields[4]);
            Some(Id::Gpt(guid))
        }
        _ => None,
    }
}

/// `store.txt`'s word: the first line `partition <PARTUUID>`; blank lines and those starting with `#` are skipped. Ok(None)
/// without such a line; Err when the first non-comment line is another word or its PARTUUID does not parse.
pub fn named(text: &[u8]) -> Result<Option<Id>, &'static str> {
    for line in text.split(|&c| c == b'\n') {
        let line = line.trim_ascii();
        if line.is_empty() || line[0] == b'#' { continue; }
        let mut words = line.split(|c| c.is_ascii_whitespace()).filter(|w| !w.is_empty());
        if words.next() != Some(b"partition".as_slice()) { return Err("STORE.TXT: NOT `partition <PARTUUID>`"); }
        let (Some(id), None) = (words.next(), words.next()) else { return Err("STORE.TXT: NOT `partition <PARTUUID>`") };
        return parse_id(id).map(Some).ok_or("STORE.TXT: NOT A PARTUUID (GPT: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx; MBR: SSSSSSSS-NN)");
    }
    Ok(None)
}

const DIGITS: &[u8; 16] = b"0123456789abcdef";

fn put_hex(out: &mut [u8], at: &mut usize, bytes: impl Iterator<Item = u8>) {
    for b in bytes { out[*at] = DIGITS[(b >> 4) as usize]; out[*at + 1] = DIGITS[(b & 15) as usize]; *at += 2; }
}

/// A GUID in GPT's byte order as text, lower-case.
pub fn guid_text(guid: &[u8; 16], out: &mut [u8; ID_TEXT]) -> usize {
    let mut at = 0;
    put_hex(out, &mut at, guid[..4].iter().rev().copied()); out[at] = b'-'; at += 1;
    put_hex(out, &mut at, guid[4..6].iter().rev().copied()); out[at] = b'-'; at += 1;
    put_hex(out, &mut at, guid[6..8].iter().rev().copied()); out[at] = b'-'; at += 1;
    put_hex(out, &mut at, guid[8..10].iter().copied()); out[at] = b'-'; at += 1;
    put_hex(out, &mut at, guid[10..].iter().copied());
    at
}

/// A PARTUUID as text, as `parse_id` reads it back.
pub fn id_text(id: &Id, out: &mut [u8; ID_TEXT]) -> usize {
    match id {
        Id::Gpt(guid) => guid_text(guid, out),
        Id::Mbr { signature, number } => {
            let mut at = 0;
            put_hex(out, &mut at, signature.to_be_bytes().into_iter()); out[at] = b'-'; at += 1;
            put_hex(out, &mut at, [*number].into_iter());
            at
        }
    }
}
