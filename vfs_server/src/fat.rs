//! FAT12/16/32 with long names (UTF-16 <-> UTF-8): reading, writing, directories and formatting. Sectors come through
//! `Sectors`: the block driver behind a write-back cache in the service, an image in memory in tests/fat_host.rs.
//! A change writes file data, then the FAT (every copy), then the directory entry into the cache; from the first change
//! until `flush` the volume is marked dirty in FAT[1] (FAT16/32), so a check after a power loss knows to look.
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

pub const SECTOR: usize = 512;
pub const ATTR_READ_ONLY: u8 = 0x01;
pub const ATTR_HIDDEN: u8 = 0x02;
pub const ATTR_SYSTEM: u8 = 0x04;
pub const ATTR_VOLUME: u8 = 0x08;
pub const ATTR_DIRECTORY: u8 = 0x10;
pub const ATTR_ARCHIVE: u8 = 0x20;
const ATTR_LONG: u8 = 0x0F;
const NAME_UNITS: usize = 255;
// UTF-16 units of a long-name entry.
const LONG_CHARS: [usize; 13] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];

/// Where the volume's sectors come from.
pub trait Sectors {
    fn read(&mut self, lba: u32, out: &mut [u8; SECTOR]) -> bool;
    fn write(&mut self, lba: u32, data: &[u8; SECTOR]) -> bool;
    /// Writes everything cached to the medium.
    fn flush(&mut self) -> bool;
    fn sectors(&self) -> u64;
    fn writable(&self) -> bool;
    /// Forgets cached sectors without writing them (before the medium is overwritten as a whole).
    fn discard(&mut self) {}
}

/// A volume can be mounted through a borrowed disk (`Volume::reformat` re-reads its geometry that way).
impl<S: Sectors> Sectors for &mut S {
    fn read(&mut self, lba: u32, out: &mut [u8; SECTOR]) -> bool { (**self).read(lba, out) }
    fn write(&mut self, lba: u32, data: &[u8; SECTOR]) -> bool { (**self).write(lba, data) }
    fn flush(&mut self) -> bool { (**self).flush() }
    fn sectors(&self) -> u64 { (**self).sectors() }
    fn writable(&self) -> bool { (**self).writable() }
    fn discard(&mut self) { (**self).discard() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { NotFound, Exists, NotEmpty, Invalid, NoSpace, ReadOnly, Io, Name, NotDirectory, IsDirectory }
pub type Result<T> = core::result::Result<T, Error>;

/// Position of a 32-byte directory entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct At { pub lba: u32, pub offset: u16 }

/// A file or directory: first cluster (0: an empty file, or the fixed root of FAT12/16), size, attributes, FAT
/// modification stamp (date << 16 | time) and where its directory entry is (none for the root).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node { pub cluster: u32, pub size: u32, pub attributes: u8, pub modified: u32, pub entry: Option<At> }

impl Node {
    pub fn is_dir(&self) -> bool { self.attributes & ATTR_DIRECTORY != 0 }
}

/// A directory entry: its name, short name and every slot it occupies (long-name entries, then the short one).
#[derive(Clone, Debug)]
pub struct Entry { pub name: String, pub short: [u8; 11], pub node: Node, pub slots: Vec<At> }

/// A FAT stamp (date << 16 | time) from days since 2000-01-01 and seconds since midnight.
pub fn stamp(days: u32, seconds: u32) -> u32 {
    let (year, month, day) = civil(days as i64 + 10957); // days since 1970-01-01
    let date = ((year.clamp(1980, 2107) - 1980) as u32) << 9 | month << 5 | day;
    let time = (seconds / 3600) << 11 | (seconds / 60 % 60) << 5 | (seconds % 60) / 2;
    date << 16 | time
}

// Gregorian date from days since 1970-01-01 (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + (month <= 2) as i64, month, day)
}

fn u16_at(b: &[u8], at: usize) -> u32 { u16::from_le_bytes([b[at], b[at + 1]]) as u32 }
fn u32_at(b: &[u8], at: usize) -> u32 { u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) }
fn put16(b: &mut [u8], at: usize, v: u32) { b[at..at + 2].copy_from_slice(&(v as u16).to_le_bytes()); }
fn put32(b: &mut [u8], at: usize, v: u32) { b[at..at + 4].copy_from_slice(&v.to_le_bytes()); }

fn checksum(short: &[u8]) -> u8 { short[..11].iter().fold(0u8, |sum, &c| sum.rotate_right(1).wrapping_add(c)) }

/// Case-insensitive comparison of names (Unicode lower case).
pub fn same_name(a: &str, b: &str) -> bool {
    (a.len() == b.len() && a.eq_ignore_ascii_case(b)) || a.chars().flat_map(char::to_lowercase).eq(b.chars().flat_map(char::to_lowercase))
}

/// A name a FAT long-name entry can hold.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && name.encode_utf16().count() <= NAME_UNITS
        && !name.ends_with('.') && !name.ends_with(' ')
        && !name.chars().any(|c| (c as u32) < 0x20 || "\"*/:<>?\\|".contains(c))
}

fn short_char(c: u8) -> bool { c.is_ascii_uppercase() || c.is_ascii_digit() || b"$%'-_@~`!(){}^#&".contains(&c) }

/// The short name for `name`: either the name itself (an 8.3 name without capitals, stored in upper case with the NT
/// lower-case flags, no long name needed) or a unique `BASIS~N.EXT` alias next to a long name. Returns (short name,
/// long name needed, NT case flags).
fn short_name(name: &str, taken: &dyn Fn(&[u8; 11]) -> bool) -> Option<([u8; 11], bool, u8)> {
    let (base, ext) = match name.rfind('.') { Some(i) if i > 0 => (&name[..i], &name[i + 1..]), _ => (name, "") };
    let plain = name.is_ascii() && !name.bytes().any(|c| c.is_ascii_uppercase()) && !base.contains('.') && (1..=8).contains(&base.len()) && ext.len() <= 3
        && base.bytes().chain(ext.bytes()).all(|c| short_char(c.to_ascii_uppercase()));
    if plain {
        let mut short = [b' '; 11];
        for (i, c) in base.bytes().enumerate() { short[i] = c.to_ascii_uppercase(); }
        for (i, c) in ext.bytes().enumerate() { short[8 + i] = c.to_ascii_uppercase(); }
        let flags = if base.bytes().any(|c| c.is_ascii_lowercase()) { 0x08 } else { 0 } | if ext.bytes().any(|c| c.is_ascii_lowercase()) { 0x10 } else { 0 };
        if !taken(&short) { return Some((short, false, flags)); }
    }
    let basis = |part: &str, max: usize| -> Vec<u8> {
        part.chars().filter(|&c| c != ' ' && c != '.').map(|c| if c.is_ascii() && short_char((c as u8).to_ascii_uppercase()) { (c as u8).to_ascii_uppercase() } else { b'_' }).take(max).collect()
    };
    let (base, ext) = (basis(base, 8), basis(ext, 3));
    let base = if base.is_empty() { vec![b'_'] } else { base };
    for n in 1..1_000_000u32 {
        let mut tail = [0u8; 8]; let mut len = 0; let mut v = n;
        let mut digits = [0u8; 7]; let mut d = 0;
        loop { digits[d] = b'0' + (v % 10) as u8; d += 1; v /= 10; if v == 0 { break; } }
        tail[len] = b'~'; len += 1;
        for i in (0..d).rev() { tail[len] = digits[i]; len += 1; }
        let keep = base.len().min(8 - len);
        let mut short = [b' '; 11];
        short[..keep].copy_from_slice(&base[..keep]);
        short[keep..keep + len].copy_from_slice(&tail[..len]);
        short[8..8 + ext.len()].copy_from_slice(&ext);
        if !taken(&short) { return Some((short, true, 0)); }
    }
    None
}

