// Volume sectors through a block driver over IPC: a cache of 64 sectors with read-ahead and write-back, and runs of
// file data read past it in large requests. A changed sector stays in the cache until it is evicted or the volume is
// flushed. Either writes every changed sector in LBA order, runs of neighbours in one write (000-KRN-0078: one write a
// sector held writes to 1.6 MiB/s); a flush then asks the drive to empty its own cache.
use crate::fat::{Sectors, SECTOR};
use alloc::rc::Rc;
use core::cell::RefCell;
use mind::abi::BLOCK_MAX_SECTORS;
use mind::block::Device;
use mind::mem::Pages;

const LINES: usize = 64;
const READ_AHEAD: usize = 32;
const SLOW_MS: u64 = 1000; // a failure that took this long: the drive does not answer (unplugged), not a bad sector
const QUIET_MS: u64 = 10_000; // then requests fail at once for this long, before the drive is asked again (211-KRN-0050)
const IDENTITY: usize = 256; // at most this many sectors read while mounting are kept, by hash, as the medium's identity

// `silent_until`: until when the drive is not asked, after it did not answer. `first`: sector 0 as mounted; `identity`:
// the sectors read while mounting (the boot manifest's among them), by hash, until `mounted`; `replaced`: the drive came
// back with another medium (000-KRN-0079).
pub struct Disk { device: Device, tags: [u32; LINES], dirty: [bool; LINES], cache: Pages, next: usize, failed: bool, silent_until: u64, first: Option<[u8; SECTOR]>, identity: alloc::vec::Vec<(u32, [u8; 32])>, recording: bool, replaced: bool }

impl Disk {
    pub fn new(mut device: Device) -> Option<Self> {
        let first = device.read(0, 1).ok().and_then(|data| data.get(..SECTOR).map(|s| { let mut sector = [0u8; SECTOR]; sector.copy_from_slice(s); sector }));
        Some(Self { device, tags: [u32::MAX; LINES], dirty: [false; LINES], cache: Pages::new(LINES * SECTOR)?, next: 0, failed: false, silent_until: 0, first, identity: alloc::vec::Vec::new(), recording: true, replaced: false })
    }
    pub fn kind(&self) -> usize { self.device.kind() }
    /// The volumes are mounted: sectors read from now on are not part of the medium's identity.
    pub fn mounted(&mut self) { self.recording = false; }

    // A drive that did not answer is not asked again for QUIET_MS: requests fail at once instead of each waiting for the
    // drive's own timeout (an unplugged USB disk, 211-KRN-0050). Cached sectors are still read.
    fn quiet(&self) -> bool { self.silent_until != 0 && (mind::time::uptime_ms() as u64) < self.silent_until }
    // Runs one request to the drive and notes whether it answered.
    fn ask<T>(&mut self, request: impl FnOnce(&mut Device) -> Option<T>) -> Option<T> {
        if self.quiet() || self.replaced { return None; }
        // Back after it did not answer: the medium compared before anything is asked of it; a read that fails there
        // means the drive still does not answer.
        if self.silent_until != 0 {
            let Some(same) = self.same_medium() else { self.silent_until = mind::time::uptime_ms() as u64 + QUIET_MS; return None };
            self.silent_until = 0;
            if !same { return None; }
            mind::println!("[VFS] THE {} DRIVE ANSWERS AGAIN", crate::device_name(self.device.kind()));
        }
        let start = mind::time::uptime_ms() as u64;
        let result = request(&mut self.device);
        let now = mind::time::uptime_ms() as u64;
        if result.is_none() && now.saturating_sub(start) >= SLOW_MS {
            mind::println!("[VFS] THE {} DRIVE DOES NOT ANSWER: REQUESTS FAIL AT ONCE, IT IS ASKED AGAIN EVERY {} S", crate::device_name(self.device.kind()), QUIET_MS / 1000);
            self.silent_until = now + QUIET_MS;
        }
        result
    }

    fn line(&self, index: usize) -> &[u8] { &self.cache.as_slice()[index * SECTOR..(index + 1) * SECTOR] }

