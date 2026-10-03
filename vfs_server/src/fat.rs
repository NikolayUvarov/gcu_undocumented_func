// Read-only FAT12/16/32: MBR or bare volume, directories, long names (ASCII).
use crate::disk::Disk;

#[derive(Clone, Copy)]
pub struct Node { pub cluster: u32, pub size: u32, pub is_dir: bool, root: bool }

pub struct Entry<'a> { pub name: &'a [u8], pub node: Node }

pub struct Volume { disk: Disk, start: u32, bits: u8, sectors_per_cluster: u32, fat_start: u32, root_start: u32, root_sectors: u32, data_start: u32, root_cluster: u32 }

fn u16_at(b: &[u8], at: usize) -> u32 { u16::from_le_bytes([b[at], b[at + 1]]) as u32 }
fn u32_at(b: &[u8], at: usize) -> u32 { u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) }

impl Volume {
    pub fn mount(mut disk: Disk) -> Option<Self> {
        let first = *disk.read(0)?;
        if first[510] != 0x55 || first[511] != 0xAA { return None; }
        let bare = matches!(first[0], 0xEB | 0xE9) && u16_at(&first, 11) == 512;
        let start = if bare { 0 } else {
            (0..4).map(|i| 446 + i * 16).find(|&e| matches!(first[e + 4], 0x01 | 0x04 | 0x06 | 0x0B | 0x0C | 0x0E | 0xEF)).map(|e| u32_at(&first, e + 8))?
        };
        let boot = *disk.read(start)?;
        if u16_at(&boot, 11) != 512 { return None; }
        let sectors_per_cluster = boot[13] as u32; let reserved = u16_at(&boot, 14); let fats = boot[16] as u32; let root_entries = u16_at(&boot, 17);
        let total = if u16_at(&boot, 19) != 0 { u16_at(&boot, 19) } else { u32_at(&boot, 32) };
        let fat_size = if u16_at(&boot, 22) != 0 { u16_at(&boot, 22) } else { u32_at(&boot, 36) };
        if sectors_per_cluster == 0 || fats == 0 || fat_size == 0 { return None; }
        let root_sectors = (root_entries * 32).div_ceil(512);
        let data = reserved + fats * fat_size + root_sectors;
        let clusters = total.checked_sub(data)? / sectors_per_cluster;
        let bits = if clusters < 4085 { 12 } else if clusters < 65525 { 16 } else { 32 };
        Some(Self { disk, start, bits, sectors_per_cluster, fat_start: start + reserved, root_start: start + reserved + fats * fat_size, root_sectors, data_start: start + data, root_cluster: if bits == 32 { u32_at(&boot, 44) } else { 0 } })
    }
    pub fn bits(&self) -> u8 { self.bits }
    pub fn kind(&self) -> usize { self.disk.kind() }
    pub fn start(&self) -> u32 { self.start }

    fn cluster_bytes(&self) -> usize { self.sectors_per_cluster as usize * 512 }
    fn sector_of(&self, cluster: u32) -> u32 { self.data_start + (cluster - 2) * self.sectors_per_cluster }
    fn byte(&mut self, offset: u32) -> Option<u8> { Some(self.disk.read(self.fat_start + offset / 512)?[(offset % 512) as usize]) }

    // Next cluster in the chain, or None at the end/on error.
    fn next(&mut self, cluster: u32) -> Option<u32> {
        let value = match self.bits {
            12 => { let at = cluster + cluster / 2; let raw = self.byte(at)? as u32 | (self.byte(at + 1)? as u32) << 8; if cluster & 1 == 0 { raw & 0xFFF } else { raw >> 4 } }
            16 => { let at = cluster * 2; self.byte(at)? as u32 | (self.byte(at + 1)? as u32) << 8 }
            _ => { let at = cluster * 4; (self.byte(at)? as u32 | (self.byte(at + 1)? as u32) << 8 | (self.byte(at + 2)? as u32) << 16 | (self.byte(at + 3)? as u32) << 24) & 0x0FFF_FFFF }
        };
        let end = match self.bits { 12 => 0xFF8, 16 => 0xFFF8, _ => 0x0FFF_FFF8 };
        (value >= 2 && value < end).then_some(value)
    }

    fn root(&self) -> Node { Node { cluster: self.root_cluster, size: 0, is_dir: true, root: self.bits != 32 } }