// The short name as text: lower case, with the dot (how this system shows names without a long name).
fn short_text(raw: &[u8]) -> String {
    let mut name = String::new();
    for &c in raw[..8].iter().filter(|&&c| c != b' ') { name.push(c.to_ascii_lowercase() as char); }
    if raw[8] != b' ' { name.push('.'); for &c in raw[8..11].iter().filter(|&&c| c != b' ') { name.push(c.to_ascii_lowercase() as char); } }
    name
}

pub struct Volume<S: Sectors> {
    pub disk: S,
    start: u32, bits: u8, spc: u32, fats: u32, fat_size: u32, fat_start: u32, root_start: u32, root_sectors: u32, data_start: u32,
    root_cluster: u32, clusters: u32, fsinfo: u32, label: [u8; 11],
    changed: bool, dirtied: bool, next_free: u32, free: Option<u32>,
}

impl<S: Sectors> Volume<S> {
    /// Mounts the FAT volume of an MBR partition or of the whole disk; gives the disk back if there is none.
    pub fn mount(mut disk: S) -> core::result::Result<Self, S> {
        let Some(start) = fat_starts(&mut disk).into_iter().flatten().next() else { return Err(disk) };
        Self::mount_at(disk, start)
    }

    /// Mounts the FAT volume that starts at sector `start`.
    pub fn mount_at(mut disk: S, start: u32) -> core::result::Result<Self, S> {
        let mut boot = [0u8; SECTOR];
        if !disk.read(start, &mut boot) || u16_at(&boot, 11) != 512 { return Err(disk); }
        let spc = boot[13] as u32; let reserved = u16_at(&boot, 14); let fats = boot[16] as u32; let root_entries = u16_at(&boot, 17);
        let total = if u16_at(&boot, 19) != 0 { u16_at(&boot, 19) } else { u32_at(&boot, 32) };
        let fat_size = if u16_at(&boot, 22) != 0 { u16_at(&boot, 22) } else { u32_at(&boot, 36) };
        if spc == 0 || !spc.is_power_of_two() || fats == 0 || fat_size == 0 || reserved == 0 { return Err(disk); }
        let root_sectors = (root_entries * 32).div_ceil(512);
        let data = reserved + fats * fat_size + root_sectors;
        let Some(clusters) = total.checked_sub(data).map(|d| d / spc) else { return Err(disk) };
        let bits = if clusters < 4085 { 12 } else if clusters < 65525 { 16 } else { 32 };
        let label_at = if bits == 32 { 71 } else { 43 };
        let mut label = [b' '; 11];
        if boot[if bits == 32 { 66 } else { 38 }] == 0x29 { label.copy_from_slice(&boot[label_at..label_at + 11]); }
        let mut volume = Self { disk, start, bits, spc, fats, fat_size, fat_start: start + reserved, root_start: start + reserved + fats * fat_size, root_sectors,
                                data_start: start + data, root_cluster: if bits == 32 { u32_at(&boot, 44) } else { 0 }, clusters,
                                fsinfo: if bits == 32 { start + u16_at(&boot, 48) } else { 0 }, label, changed: false, dirtied: false, next_free: 2, free: None };
        // The label entry in the root directory wins over the boot sector's copy.
        let root = volume.root();
        if let Ok(sectors) = volume.dir_sectors(&root) {
            'scan: for lba in sectors {
                let mut data = [0u8; SECTOR];
                if !volume.disk.read(lba, &mut data) { break; }
                for raw in data.chunks_exact(32) {
                    if raw[0] == 0 { break 'scan; }
                    if raw[0] != 0xE5 && raw[11] & 0x3F == ATTR_VOLUME { volume.label.copy_from_slice(&raw[..11]); break 'scan; }
                }
            }
        }
        Ok(volume)
    }

    /// Writes a new empty volume over the whole disk and mounts it in place (the RAM disk's `format` request). What was
    /// cached for the old volume is dropped first, so none of it is written over the new one. Nodes of the old volume
    /// mean nothing afterwards; the caller forgets them.
    pub fn reformat(&mut self, label: &str, stamp: u32) -> Result<()> {
        self.disk.discard();
        format(&mut self.disk, label, stamp)?;
        if !self.disk.flush() { return Err(Error::Io); }
        let Ok(fresh) = Volume::mount(&mut self.disk) else { return Err(Error::Io) };
        let Volume { disk: _, start, bits, spc, fats, fat_size, fat_start, root_start, root_sectors, data_start, root_cluster, clusters, fsinfo, label, changed, dirtied, next_free, free } = fresh;
        (self.start, self.bits, self.spc, self.fats, self.fat_size, self.fat_start, self.root_start, self.root_sectors) = (start, bits, spc, fats, fat_size, fat_start, root_start, root_sectors);
        (self.data_start, self.root_cluster, self.clusters, self.fsinfo, self.label, self.changed, self.dirtied, self.next_free, self.free) = (data_start, root_cluster, clusters, fsinfo, label, changed, dirtied, next_free, free);
        Ok(())
    }

    pub fn bits(&self) -> u8 { self.bits }
    pub fn start(&self) -> u32 { self.start }
    pub fn cluster_bytes(&self) -> u32 { self.spc * SECTOR as u32 }
    pub fn total_bytes(&self) -> u64 { self.clusters as u64 * self.cluster_bytes() as u64 }
    pub fn writable(&self) -> bool { self.disk.writable() }
    pub fn label(&self) -> String { String::from_utf8_lossy(&self.label).trim_end().into() }
    pub fn root(&self) -> Node { Node { cluster: if self.bits == 32 { self.root_cluster } else { 0 }, size: 0, attributes: ATTR_DIRECTORY, modified: 0, entry: None } }

    fn eoc(&self) -> u32 { match self.bits { 12 => 0xFFF, 16 => 0xFFFF, _ => 0x0FFF_FFFF } }
    fn is_cluster(&self, value: u32) -> bool { value >= 2 && value < self.clusters + 2 }
    fn sector_of(&self, cluster: u32) -> u32 { self.data_start + (cluster - 2) * self.spc }

    fn read_sector(&mut self, lba: u32) -> Result<[u8; SECTOR]> {
        let mut data = [0u8; SECTOR];
        if self.disk.read(lba, &mut data) { Ok(data) } else { Err(Error::Io) }
    }
    // Every write goes through here, so the first one after a flush marks the volume dirty.
    fn write_sector(&mut self, lba: u32, data: &[u8; SECTOR]) -> Result<()> {
        if !self.disk.writable() { return Err(Error::ReadOnly); }
        self.changing()?;
        if self.disk.write(lba, data) { Ok(()) } else { Err(Error::Io) }
    }

    // Bytes of the first FAT at `offset`.
    fn fat_bytes(&mut self, offset: u32, out: &mut [u8]) -> Result<()> {
        for (i, byte) in out.iter_mut().enumerate() {
            let at = offset + i as u32;
            *byte = self.read_sector(self.fat_start + at / 512)?[(at % 512) as usize];
        }
        Ok(())
    }
    // The same bytes in every FAT copy. The volume is marked dirty before any sector is read: that mark rewrites FAT
    // sector 0, and a copy read before it would overwrite it (175-KRN-0049).
    fn set_fat_bytes(&mut self, offset: u32, bytes: &[u8]) -> Result<()> {
        self.changing()?;
        for copy in 0..self.fats {
            let base = self.fat_start + copy * self.fat_size;
            let mut i = 0;
            while i < bytes.len() {
                let at = offset + i as u32;
                let lba = base + at / 512;
                let mut data = self.read_sector(lba)?;
                while i < bytes.len() && (offset + i as u32) / 512 == at / 512 { data[((offset + i as u32) % 512) as usize] = bytes[i]; i += 1; }
                self.write_sector(lba, &data)?;
            }
        }
        Ok(())
    }

    /// The FAT entry of `cluster`.
    pub fn fat(&mut self, cluster: u32) -> Result<u32> {
        match self.bits {
            12 => { let mut b = [0u8; 2]; self.fat_bytes(cluster + cluster / 2, &mut b)?; let raw = b[0] as u32 | (b[1] as u32) << 8; Ok(if cluster & 1 == 0 { raw & 0xFFF } else { raw >> 4 }) }
            16 => { let mut b = [0u8; 2]; self.fat_bytes(cluster * 2, &mut b)?; Ok(b[0] as u32 | (b[1] as u32) << 8) }
            _ => { let mut b = [0u8; 4]; self.fat_bytes(cluster * 4, &mut b)?; Ok(u32::from_le_bytes(b) & 0x0FFF_FFFF) }
        }
    }

    fn set_fat(&mut self, cluster: u32, value: u32) -> Result<()> {
        match self.bits {
            12 => {
                let at = cluster + cluster / 2; let mut b = [0u8; 2]; self.fat_bytes(at, &mut b)?;
                if cluster & 1 == 0 { b[0] = value as u8; b[1] = (b[1] & 0xF0) | ((value >> 8) as u8 & 0x0F); } else { b[0] = (b[0] & 0x0F) | (value << 4) as u8; b[1] = (value >> 4) as u8; }
                self.set_fat_bytes(at, &b)
            }
            16 => self.set_fat_bytes(cluster * 2, &(value as u16).to_le_bytes()),
            _ => { let mut b = [0u8; 4]; self.fat_bytes(cluster * 4, &mut b)?; let old = u32::from_le_bytes(b); self.set_fat_bytes(cluster * 4, &((old & 0xF000_0000) | (value & 0x0FFF_FFFF)).to_le_bytes()) }
        }
    }

    // The first change since the last flush marks the volume dirty (FAT[1]) and, on FAT32, the free count unknown. The
    // mark is flushed by itself, so it is on the medium before any change it covers (175-KRN-0049); if it fails, the
    // next change tries again.
    fn changing(&mut self) -> Result<()> {
        if self.changed { return Ok(()); }
        if !self.disk.writable() { return Err(Error::ReadOnly); }
        self.changed = true; // first: the writes below come back here
        self.dirtied = true; // a mark that failed half way is cleared by the next flush too
        let marked = self.mark_dirty();
        if marked.is_err() { self.changed = false; }
        marked
    }
    fn mark_dirty(&mut self) -> Result<()> {
        self.set_clean(false)?;
        if self.bits == 32 && self.fsinfo != 0 {
            let mut info = self.read_sector(self.fsinfo)?;
            if u32_at(&info, 0) == 0x4161_5252 && u32_at(&info, 484) == 0x6141_7272 { put32(&mut info, 488, u32::MAX); put32(&mut info, 492, u32::MAX); self.write_sector(self.fsinfo, &info)?; }
        }
        if self.bits != 12 && !self.disk.flush() { return Err(Error::Io); }
        Ok(())
    }

    // The clean bit in FAT[1]: FAT16 bit 15, FAT32 bit 27 (FAT12 has none).
    fn set_clean(&mut self, clean: bool) -> Result<()> {
        let (offset, width, bit) = match self.bits { 16 => (2u32, 2usize, 15), 32 => (4, 4, 27), _ => return Ok(()) };
        let mut b = [0u8; 4];
        self.fat_bytes(offset, &mut b[..width])?;
        let mut value = u32::from_le_bytes(b);
        value = if clean { value | 1 << bit } else { value & !(1 << bit) };
        self.set_fat_bytes(offset, &value.to_le_bytes()[..width])
    }

    /// Writes everything to the medium and marks the volume clean: the clean bit is written only after the rest has
    /// reached the medium, and flushed by itself (175-KRN-0049). After an error the volume stays dirty.
    pub fn flush(&mut self) -> Result<()> {
        if !self.dirtied || self.bits == 12 {
            if !self.disk.flush() { return Err(Error::Io); }
            self.changed = false;
            return Ok(());
        }
        if !self.disk.flush() { return Err(Error::Io); }
        self.changed = true; // the clean mark below is no change to mark dirty for
        if self.set_clean(true).is_ok() && self.disk.flush() { (self.changed, self.dirtied) = (false, false); return Ok(()); }
        // The bit may be clean in the cache: it is marked dirty again now, or before the next change at the latest.
        self.changed = false;
        let _ = self.changing();
        Err(Error::Io)
    }

    /// Free clusters (counted once, then kept up to date).
    pub fn free_clusters(&mut self) -> Result<u32> {
        if let Some(free) = self.free { return Ok(free); }
        // Each FAT sector read once for all its entries, not once an entry (251-KRN-0072).
        let mut window = FatWindow { lba: u32::MAX, data: [0; 2 * SECTOR] };
        let mut free = 0;
        for cluster in 2..self.clusters + 2 { if self.entry_at(&mut window, cluster)? == 0 { free += 1; } }
        self.free = Some(free);
        Ok(free)
    }

    // A free cluster marked as the end of a chain and linked after `previous` (0: none); `zero` clears it.
    fn allocate(&mut self, previous: u32, zero: bool) -> Result<u32> {
        let cluster = self.free_run(1)?[0];
        if zero { for s in 0..self.spc { let lba = self.sector_of(cluster) + s; self.write_sector(lba, &[0; SECTOR])?; } }
        self.link_chain(previous, &[cluster])?;
        Ok(cluster)
    }

    // `count` free clusters, found without taking any: NoSpace changes nothing (175-KRN-0047).
    fn free_run(&mut self, count: usize) -> Result<Vec<u32>> {
        let total = self.clusters;
        let mut found = Vec::new();
        for i in 0..total {
            if found.len() == count { break; }
            let cluster = 2 + (self.next_free.saturating_sub(2) + i) % total;
            if self.fat(cluster)? == 0 { found.push(cluster); }
        }
        if found.len() < count { return Err(Error::NoSpace); }
        Ok(found)
    }

    // Free `clusters` made one chain after `previous` (0: none): the end marked first, `previous` linked last. On an
    // error the clusters are freed again and `previous` keeps its end.
    fn link_chain(&mut self, previous: u32, clusters: &[u32]) -> Result<()> {
        let eoc = self.eoc();
        let mut marked = 0;
        let mut result = Ok(());
        for k in (0..clusters.len()).rev() {
            let next = clusters.get(k + 1).copied().unwrap_or(eoc);
            if let Err(error) = self.set_fat(clusters[k], next) { result = Err(error); break; }
            marked += 1;
        }
        if result.is_ok() && previous != 0 {
            result = self.set_fat(previous, clusters[0]);
            if result.is_err() && self.set_fat(previous, eoc).is_err() { self.free = None; }
        }
        if let Err(error) = result {
            // The one that failed too: its first FAT copy may hold the mark.
            for &cluster in clusters.iter().rev().take(marked + 1) { if self.set_fat(cluster, 0).is_err() { self.free = None; } }
            return Err(error);
        }
        if let Some(&last) = clusters.last() { self.next_free = last + 1; }
        if let Some(free) = self.free.as_mut() { *free -= clusters.len() as u32; }
        Ok(())
    }

    // After a failed write, `node`'s chain is cut back to the `length` clusters it had (all freed for 0) and `node` is
    // as it was (175-KRN-0047). Errors here leave the volume dirty for a check.
    fn cut_back(&mut self, node: &mut Node, before: Node, length: usize) {
        if node.cluster >= 2 {
            match self.chain(node.cluster) {
                Ok(chain) if chain.len() > length => {
                    let eoc = self.eoc();
                    let cut = if length == 0 { self.free_chain(chain[0]) } else { self.set_fat(chain[length - 1], eoc).and_then(|_| self.free_chain(chain[length])) };
                    if cut.is_err() { self.free = None; }
                }
                Ok(_) => {}
                Err(_) => self.free = None,
            }
        }
        *node = before;
    }

    fn free_chain(&mut self, first: u32) -> Result<()> {
        let mut cluster = first;
        let mut steps = 0;
        while self.is_cluster(cluster) && steps <= self.clusters {
            let next = self.fat(cluster)?;
            self.set_fat(cluster, 0)?;
            if let Some(free) = self.free.as_mut() { *free += 1; }
            cluster = next; steps += 1;
        }
        Ok(())
    }

    // Clusters of a chain, in order.
    fn chain(&mut self, first: u32) -> Result<Vec<u32>> {
        let mut out = Vec::new();
        let mut cluster = first;
        while self.is_cluster(cluster) {
            if out.len() as u32 > self.clusters { return Err(Error::Io); } // a loop in the FAT
            out.push(cluster);
            cluster = self.fat(cluster)?;
        }
        Ok(out)
    }

    // Sectors of a directory.
    fn dir_sectors(&mut self, dir: &Node) -> Result<Vec<u32>> {
        if dir.cluster == 0 { return Ok((self.root_start..self.root_start + self.root_sectors).collect()); }
        let spc = self.spc;
        let chain = self.chain(dir.cluster)?;
        Ok(chain.into_iter().flat_map(|c| { let first = self.data_start + (c - 2) * spc; first..first + spc }).collect())
    }

    /// Entries of a directory (without `.`, `..` and the volume label).
    pub fn list(&mut self, dir: &Node) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        self.scan(dir, &mut |entry| { out.push(entry.clone()); true })?;
        Ok(out)
    }