    // A drive back after it did not answer may hold another medium: a stick rewritten elsewhere and put back, or another
    // one (000-KRN-0079). Sector 0, the identity's sectors and every unchanged cached sector must read as before;
    // otherwise nothing cached is written to it, its volumes fail from now on, and the log says so. None: a read failed.
    fn same_medium(&mut self) -> Option<bool> {
        let mut checks: alloc::vec::Vec<(u32, [u8; 32])> = self.first.iter().map(|s| (0u32, mind::sha256::digest(s))).collect();
        checks.extend(self.identity.iter().copied());
        for index in (0..LINES).filter(|&i| self.tags[i] != u32::MAX && !self.dirty[i]) { checks.push((self.tags[index], mind::sha256::digest(self.line(index)))); }
        checks.sort_unstable_by_key(|&(lba, _)| lba);
        checks.dedup_by_key(|&mut (lba, _)| lba);
        let mut same = true;
        for (lba, before) in &checks {
            let now = self.device.read(*lba as u64, 1).ok()?;
            if now.get(..SECTOR).map(mind::sha256::digest) != Some(*before) { same = false; break; }
        }
        if !same {
            let lost = self.dirty.iter().filter(|&&d| d).count();
            self.discard();
            self.replaced = true;
            mind::println!("[VFS] THE {} DRIVE CAME BACK WITH ANOTHER MEDIUM: ITS VOLUMES ARE CLOSED, {} CHANGED SECTORS OF THE OLD ONE NOT WRITTEN TO IT", crate::device_name(self.device.kind()), lost);
        }
        Some(same)
    }

    // A sector read while mounting joins the identity; one this system writes leaves it (the medium may not hold it yet).
    fn note_read(&mut self, lba: u32, data: &[u8; SECTOR]) {
        if self.recording && self.identity.len() < IDENTITY && !self.identity.iter().any(|&(at, _)| at == lba) { self.identity.push((lba, mind::sha256::digest(data))); }
    }

    // A line to fill: the next one round robin; a changed one is written out first, with every other changed line.
    fn victim(&mut self) -> Option<usize> {
        let index = self.next; self.next = (self.next + 1) % LINES;
        if self.dirty[index] && !self.write_dirty() { self.failed = true; return None; }
        Some(index)
    }

    // Writes the changed lines in LBA order, neighbours in one write. A run that fails stays dirty.
    fn write_dirty(&mut self) -> bool {
        let mut order: [usize; LINES] = core::array::from_fn(|i| i);
        order.sort_unstable_by_key(|&i| self.tags[i]);
        let dirty: alloc::vec::Vec<usize> = order.iter().copied().filter(|&i| self.dirty[i]).collect();
        let mut run = alloc::vec::Vec::with_capacity(LINES * SECTOR);
        let mut at = 0;
        while at < dirty.len() {
            let mut end = at + 1;
            while end < dirty.len() && self.tags[dirty[end]] == self.tags[dirty[end - 1]] + 1 { end += 1; }
            run.clear();
            for &index in &dirty[at..end] { run.extend_from_slice(self.line(index)); }
            let lba = self.tags[dirty[at]] as u64;
            if self.ask(|device| (device.write(lba, &run) == Ok(end - at)).then_some(())).is_none() { return false; }
            for &index in &dirty[at..end] { self.dirty[index] = false; }
            at = end;
        }
        true
    }
}

impl Sectors for Disk {
    fn read(&mut self, lba: u32, out: &mut [u8; SECTOR]) -> bool {
        if self.replaced { return false; }
        if let Some(index) = self.tags.iter().position(|&tag| tag == lba) { out.copy_from_slice(self.line(index)); if !self.dirty[index] { self.note_read(lba, out); } return true; }
        // Read ahead only sectors that are not cached (a cached copy may be newer than the disk).
        let mut count = READ_AHEAD.min((self.device.sectors().saturating_sub(lba as u64)) as usize);
        if let Some(cached) = (1..count).find(|&i| self.tags.contains(&(lba + i as u32))) { count = cached; }
        if count == 0 { return false; }
        let mut sectors = [[0u8; SECTOR]; READ_AHEAD];
        let read = |device: &mut Device| device.read(lba as u64, count).ok().map(|data| {
            for (i, sector) in data.chunks_exact(SECTOR).take(READ_AHEAD).enumerate() { sectors[i].copy_from_slice(sector); }
            (data.len() / SECTOR).min(READ_AHEAD)
        });
        let Some(got) = self.ask(read) else { return false };
        if got == 0 { return false; }
        out.copy_from_slice(&sectors[0]);
        self.note_read(lba, out);
        for (i, sector) in sectors[..got].iter().enumerate() {
            let Some(index) = self.victim() else { return i > 0 };
            self.tags[index] = lba + i as u32;
            self.cache.as_mut_slice()[index * SECTOR..(index + 1) * SECTOR].copy_from_slice(sector);
        }
        true
    }