    /// Directory iteration; `visit` returns false to stop.
    pub fn walk(&mut self, dir: &Node, mut visit: impl FnMut(&Entry) -> bool) {
        let mut long = [0u8; 255]; let mut long_len = 0usize; let mut long_sum: Option<u8> = None;
        let mut cluster = dir.cluster; let mut sector_index = 0u32;
        loop {
            let sector = if dir.root && dir.is_dir && dir.cluster == 0 {
                if sector_index >= self.root_sectors { return; }
                self.root_start + sector_index
            } else {
                if sector_index == self.sectors_per_cluster { match self.next(cluster) { Some(n) => { cluster = n; sector_index = 0; } None => return } }
                if cluster < 2 { return; }
                self.sector_of(cluster) + sector_index
            };
            sector_index += 1;
            let Some(data) = self.disk.read(sector).copied() else { return };
            for raw in data.chunks_exact(32) {
                if raw[0] == 0 { return; }
                if raw[0] == 0xE5 { long_sum = None; continue; }
                if raw[11] == 0x0F {
                    // Long name: fragments come in reverse order, 13 UCS-2 characters each.
                    let order = (raw[0] & 0x1F) as usize; if order == 0 || order > 20 { long_sum = None; continue; }
                    if raw[0] & 0x40 != 0 { long_len = 0; long_sum = Some(raw[13]); }
                    for (i, at) in [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30].into_iter().enumerate() {
                        let ch = u16::from_le_bytes([raw[at], raw[at + 1]]); let index = (order - 1) * 13 + i;
                        if ch == 0 || ch == 0xFFFF || index >= long.len() { continue; }
                        long[index] = if ch < 128 { ch as u8 } else { b'?' }; long_len = long_len.max(index + 1);
                    }
                    continue;
                }
                if raw[11] & 0x08 != 0 { long_sum = None; continue; } // volume label
                let checksum = raw[..11].iter().fold(0u8, |sum, &c| sum.rotate_right(1).wrapping_add(c));
                let mut short = [0u8; 12]; let mut short_len = 0;
                for &c in raw[..8].iter().filter(|&&c| c != b' ') { short[short_len] = c.to_ascii_lowercase(); short_len += 1; }
                if raw[8] != b' ' { short[short_len] = b'.'; short_len += 1; for &c in raw[8..11].iter().filter(|&&c| c != b' ') { short[short_len] = c.to_ascii_lowercase(); short_len += 1; } }
                let name: &[u8] = if long_sum == Some(checksum) && long_len > 0 { &long[..long_len] } else { &short[..short_len] };
                long_sum = None;
                if name == b"." || name == b".." { continue; }
                let first = u16_at(raw, 26) | if self.bits == 32 { u16_at(raw, 20) << 16 } else { 0 };
                let node = Node { cluster: first, size: u32_at(raw, 28), is_dir: raw[11] & 0x10 != 0, root: false };
                if !visit(&Entry { name, node }) { return; }
            }
        }
    }

    /// Path of the form `dir/file.ext` (case-insensitive); an empty path is the root.
    pub fn resolve(&mut self, path: &[u8]) -> Option<Node> {
        let mut node = self.root();
        for part in path.split(|&c| c == b'/' || c == b'\\').filter(|p| !p.is_empty()) {
            if !node.is_dir { return None; }
            let mut found = None;
            self.walk(&node, |entry| { if entry.name.eq_ignore_ascii_case(part) { found = Some(entry.node); false } else { true } });
            node = found?;
            if node.is_dir && node.cluster == 0 { node = self.root(); } // ".." to the root on FAT12/16
        }
        Some(node)
    }

    /// Reads from `offset`; `cursor` remembers the chain position for sequential reads.
    pub fn read(&mut self, file: &Node, offset: usize, out: &mut [u8], cursor: &mut Option<(usize, u32)>) -> usize {
        let size = file.size as usize; if offset >= size || file.cluster < 2 { return 0; }
        let want = out.len().min(size - offset); let per = self.cluster_bytes();
        let (mut index, mut cluster) = match *cursor { Some((i, c)) if i <= offset / per => (i, c), _ => (0, file.cluster) };
        let mut done = 0;
        while done < want {
            let position = offset + done; let target = position / per;
            while index < target { match self.next(cluster) { Some(n) => { cluster = n; index += 1; } None => return done } }
            *cursor = Some((index, cluster));
            let within = position % per; let sector = self.sector_of(cluster) + (within / 512) as u32;
            let Some(data) = self.disk.read(sector) else { return done };
            let at = within % 512; let take = (512 - at).min(want - done);
            out[done..done + take].copy_from_slice(&data[at..at + take]);
            done += take;
        }
        done
    }
}