    fn scan(&mut self, dir: &Node, visit: &mut dyn FnMut(&Entry) -> bool) -> Result<()> {
        if !dir.is_dir() { return Err(Error::NotDirectory); }
        let mut units = [0xFFFFu16; 260];
        let mut long: Option<u8> = None; // checksum of the long name being collected
        let mut slots: Vec<At> = Vec::new();
        for lba in self.dir_sectors(dir)? {
            let data = self.read_sector(lba)?;
            for (index, raw) in data.chunks_exact(32).enumerate() {
                let at = At { lba, offset: (index * 32) as u16 };
                if raw[0] == 0 { return Ok(()); }
                if raw[0] == 0xE5 { long = None; slots.clear(); continue; }
                if raw[11] & 0x3F == ATTR_LONG {
                    let order = raw[0] & 0x1F;
                    if raw[0] & 0x40 != 0 { units = [0xFFFF; 260]; long = Some(raw[13]); slots.clear(); }
                    if long == Some(raw[13]) && (1..=20).contains(&order) {
                        for (i, &o) in LONG_CHARS.iter().enumerate() { units[(order as usize - 1) * 13 + i] = u16_at(raw, o) as u16; }
                        slots.push(at);
                    } else { long = None; slots.clear(); }
                    continue;
                }
                if raw[11] & ATTR_VOLUME != 0 || raw[0] == b'.' { long = None; slots.clear(); continue; }
                let mut short = [0u8; 11]; short.copy_from_slice(&raw[..11]);
                if short[0] == 0x05 { short[0] = 0xE5; }
                let name: String = if long == Some(checksum(raw)) {
                    let end = units.iter().position(|&u| u == 0 || u == 0xFFFF).unwrap_or(units.len());
                    char::decode_utf16(units[..end].iter().copied()).map(|c| c.unwrap_or('\u{FFFD}')).collect()
                } else { slots.clear(); short_text(&short) };
                let cluster = u16_at(raw, 26) | if self.bits == 32 { u16_at(raw, 20) << 16 } else { 0 };
                let node = Node { cluster, size: u32_at(raw, 28), attributes: raw[11], modified: u16_at(raw, 24) << 16 | u16_at(raw, 22), entry: Some(at) };
                slots.push(at);
                let entry = Entry { name, short, node, slots: core::mem::take(&mut slots) };
                long = None;
                if !visit(&entry) { return Ok(()); }
            }
        }
        Ok(())
    }