    fn write(&mut self, lba: u32, data: &[u8; SECTOR]) -> bool {
        if self.replaced { return false; }
        self.identity.retain(|&(at, _)| at != lba);
        let index = match self.tags.iter().position(|&tag| tag == lba) { Some(index) => index, None => match self.victim() { Some(index) => index, None => return false } };
        self.tags[index] = lba;
        self.cache.as_mut_slice()[index * SECTOR..(index + 1) * SECTOR].copy_from_slice(data);
        self.dirty[index] = true;
        true
    }

    fn flush(&mut self) -> bool {
        if self.device.read_only() { return true; } // nothing can have changed
        // A run that fails stays dirty and is written by the next flush: nothing is lost, so `failed` is not set.
        if !self.write_dirty() { return false; }
        !core::mem::take(&mut self.failed) && self.ask(|device| device.flush().ok()).is_some()
    }

    fn discard(&mut self) { self.tags = [u32::MAX; LINES]; self.dirty = [false; LINES]; self.failed = false; }

    // File data in requests of up to BLOCK_MAX_SECTORS, past the cache; then the cached sectors of the run over it,
    // which are as new as the medium's or newer.
    fn read_run(&mut self, lba: u32, out: &mut [u8]) -> bool {
        let count = out.len() / SECTOR;
        let mut at = 0;
        while at < count {
            let Ok(data) = self.device.read(lba as u64 + at as u64, (count - at).min(BLOCK_MAX_SECTORS)) else { return false };
            let got = (data.len() / SECTOR).min(count - at);
            if got == 0 { return false; }
            out[at * SECTOR..(at + got) * SECTOR].copy_from_slice(&data[..got * SECTOR]);
            at += got;
        }
        for (index, &tag) in self.tags.iter().enumerate() {
            if tag >= lba && ((tag - lba) as usize) < count { let i = (tag - lba) as usize; out[i * SECTOR..(i + 1) * SECTOR].copy_from_slice(&self.cache.as_slice()[index * SECTOR..(index + 1) * SECTOR]); }
        }
        true
    }

    fn sectors(&self) -> u64 { self.device.sectors() }
    fn writable(&self) -> bool { !self.device.read_only() }
}

/// A disk that several volumes share (the boot volume and the log volume, 211-KRN-0019): one cache, one driver client.
#[derive(Clone)]
pub struct Shared(Rc<RefCell<Disk>>);

impl Shared {
    pub fn new(disk: Disk) -> Self { Self(Rc::new(RefCell::new(disk))) }
    pub fn kind(&self) -> usize { self.0.borrow().kind() }
    pub fn mounted(&self) { self.0.borrow_mut().mounted() }
}

impl Sectors for Shared {
    fn read(&mut self, lba: u32, out: &mut [u8; SECTOR]) -> bool { self.0.borrow_mut().read(lba, out) }
    fn write(&mut self, lba: u32, data: &[u8; SECTOR]) -> bool { self.0.borrow_mut().write(lba, data) }
    fn flush(&mut self) -> bool { self.0.borrow_mut().flush() }
    fn discard(&mut self) { self.0.borrow_mut().discard() }
    fn read_run(&mut self, lba: u32, out: &mut [u8]) -> bool { self.0.borrow_mut().read_run(lba, out) }
    fn sectors(&self) -> u64 { self.0.borrow().sectors() }
    fn writable(&self) -> bool { self.0.borrow().writable() }
}
