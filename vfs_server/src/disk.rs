// Volume sectors via a block driver over IPC, with read-ahead and a small cache.
use mind::block::Device;
use mind::mem::Pages;

const LINES: usize = 64;
const READ_AHEAD: usize = 32;

pub struct Disk { device: Device, tags: [u32; LINES], cache: Pages, next: usize }

impl Disk {
    pub fn new(device: Device) -> Option<Self> {
        Some(Self { device, tags: [u32::MAX; LINES], cache: Pages::new(LINES * 512)?, next: 0 })
    }
    pub fn kind(&self) -> usize { self.device.kind() }
    /// The driver refuses writes (protected medium, or no write badge on our capability).
    pub fn read_only(&self) -> bool { self.device.read_only() }

    fn line(&self, index: usize) -> &[u8; 512] { self.cache.as_slice()[index * 512..index * 512 + 512].try_into().unwrap() }

    pub fn read(&mut self, lba: u32) -> Option<&[u8; 512]> {
        if let Some(index) = self.tags.iter().position(|&tag| tag == lba) { return Some(self.line(index)); }
        let count = READ_AHEAD.min((self.device.sectors().saturating_sub(lba as u64)) as usize);
        let data = self.device.read(lba as u64, count).ok()?;
        let mut first = None;
        for (i, sector) in data.chunks_exact(512).enumerate() {
            let index = self.next; self.next = (self.next + 1) % LINES;
            self.tags[index] = lba + i as u32;
            self.cache.as_mut_slice()[index * 512..index * 512 + 512].copy_from_slice(sector);
            first.get_or_insert(index);
        }
        first.map(|index| self.line(index))
    }
}