    /// The entry called `name` (ignoring case, long or short name) in `dir`.
    pub fn find(&mut self, dir: &Node, name: &str) -> Result<Entry> {
        let mut found = None;
        self.scan(dir, &mut |entry| { if same_name(&entry.name, name) || same_name(&short_text(&entry.short), name) { found = Some(entry.clone()); false } else { true } })?;
        found.ok_or(Error::NotFound)
    }

    /// The node at `path` (names separated by `/`) below `dir`.
    #[allow(dead_code)] // the service walks paths itself (zones); the host tests use this
    pub fn lookup(&mut self, dir: &Node, path: &str) -> Result<Node> {
        let mut node = *dir;
        for part in path.split('/').filter(|p| !p.is_empty()) {
            if !node.is_dir() { return Err(Error::NotDirectory); }
            node = self.find(&node, part)?.node;
        }
        Ok(node)
    }

    /// Reads from `offset`; returns the bytes read (fewer at the end of the file).
    pub fn read(&mut self, node: &Node, offset: u32, out: &mut [u8]) -> Result<usize> {
        if node.is_dir() { return Err(Error::IsDirectory); }
        if offset >= node.size || node.cluster < 2 { return Ok(0); }
        let want = out.len().min((node.size - offset) as usize);
        let per = self.cluster_bytes() as usize;
        let chain = self.chain(node.cluster)?;
        let mut done = 0;
        while done < want {
            let position = offset as usize + done;
            let Some(&cluster) = chain.get(position / per) else { break };
            let within = position % per;
            let data = self.read_sector(self.sector_of(cluster) + (within / SECTOR) as u32)?;
            let at = within % SECTOR; let take = (SECTOR - at).min(want - done);
            out[done..done + take].copy_from_slice(&data[at..at + take]);
            done += take;
        }
        Ok(done)
    }

