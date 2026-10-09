// Volume sectors through a block driver over IPC: a cache of 64 sectors with read-ahead and write-back. A changed
// sector stays in the cache until it is evicted or the volume is flushed; a flush writes the changed sectors in LBA
// order (runs of neighbours in one write) and then asks the drive to empty its own cache.
use crate::fat::{Sectors, SECTOR};
use alloc::rc::Rc;
use core::cell::RefCell;
use mind::block::Device;
use mind::mem::Pages;

const LINES: usize = 64;
const READ_AHEAD: usize = 32;

pub struct Disk { device: Device, tags: [u32; LINES], dirty: [bool; LINES], cache: Pages, next: usize, failed: bool }

impl Disk {
    pub fn new(device: Device) -> Option<Self> {
        Some(Self { device, tags: [u32::MAX; LINES], dirty: [false; LINES], cache: Pages::new(LINES * SECTOR)?, next: 0, failed: false })
    }
    pub fn kind(&self) -> usize { self.device.kind() }

    fn line(&self, index: usize) -> &[u8] { &self.cache.as_slice()[index * SECTOR..(index + 1) * SECTOR] }

    // A line to fill: the next one round robin, written out first if it was changed.
    fn victim(&mut self) -> Option<usize> {
        let index = self.next; self.next = (self.next + 1) % LINES;
        if self.dirty[index] && !self.write_line(index) { return None; }
        Some(index)
    }

    fn write_line(&mut self, index: usize) -> bool {
        let done = self.device.write(self.tags[index] as u64, &self.cache.as_slice()[index * SECTOR..(index + 1) * SECTOR]) == Ok(1);
        if done { self.dirty[index] = false; } else { self.failed = true; }
        done
    }
}

impl Sectors for Disk {
    fn read(&mut self, lba: u32, out: &mut [u8; SECTOR]) -> bool {
        if let Some(index) = self.tags.iter().position(|&tag| tag == lba) { out.copy_from_slice(self.line(index)); return true; }
        // Read ahead only sectors that are not cached (a cached copy may be newer than the disk).
        let mut count = READ_AHEAD.min((self.device.sectors().saturating_sub(lba as u64)) as usize);
        if let Some(cached) = (1..count).find(|&i| self.tags.contains(&(lba + i as u32))) { count = cached; }
        if count == 0 { return false; }
        let Ok(data) = self.device.read(lba as u64, count) else { return false };
        let mut sectors = [[0u8; SECTOR]; READ_AHEAD];
        let got = data.len() / SECTOR;
        for (i, sector) in data.chunks_exact(SECTOR).enumerate() { sectors[i].copy_from_slice(sector); }
        if got == 0 { return false; }
        out.copy_from_slice(&sectors[0]);
        for (i, sector) in sectors[..got].iter().enumerate() {
            let Some(index) = self.victim() else { return i > 0 };
            self.tags[index] = lba + i as u32;
            self.cache.as_mut_slice()[index * SECTOR..(index + 1) * SECTOR].copy_from_slice(sector);
        }
        true
    }

    fn write(&mut self, lba: u32, data: &[u8; SECTOR]) -> bool {
        let index = match self.tags.iter().position(|&tag| tag == lba) { Some(index) => index, None => match self.victim() { Some(index) => index, None => return false } };
        self.tags[index] = lba;
        self.cache.as_mut_slice()[index * SECTOR..(index + 1) * SECTOR].copy_from_slice(data);
        self.dirty[index] = true;
        true
    }

    fn flush(&mut self) -> bool {
        if self.device.read_only() { return true; } // nothing can have changed
        let mut order: [usize; LINES] = core::array::from_fn(|i| i);
        order.sort_unstable_by_key(|&i| self.tags[i]);
        // Changed lines of neighbouring sectors go to the drive in one write (each write carries a sealed copy).
        let dirty: alloc::vec::Vec<usize> = order.iter().copied().filter(|&i| self.dirty[i]).collect();
        let mut run = alloc::vec::Vec::with_capacity(LINES * SECTOR);
        let mut at = 0;
        while at < dirty.len() {
            let mut end = at + 1;
            while end < dirty.len() && self.tags[dirty[end]] == self.tags[dirty[end - 1]] + 1 { end += 1; }
            run.clear();
            for &index in &dirty[at..end] { run.extend_from_slice(self.line(index)); }
            if self.device.write(self.tags[dirty[at]] as u64, &run) != Ok(end - at) { self.failed = true; return false; }
            for &index in &dirty[at..end] { self.dirty[index] = false; }
            at = end;
        }
        !core::mem::take(&mut self.failed) && self.device.flush().is_ok()
    }

    fn discard(&mut self) { self.tags = [u32::MAX; LINES]; self.dirty = [false; LINES]; self.failed = false; }

    fn sectors(&self) -> u64 { self.device.sectors() }
    fn writable(&self) -> bool { !self.device.read_only() }
}

/// A disk that several volumes share (the boot volume and the log volume, 211-KRN-0019): one cache, one driver client.
#[derive(Clone)]
pub struct Shared(Rc<RefCell<Disk>>);

impl Shared {
    pub fn new(disk: Disk) -> Self { Self(Rc::new(RefCell::new(disk))) }
    pub fn kind(&self) -> usize { self.0.borrow().kind() }
}

impl Sectors for Shared {
    fn read(&mut self, lba: u32, out: &mut [u8; SECTOR]) -> bool { self.0.borrow_mut().read(lba, out) }
    fn write(&mut self, lba: u32, data: &[u8; SECTOR]) -> bool { self.0.borrow_mut().write(lba, data) }
    fn flush(&mut self) -> bool { self.0.borrow_mut().flush() }
    fn discard(&mut self) { self.0.borrow_mut().discard() }
    fn sectors(&self) -> u64 { self.0.borrow().sectors() }
    fn writable(&self) -> bool { self.0.borrow().writable() }
}