    // Writes `node`'s first cluster, size, attributes and stamp into its directory entry.
    fn store(&mut self, node: &Node) -> Result<()> {
        let Some(at) = node.entry else { return Ok(()) };
        let mut data = self.read_sector(at.lba)?;
        let raw = &mut data[at.offset as usize..at.offset as usize + 32];
        raw[11] = node.attributes;
        put16(raw, 26, node.cluster & 0xFFFF);
        if self.bits == 32 { put16(raw, 20, node.cluster >> 16); }
        put32(raw, 28, if node.is_dir() { 0 } else { node.size });
        put16(raw, 22, node.modified & 0xFFFF); put16(raw, 24, node.modified >> 16);
        put16(raw, 18, node.modified >> 16); // access date
        self.write_sector(at.lba, &data)
    }

    // Writes bytes of a file without changing its size; the chain grows to cover them.
    fn write_raw(&mut self, node: &mut Node, offset: u32, data: &[u8]) -> Result<()> {
        if data.is_empty() { return Ok(()); }
        let per = self.cluster_bytes() as usize;
        let end = offset as usize + data.len();
        let mut chain = if node.cluster >= 2 { self.chain(node.cluster)? } else { Vec::new() };
        // All the clusters needed are found before any is linked.
        if chain.len() * per < end {
            let fresh = self.free_run(end.div_ceil(per) - chain.len())?;
            self.link_chain(chain.last().copied().unwrap_or(0), &fresh)?;
            if chain.is_empty() { node.cluster = fresh[0]; }
            chain.extend(fresh);
        }
        let mut done = 0;
        while done < data.len() {
            let position = offset as usize + done;
            let cluster = chain[position / per];
            let within = position % per;
            let lba = self.sector_of(cluster) + (within / SECTOR) as u32;
            let at = within % SECTOR; let take = (SECTOR - at).min(data.len() - done);
            let mut sector = if take == SECTOR { [0u8; SECTOR] } else { self.read_sector(lba)? };
            sector[at..at + take].copy_from_slice(&data[done..done + take]);
            self.write_sector(lba, &sector)?;
            done += take;
        }
        Ok(())
    }

    fn zeros(&mut self, node: &mut Node, from: u32, to: u32) -> Result<()> {
        let block = [0u8; 4096];
        let mut at = from;
        while at < to { let n = ((to - at) as usize).min(block.len()); self.write_raw(node, at, &block[..n])?; at += n as u32; }
        Ok(())
    }

    // The clusters of `node`'s chain when a write ending at `end` may grow it, for `cut_back`; None when it cannot grow.
    fn grows(&mut self, node: &Node, end: u32) -> Result<Option<usize>> {
        let per = self.cluster_bytes();
        if node.cluster >= 2 && end <= node.size.div_ceil(per) * per { return Ok(None); }
        Ok(Some(if node.cluster >= 2 { self.chain(node.cluster)?.len() } else { 0 }))
    }

    /// Writes `data` at `offset` (a gap after the end reads as zeros); the size, stamp and archive bit follow. A write
    /// that fails leaves the file as it was, its chain cut back (175-KRN-0047).
    pub fn write(&mut self, node: &mut Node, offset: u32, data: &[u8], stamp: u32) -> Result<usize> {
        if node.is_dir() { return Err(Error::IsDirectory); }
        if !self.disk.writable() { return Err(Error::ReadOnly); }
        let end = offset.checked_add(data.len() as u32).ok_or(Error::NoSpace)?;
        let (before, length) = (*node, self.grows(node, end)?);
        let result = (|| {
            if offset > node.size { let size = node.size; self.zeros(node, size, offset)?; }
            self.write_raw(node, offset, data)?;
            node.size = node.size.max(end);
            node.modified = stamp;
            node.attributes |= ATTR_ARCHIVE;
            self.store(node)
        })();
        if let Err(error) = result {
            match length { Some(length) => self.cut_back(node, before, length), None => *node = before }
            return Err(error);
        }
        Ok(data.len())
    }

    /// Writes bytes inside a file in place: its size, clusters and directory entry stay as they are (a boot record,
    /// 351-UPD-0008). Bytes past its end are refused.
    pub fn overwrite(&mut self, node: &Node, offset: u32, data: &[u8]) -> Result<()> {
        if node.is_dir() { return Err(Error::IsDirectory); }
        if !self.disk.writable() { return Err(Error::ReadOnly); }
        if offset.checked_add(data.len() as u32).is_none_or(|end| end > node.size) { return Err(Error::Invalid); }
        self.write_raw(&mut node.clone(), offset, data)
    }

    /// Sets the size of a file: clusters past it are freed, a longer file reads zeros.
    pub fn truncate(&mut self, node: &mut Node, size: u32, stamp: u32) -> Result<()> {
        if node.is_dir() { return Err(Error::IsDirectory); }
        if !self.disk.writable() { return Err(Error::ReadOnly); }
        if size > node.size {
            // Growing: a failure leaves the file as it was (175-KRN-0047).
            let (before, length) = (*node, self.grows(node, size)?);
            let old = node.size;
            let grown = (|| { self.zeros(node, old, size)?; node.size = size; node.modified = stamp; node.attributes |= ATTR_ARCHIVE; self.store(node) })();
            if grown.is_err() { match length { Some(length) => self.cut_back(node, before, length), None => *node = before } }
            return grown;
        }
        else if node.cluster >= 2 {
            let keep = size.div_ceil(self.cluster_bytes());
            if keep == 0 { let first = node.cluster; self.free_chain(first)?; node.cluster = 0; }
            else {
                let chain = self.chain(node.cluster)?;
                if let (Some(&last), Some(&next)) = (chain.get(keep as usize - 1), chain.get(keep as usize)) {
                    let eoc = self.eoc(); self.set_fat(last, eoc)?; self.free_chain(next)?;
                }
            }
        }
        node.size = size;
        node.modified = stamp;
        node.attributes |= ATTR_ARCHIVE;
        self.store(node)
    }

    // `count` free consecutive slots in `dir`, growing it when needed (not the fixed root of FAT12/16).
    fn free_slots(&mut self, dir: &Node, count: usize) -> Result<Vec<At>> {
        let mut run: Vec<At> = Vec::new();
        let mut sectors = self.dir_sectors(dir)?;
        let mut index = 0;
        loop {
            while index < sectors.len() {
                let lba = sectors[index];
                let data = self.read_sector(lba)?;
                for (i, raw) in data.chunks_exact(32).enumerate() {
                    if raw[0] == 0 || raw[0] == 0xE5 {
                        run.push(At { lba, offset: (i * 32) as u16 });
                        if run.len() == count {
                            // Slots taken from the free tail: the slot after them must still mark the end.
                            if raw[0] == 0 { self.end_after(&sectors, index, i)?; }
                            return Ok(run);
                        }
                    } else { run.clear(); }
                }
                index += 1;
            }
            if dir.cluster == 0 { return Err(Error::NoSpace); }
            let last = *self.chain(dir.cluster)?.last().ok_or(Error::Io)?;
            let cluster = self.allocate(last, true)?;
            let first = self.sector_of(cluster);
            sectors.extend(first..first + self.spc);
        }
    }

    fn end_after(&mut self, sectors: &[u32], index: usize, i: usize) -> Result<()> {
        let (lba, entry) = if i + 1 < SECTOR / 32 { (sectors[index], i + 1) } else if index + 1 < sectors.len() { (sectors[index + 1], 0) } else { return Ok(()) };
        let mut data = self.read_sector(lba)?;
        if data[entry * 32] != 0 { data[entry * 32] = 0; self.write_sector(lba, &data)?; }
        Ok(())
    }

    // Writes the entries of `name` (the long name if needed, then the short one) for `node` into `dir`; returns their
    // slots. `own`: a short name that does not count as taken (the entry being renamed keeps its alias). On an error the
    // slots written are freed again.
    fn link(&mut self, dir: &Node, name: &str, node: &mut Node, own: Option<[u8; 11]>) -> Result<Vec<At>> {
        let mut shorts: Vec<[u8; 11]> = Vec::new();
        self.scan(dir, &mut |e| { if Some(e.short) != own { shorts.push(e.short); } true })?;
        let (short, long, case) = short_name(name, &|s| shorts.contains(s)).ok_or(Error::Name)?;
        let units: Vec<u16> = name.encode_utf16().collect();
        let parts = if long { units.len().div_ceil(13) } else { 0 };
        let slots = self.free_slots(dir, parts + 1)?;
        let sum = checksum(&short);
        for (k, at) in slots.iter().enumerate() {
            let mut data = self.read_sector(at.lba)?;
            let raw = &mut data[at.offset as usize..at.offset as usize + 32];
            raw.fill(0);
            if k < parts {
                let order = parts - k; // the last part comes first
                raw[0] = order as u8 | if k == 0 { 0x40 } else { 0 };
                raw[11] = ATTR_LONG; raw[13] = sum;
                for (i, &o) in LONG_CHARS.iter().enumerate() {
                    let u = (order - 1) * 13 + i;
                    let unit = if u < units.len() { units[u] } else if u == units.len() { 0 } else { 0xFFFF };
                    put16(raw, o, unit as u32);
                }
            } else {
                raw[..11].copy_from_slice(&short);
                if raw[0] == 0xE5 { raw[0] = 0x05; }
                raw[12] = case;
                put16(raw, 14, node.modified & 0xFFFF); put16(raw, 16, node.modified >> 16); // created
            }
            if let Err(error) = self.write_sector(at.lba, &data) { let _ = self.unlink_slots(&slots[..k]); return Err(error); }
        }
        let before = node.entry;
        node.entry = slots.last().copied();
        if let Err(error) = self.store(node) { node.entry = before; let _ = self.unlink_slots(&slots); return Err(error); }
        Ok(slots)
    }

    fn unlink(&mut self, entry: &Entry) -> Result<()> { self.unlink_slots(&entry.slots) }

    // Marks the slots free; if a write fails, those already marked are given their first byte back.
    fn unlink_slots(&mut self, slots: &[At]) -> Result<()> {
        let mut marked: Vec<(At, u8)> = Vec::new();
        for at in slots {
            let mut data = self.read_sector(at.lba)?;
            let first = data[at.offset as usize];
            data[at.offset as usize] = 0xE5;
            if let Err(error) = self.write_sector(at.lba, &data) {
                for (at, first) in marked.into_iter().rev() {
                    if let Ok(mut data) = self.read_sector(at.lba) { data[at.offset as usize] = first; let _ = self.write_sector(at.lba, &data); }
                }
                return Err(error);
            }
            marked.push((*at, first));
        }
        Ok(())
    }

    /// Creates a file (empty) or a directory (with `.` and `..`) called `name` in `dir`.
    pub fn create(&mut self, dir: &Node, name: &str, directory: bool, stamp: u32) -> Result<Node> {
        if !dir.is_dir() { return Err(Error::NotDirectory); }
        if !valid_name(name) { return Err(Error::Name); }
        if !self.disk.writable() { return Err(Error::ReadOnly); }
        match self.find(dir, name) { Ok(_) => return Err(Error::Exists), Err(Error::NotFound) => {} Err(e) => return Err(e) }
        let mut node = Node { cluster: 0, size: 0, attributes: if directory { ATTR_DIRECTORY } else { ATTR_ARCHIVE }, modified: stamp, entry: None };
        if directory {
            node.cluster = self.allocate(0, true)?;
            let mut data = [0u8; SECTOR];
            for (k, dots) in [*b".          ", *b"..         "].iter().enumerate() {
                let raw = &mut data[k * 32..k * 32 + 32];
                raw[..11].copy_from_slice(dots); raw[11] = ATTR_DIRECTORY;
                let cluster = if k == 0 { node.cluster } else if dir.entry.is_none() { 0 } else { dir.cluster };
                put16(raw, 26, cluster & 0xFFFF); if self.bits == 32 { put16(raw, 20, cluster >> 16); }
                put16(raw, 14, stamp & 0xFFFF); put16(raw, 16, stamp >> 16); put16(raw, 22, stamp & 0xFFFF); put16(raw, 24, stamp >> 16);
            }
            let lba = self.sector_of(node.cluster);
            self.write_sector(lba, &data)?;
        }
        if let Err(error) = self.link(dir, name, &mut node, None) {
            if directory { let _ = self.free_chain(node.cluster); }
            return Err(error);
        }
        Ok(node)
    }

    fn empty(&mut self, dir: &Node) -> Result<bool> {
        let mut any = false;
        self.scan(dir, &mut |_| { any = true; false })?;
        Ok(!any)
    }

    /// Removes a file or an empty directory.
    pub fn remove(&mut self, dir: &Node, name: &str) -> Result<()> {
        if !self.disk.writable() { return Err(Error::ReadOnly); }
        let entry = self.find(dir, name)?;
        if entry.node.is_dir() && !self.empty(&entry.node)? { return Err(Error::NotEmpty); }
        self.unlink(&entry)?;
        if entry.node.cluster >= 2 { self.free_chain(entry.node.cluster)?; }
        Ok(())
    }

    /// Renames or moves `name` in `dir` to `new_name` in `target`; returns the node at its new place. A directory cannot
    /// move into itself.
    pub fn rename(&mut self, dir: &Node, name: &str, target: &Node, new_name: &str) -> Result<Node> {
        if !target.is_dir() { return Err(Error::NotDirectory); }
        if !valid_name(new_name) { return Err(Error::Name); }
        if !self.disk.writable() { return Err(Error::ReadOnly); }
        let entry = self.find(dir, name)?;
        let same_place = dir.cluster == target.cluster;
        let case_change = match self.find(target, new_name) {
            Ok(existing) if same_place && existing.node.entry == entry.node.entry => true,
            Ok(_) => return Err(Error::Exists),
            Err(Error::NotFound) => false,
            Err(e) => return Err(e),
        };
        if entry.node.is_dir() && !same_place {
            // Walk up from the target through ".." entries: meeting the moved directory would cut it off.
            let mut cluster = target.cluster;
            let mut steps = 0;
            while cluster >= 2 && !(self.bits == 32 && cluster == self.root_cluster) {
                if cluster == entry.node.cluster || steps > 4096 { return Err(Error::Invalid); }
                let data = self.read_sector(self.sector_of(cluster))?;
                cluster = u16_at(&data, 32 + 26) | if self.bits == 32 { u16_at(&data, 32 + 20) << 16 } else { 0 };
                steps += 1;
            }
        }
        // The new entry is written before the old one goes, so a failure leaves the old name (175-KRN-0048); a change
        // of case keeps the short alias.
        let mut node = entry.node;
        let slots = self.link(target, new_name, &mut node, if case_change { Some(entry.short) } else { None })?;
        if let Err(error) = self.unlink(&entry) { let _ = self.unlink_slots(&slots); return Err(error); }
        if case_change { return Ok(node); }
        if node.is_dir() && !same_place {
            let lba = self.sector_of(node.cluster);
            let mut data = self.read_sector(lba)?;
            let parent = if target.entry.is_none() { 0 } else { target.cluster };
            put16(&mut data, 32 + 26, parent & 0xFFFF); if self.bits == 32 { put16(&mut data, 32 + 20, parent >> 16); }
            self.write_sector(lba, &data)?;
        }
        Ok(node)
    }
}

/// What `check` found. Nothing is changed by a check.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub files: u32,
    pub directories: u32,
    /// Clusters reached from the directory tree, and free ones.
    pub used: u32,
    pub free: u32,
    /// Clusters marked in use that no file or directory reaches, and the chains they form.
    pub lost: u32,
    pub lost_chains: u32,
    /// Clusters reached by two chains.
    pub cross_linked: u32,
    /// Chains that run into a free, bad or invalid cluster.
    pub bad_chains: u32,
    /// Files whose size does not match the length of their chain.
    pub sizes: u32,
    /// Directory entries that are invalid (a bad short name, a directory with a size, a bad first cluster).
    pub bad_entries: u32,
    /// The volume is marked dirty (not flushed since a change).
    pub dirty: bool,
    /// The first problem, with its path.
    pub first: String,
}

impl Report {
    #[allow(dead_code)] // the host tests use it
    pub fn clean(&self) -> bool { self.lost + self.cross_linked + self.bad_chains + self.sizes + self.bad_entries == 0 }
    fn problem(&mut self, path: &str, what: &str) { if self.first.is_empty() { self.first = alloc::format!("{}: {}", if path.is_empty() { "/" } else { path }, what); } }
}

// FAT entries read two sectors at a time (a FAT12 entry may span them).
struct FatWindow { lba: u32, data: [u8; 2 * SECTOR] }

fn bad_short(short: &[u8; 11]) -> bool {
    short[0] == b' ' || short.iter().enumerate().any(|(i, &c)| (c < 0x20 && !(i == 0 && c == 0x05)) || c == 0x7F || b"\"*+,./:;<=>?[\\]|".contains(&c))
}

impl<S: Sectors> Volume<S> {
    fn entry_at(&mut self, window: &mut FatWindow, cluster: u32) -> Result<u32> {
        let offset = match self.bits { 12 => cluster + cluster / 2, 16 => cluster * 2, _ => cluster * 4 };
        let lba = self.fat_start + offset / 512;
        if window.lba != lba {
            // Walking on, the window's second sector becomes its first: one read a sector.
            if window.lba != u32::MAX && window.lba + 1 == lba { window.data.copy_within(SECTOR.., 0); } else { window.data[..SECTOR].copy_from_slice(&self.read_sector(lba)?); }
            let next = if lba + 1 < self.fat_start + self.fat_size { self.read_sector(lba + 1)? } else { [0; SECTOR] };
            window.data[SECTOR..].copy_from_slice(&next);
            window.lba = lba;
        }
        let at = (offset % 512) as usize;
        let b = &window.data[at..at + 4];
        Ok(match self.bits {
            12 => { let raw = b[0] as u32 | (b[1] as u32) << 8; if cluster & 1 == 0 { raw & 0xFFF } else { raw >> 4 } }
            16 => b[0] as u32 | (b[1] as u32) << 8,
            _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) & 0x0FFF_FFFF,
        })
    }

    // Follows a chain from `first`, marking its clusters; returns its length.
    fn check_chain(&mut self, window: &mut FatWindow, owned: &mut [u8], first: u32, path: &str, report: &mut Report) -> Result<u32> {
        let (eoc, bad) = (self.eoc() & !7, self.eoc() - 8);
        let mut cluster = first;
        let mut length = 0;
        loop {
            let (byte, bit) = (cluster as usize / 8, 1u8 << (cluster % 8));
            if owned[byte] & bit != 0 { report.cross_linked += 1; report.problem(path, "cross-linked with another chain"); return Ok(length); }
            owned[byte] |= bit;
            report.used += 1;
            length += 1;
            let next = self.entry_at(window, cluster)?;
            if next >= eoc { return Ok(length); }
            if next == 0 || next == bad || !self.is_cluster(next) { report.bad_chains += 1; report.problem(path, "the cluster chain runs into a free or invalid cluster"); return Ok(length); }
            cluster = next;
        }
    }

    /// Checks the volume without changing it: walks the directory tree following every chain, then looks for clusters
    /// in use that nothing reaches.
    pub fn check(&mut self) -> Result<Report> {
        let mut report = Report::default();
        let mut window = FatWindow { lba: u32::MAX, data: [0; 2 * SECTOR] };
        let mut owned = vec![0u8; (self.clusters as usize + 2).div_ceil(8)];
        let per = self.cluster_bytes();
        let clean_bit = match self.bits { 16 => Some(1u32 << 15), 32 => Some(1 << 27), _ => None };
        if let Some(bit) = clean_bit { report.dirty = self.changed || self.entry_at(&mut window, 1)? & bit == 0; }
        let root = self.root();
        if root.cluster != 0 { self.check_chain(&mut window, &mut owned, root.cluster, "", &mut report)?; }
        let mut stack: Vec<(Node, String, usize)> = vec![(root, String::new(), 0)];
        while let Some((dir, path, depth)) = stack.pop() {
            let entries = self.list(&dir)?;
            for entry in entries {
                let full = if path.is_empty() { entry.name.clone() } else { alloc::format!("{}/{}", path, entry.name) };
                let node = entry.node;
                if bad_short(&entry.short) { report.bad_entries += 1; report.problem(&full, "invalid short name"); }
                if node.cluster != 0 && !self.is_cluster(node.cluster) { report.bad_entries += 1; report.problem(&full, "invalid first cluster"); continue; }
                if node.is_dir() {
                    report.directories += 1;
                    if node.size != 0 { report.bad_entries += 1; report.problem(&full, "a directory with a size"); }
                    if node.cluster == 0 { report.bad_entries += 1; report.problem(&full, "a directory without clusters"); continue; }
                    let before = report.cross_linked + report.bad_chains;
                    self.check_chain(&mut window, &mut owned, node.cluster, &full, &mut report)?;
                    // A directory whose chain is damaged is not walked (it may lead anywhere).
                    if report.cross_linked + report.bad_chains == before && depth < 64 { stack.push((node, full, depth + 1)); }
                } else {
                    report.files += 1;
                    let length = if node.cluster == 0 { 0 } else { self.check_chain(&mut window, &mut owned, node.cluster, &full, &mut report)? };
                    if length != node.size.div_ceil(per) { report.sizes += 1; report.problem(&full, "the size does not match the cluster chain"); }
                }
            }
        }
        // Clusters in use that nothing reached; a lost chain starts at one no other lost cluster points to.
        let bad = self.eoc() - 8;
        let mut lost = vec![0u8; owned.len()];
        let mut pointed = vec![0u8; owned.len()];
        for cluster in 2..self.clusters + 2 {
            let value = self.entry_at(&mut window, cluster)?;
            let (byte, bit) = (cluster as usize / 8, 1u8 << (cluster % 8));
            if value == 0 { report.free += 1; continue; }
            if value == bad || owned[byte] & bit != 0 { continue; }
            report.lost += 1;
            lost[byte] |= bit;
            if self.is_cluster(value) { pointed[value as usize / 8] |= 1 << (value % 8); }
        }
        report.lost_chains = (2..self.clusters + 2).filter(|&c| { let (byte, bit) = (c as usize / 8, 1u8 << (c % 8)); lost[byte] & bit != 0 && pointed[byte] & bit == 0 }).count() as u32;
        if report.lost > 0 { report.problem("", "clusters in use that no file reaches"); }
        Ok(report)
    }
}

/// Where FAT volumes start: those of the MBR partitions with a FAT type, in table order, or the whole disk's.
pub fn fat_starts<S: Sectors>(disk: &mut S) -> [Option<u32>; 4] {
    let mut first = [0u8; SECTOR];
    if !disk.read(0, &mut first) || first[510] != 0x55 || first[511] != 0xAA { return [None; 4]; }
    if matches!(first[0], 0xEB | 0xE9) && u16_at(&first, 11) == 512 { return [Some(0), None, None, None]; }
    let mut starts = [None; 4];
    let entries = (0..4).map(|i| 446 + i * 16).filter(|&e| matches!(first[e + 4], 0x01 | 0x04 | 0x06 | 0x0B | 0x0C | 0x0E | 0xEF));
    for (slot, e) in starts.iter_mut().zip(entries) { *slot = Some(u32_at(&first, e + 8)); }
    starts
}

/// Formats a disk as one FAT16 volume (FAT12 when it is too small for FAT16) without a partition table.
pub fn format<S: Sectors>(disk: &mut S, label: &str, stamp: u32) -> Result<()> {
    if !disk.writable() { return Err(Error::ReadOnly); }
    let total = disk.sectors().min(u32::MAX as u64) as u32;
    let (reserved, fats, root_entries) = (1u32, 2u32, 512u32);
    let root_sectors = root_entries * 32 / 512;
    let mut spc = 1u32;
    let (mut bits, mut fat_size, mut clusters);
    loop {
        fat_size = 1;
        for _ in 0..3 {
            let clusters = total.saturating_sub(reserved + fats * fat_size + root_sectors) / spc;
            let bits = if clusters < 4085 { 12 } else { 16 };
            fat_size = ((clusters + 2) * bits).div_ceil(8 * 512).max(1);
        }
        clusters = total.saturating_sub(reserved + fats * fat_size + root_sectors) / spc;
        bits = if clusters < 4085 { 12 } else { 16 };
        if clusters <= 65524 { break; }
        spc *= 2;
        if spc > 64 { return Err(Error::Invalid); }
    }
    if clusters < 16 { return Err(Error::NoSpace); }
    let mut boot = [0u8; SECTOR];
    boot[..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
    boot[3..11].copy_from_slice(b"MINDCORE");
    put16(&mut boot, 11, 512); boot[13] = spc as u8; put16(&mut boot, 14, reserved); boot[16] = fats as u8; put16(&mut boot, 17, root_entries);
    if total < 65536 { put16(&mut boot, 19, total); } else { put32(&mut boot, 32, total); }
    boot[21] = 0xF8; put16(&mut boot, 22, fat_size); put16(&mut boot, 24, 32); put16(&mut boot, 26, 64);
    boot[36] = 0x80; boot[38] = 0x29; put32(&mut boot, 39, stamp ^ 0x4D49_4E44);
    let mut name = [b' '; 11];
    for (i, c) in label.bytes().filter(|c| c.is_ascii_graphic() || *c == b' ').take(11).enumerate() { name[i] = c.to_ascii_uppercase(); }
    boot[43..54].copy_from_slice(&name);
    boot[54..62].copy_from_slice(if bits == 12 { b"FAT12   " } else { b"FAT16   " });
    boot[510] = 0x55; boot[511] = 0xAA;
    let write = |disk: &mut S, lba: u32, data: &[u8; SECTOR]| if disk.write(lba, data) { Ok(()) } else { Err(Error::Io) };
    write(disk, 0, &boot)?;
    for copy in 0..fats {
        let base = reserved + copy * fat_size;
        for s in 0..fat_size {
            let mut data = [0u8; SECTOR];
            if s == 0 {
                if bits == 12 { data[..3].copy_from_slice(&[0xF8, 0xFF, 0xFF]); } else { data[..4].copy_from_slice(&[0xF8, 0xFF, 0xFF, 0xFF]); }
            }
            write(disk, base + s, &data)?;
        }
    }
    let root = reserved + fats * fat_size;
    for s in 0..root_sectors {
        let mut data = [0u8; SECTOR];
        if s == 0 && name != [b' '; 11] {
            data[..11].copy_from_slice(&name); data[11] = ATTR_VOLUME;
            put16(&mut data, 22, stamp & 0xFFFF); put16(&mut data, 24, stamp >> 16);
        }
        write(disk, root + s, &data)?;
    }
    if disk.flush() { Ok(()) } else { Err(Error::Io) }
}
