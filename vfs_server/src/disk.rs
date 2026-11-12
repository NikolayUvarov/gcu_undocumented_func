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
use mind::sys::{Error, Result};

const LINES: usize = 64;
const READ_AHEAD: usize = 32;
const SLOW_MS: u64 = 1000; // a failure that took this long: the drive does not answer (unplugged), not a bad sector
const QUIET_MS: u64 = 10_000; // then requests fail at once for this long, before the drive is asked again (211-KRN-0050)
const BACK_MS: u64 = 10_000; // a driver that answered "not found" is tried this long before it is said not to answer (a fresh
// instance and init's windows: 6 s in QEMU)
const RETRY_MS: u64 = 500; // meanwhile, between tries
const IDENTITY: usize = 256; // at most this many sectors read while mounting are kept, by hash, as the medium's identity

// `silent_until`: until when the drive is not asked, after it did not answer; `missing_since`: when it answered "not
// found" first, and `said`: whether the log says it does not answer. `first`: sector 0 as mounted (none through
// windows); `identity`: the sectors read while mounting (the boot manifest's among them), by hash, until `mounted`;
// `replaced`: the drive came back with another medium (000-KRN-0079).
pub struct Disk { device: Io, tags: [u32; LINES], dirty: [bool; LINES], cache: Pages, next: usize, failed: bool, silent_until: u64, missing_since: u64, said: bool, first: Option<[u8; SECTOR]>, identity: alloc::vec::Vec<(u32, [u8; 32])>, recording: bool, replaced: bool }

/// A disk as vfs_server reaches it: the whole device, or windows of a partitioned one, each at its first sector on the
/// disk (307-KRN-0229). A sector is asked of the window that starts last at or before it, so one past that window's end
/// reaches its driver, which refuses it; one before every window is refused here.
pub enum Io { Whole(Device), Windows(alloc::vec::Vec<(u64, Device)>) }

impl Io {
    fn at(&mut self, lba: u64) -> Option<(&mut Device, u64)> {
        match self {
            Io::Whole(device) => Some((device, lba)),
            Io::Windows(windows) => windows.iter_mut().rev().find(|(first, _)| *first <= lba).map(|(first, device)| (device, lba - *first)),
        }
    }
    fn devices(&self) -> impl Iterator<Item = &Device> { let (whole, windows) = match self { Io::Whole(device) => (Some(device), &[][..]), Io::Windows(windows) => (None, &windows[..]) }; whole.into_iter().chain(windows.iter().map(|(_, device)| device)) }
    fn devices_mut(&mut self) -> impl Iterator<Item = &mut Device> { let (whole, windows) = match self { Io::Whole(device) => (Some(device), &mut [][..]), Io::Windows(windows) => (None, &mut windows[..]) }; whole.into_iter().chain(windows.iter_mut().map(|(_, device)| device)) }
    /// A driver that answered as a fresh instance does is not asked again before the medium is checked (000-KRN-0085).
    fn check_returns(&mut self) { self.devices_mut().for_each(Device::check_returns); }
    fn returned(&self) -> bool { self.devices().any(Device::returned) }
    // Every window's client re-attached: one driver behind them all, restarted for all.
    fn reattach(&mut self) -> Result<()> { self.devices_mut().try_for_each(Device::reattach) }
    pub fn read(&mut self, lba: u64, count: usize) -> Result<&[u8]> { let (device, lba) = self.at(lba).ok_or(Error::Invalid)?; device.read(lba, count) }
    pub fn write(&mut self, lba: u64, data: &[u8]) -> Result<usize> { let (device, lba) = self.at(lba).ok_or(Error::Invalid)?; device.write(lba, data) }
    // One drive behind every window: one flush empties its cache.
    pub fn flush(&mut self) -> Result<()> { match self { Io::Whole(device) => device.flush(), Io::Windows(windows) => windows.first_mut().ok_or(Error::NotFound)?.1.flush() } }
    pub fn kind(&self) -> usize { self.devices().next().map_or(0, Device::kind) }
    pub fn read_only(&self) -> bool { self.devices().any(Device::read_only) }
    /// Up to the end of the last window.
    pub fn sectors(&self) -> u64 { match self { Io::Whole(device) => device.sectors(), Io::Windows(windows) => windows.iter().map(|(first, device)| first + device.sectors()).max().unwrap_or(0) } }
}

impl Disk {
    pub fn new(mut device: Io) -> Option<Self> {
        device.check_returns();
        let first = device.read(0, 1).ok().and_then(|data| data.get(..SECTOR).map(|s| { let mut sector = [0u8; SECTOR]; sector.copy_from_slice(s); sector }));
        Some(Self { device, tags: [u32::MAX; LINES], dirty: [false; LINES], cache: Pages::new(LINES * SECTOR)?, next: 0, failed: false, silent_until: 0, missing_since: 0, said: false, first, identity: alloc::vec::Vec::new(), recording: true, replaced: false })
    }
    pub fn kind(&self) -> usize { self.device.kind() }
    /// The volumes are mounted: sectors read from now on are not part of the medium's identity.
    pub fn mounted(&mut self) { self.recording = false; }

    // A drive that did not answer is not asked again for QUIET_MS: requests fail at once instead of each waiting for the
    // drive's own timeout (an unplugged USB disk, 211-KRN-0050). Cached sectors are still read.
    fn quiet(&self) -> bool { self.silent_until != 0 && (mind::time::uptime_ms() as u64) < self.silent_until }
    // Runs one request to the drive and notes whether it answered. A driver that answers "not found" (restarted, its
    // medium gone and back, 000-KRN-0085, or its device gone) gets the request again only after the medium is compared,
    // and nothing waits here: while the comparison cannot be made yet, requests fail at once and it is tried again
    // RETRY_MS later, and after BACK_MS of that the drive is said not to answer.
    fn ask<T>(&mut self, mut request: impl FnMut(&mut Io) -> Option<T>) -> Option<T> {
        if self.quiet() || self.replaced { return None; }
        if self.silent_until != 0 || self.device.returned() {
            if !self.back() { return None; }
        }
        let start = mind::time::uptime_ms() as u64;
        let result = request(&mut self.device);
        let now = mind::time::uptime_ms() as u64;
        if result.is_some() { return result; }
        // Slow: the drive does not answer (unplugged); a return it brings is checked when it is asked again.
        if now.saturating_sub(start) >= SLOW_MS {
            self.missing_since = start;
            self.not_answering(now);
            return None;
        }
        if !self.device.returned() { return None; }
        mind::println!("[VFS] THE {} DRIVE ANSWERED NOT FOUND: ITS MEDIUM IS COMPARED BEFORE ANYTHING ELSE IS ASKED OF IT", crate::device_name(self.device.kind()));
        self.missing_since = start;
        if self.back() { request(&mut self.device) } else { None }
    }

    fn not_answering(&mut self, now: u64) {
        if !self.said {
            mind::println!("[VFS] THE {} DRIVE DOES NOT ANSWER: REQUESTS FAIL AT ONCE, IT IS ASKED AGAIN EVERY {} S", crate::device_name(self.device.kind()), QUIET_MS / 1000);
            self.said = true;
        }
        self.silent_until = now + QUIET_MS;
    }

    // Back after it did not answer, or after a return: the buffer lent to the driver's current instance, then the medium
    // compared before anything else is asked of it. A fresh instance serves sectors only once init has set its windows
    // again, and a driver finds a device that came back only when a read asks for it, so a comparison that meets "not
    // found" is tried again with a later request, within BACK_MS of the first. False: not back yet; or another medium,
    // whose volumes are closed.
    fn back(&mut self) -> bool {
        let now = mind::time::uptime_ms() as u64;
        match if self.device.reattach().is_ok() { self.same_medium() } else { None } {
            Some(true) => {
                (self.silent_until, self.missing_since, self.said) = (0, 0, false);
                mind::println!("[VFS] THE {} DRIVE ANSWERS AGAIN WITH THE SAME MEDIUM", crate::device_name(self.device.kind()));
                true
            }
            Some(false) => { self.silent_until = 0; self.replace(); false }
            None if self.said || now.saturating_sub(self.missing_since) >= BACK_MS => { self.not_answering(now); false }
            None => { self.silent_until = now + RETRY_MS; false }
        }
    }

    fn line(&self, index: usize) -> &[u8] { &self.cache.as_slice()[index * SECTOR..(index + 1) * SECTOR] }

    // A drive back after it did not answer, or after a return, may hold another medium: a stick rewritten elsewhere and
    // put back, or another one (000-KRN-0079, 000-KRN-0085). Sector 0, the identity's sectors and every unchanged cached
    // sector must read as before. None: a read failed.
    fn same_medium(&mut self) -> Option<bool> {
        let mut checks: alloc::vec::Vec<(u32, [u8; 32])> = self.first.iter().map(|s| (0u32, mind::sha256::digest(s))).collect();
        checks.extend(self.identity.iter().copied());
        for index in (0..LINES).filter(|&i| self.tags[i] != u32::MAX && !self.dirty[i]) { checks.push((self.tags[index], mind::sha256::digest(self.line(index)))); }
        checks.sort_unstable_by_key(|&(lba, _)| lba);
        checks.dedup_by_key(|&mut (lba, _)| lba);
        for (lba, before) in &checks {
            let now = self.device.read(*lba as u64, 1).ok()?;
            if now.get(..SECTOR).map(mind::sha256::digest) != Some(*before) { return Some(false); }
        }
        Some(true)
    }

    // Another medium: nothing cached is written to it, and its volumes fail from now on.
    fn replace(&mut self) {
        let lost = self.dirty.iter().filter(|&&d| d).count();
        self.discard();
        self.replaced = true;
        mind::println!("[VFS] THE {} DRIVE CAME BACK WITH ANOTHER MEDIUM: ITS VOLUMES ARE CLOSED, {} CHANGED SECTORS OF THE OLD ONE NOT WRITTEN TO IT", crate::device_name(self.device.kind()), lost);
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

    // Writes the changed lines in LBA order, neighbours in one write. A run that fails stays dirty; what lies past the
    // driver's reach (the device's end, or its window's, 307-KRN-0229) can never be written: it is dropped, named in the
    // log, and one flush fails for it, this one or the next.
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
            let (lba, count) = (self.tags[dirty[at]] as u64, end - at);
            // The sectors written: fewer when the run was clipped at the end of the driver's reach, none when it began past it.
            let Some(written) = self.ask(|device| match device.write(lba, &run) { Ok(n) => Some(n.min(count)), Err(Error::Invalid) => Some(0), Err(_) => None }) else { return false };
            for &index in &dirty[at..at + written] { self.dirty[index] = false; }
            if written < count {
                mind::println!("[VFS] THE {} DRIVE REFUSED {} SECTORS FROM LBA {}: PAST ITS REACH, NOT WRITTEN", crate::device_name(self.device.kind()), count - written, lba + written as u64);
                for &index in &dirty[at + written..end] { (self.tags[index], self.dirty[index]) = (u32::MAX, false); }
                self.failed = true;
            }
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
        let read = |device: &mut Io| device.read(lba as u64, count).ok().map(|data| {
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
    // which are as new as the medium's or newer. Each request is asked like any other (a quiet or replaced drive is not
    // read), and while mounting the unchanged sectors join the identity (the boot manifest is read this way).
    fn read_run(&mut self, lba: u32, out: &mut [u8]) -> bool {
        let count = out.len() / SECTOR;
        let mut at = 0;
        while at < count {
            let (from, want) = (lba as u64 + at as u64, (count - at).min(BLOCK_MAX_SECTORS));
            let chunk = &mut out[at * SECTOR..(at + want) * SECTOR];
            let read = |device: &mut Io| device.read(from, want).ok().map(|data| {
                let got = (data.len() / SECTOR).min(want);
                chunk[..got * SECTOR].copy_from_slice(&data[..got * SECTOR]);
                got
            });
            let Some(got) = self.ask(read) else { return false };
            if got == 0 { return false; }
            at += got;
        }
        for (index, &tag) in self.tags.iter().enumerate() {
            if tag >= lba && ((tag - lba) as usize) < count { let i = (tag - lba) as usize; out[i * SECTOR..(i + 1) * SECTOR].copy_from_slice(&self.cache.as_slice()[index * SECTOR..(index + 1) * SECTOR]); }
        }
        if self.recording {
            for (i, sector) in out.chunks_exact(SECTOR).enumerate() {
                let at = lba + i as u32;
                if self.identity.len() >= IDENTITY { break; }
                if !self.tags.iter().zip(self.dirty).any(|(&tag, dirty)| tag == at && dirty) { self.note_read(at, sector.try_into().unwrap()); }
            }
        }
        true
    }

    // File data in requests of up to BLOCK_MAX_SECTORS, past the cache (000-KRN-0078). A cached copy of a sector a
    // request wrote is older than it and is dropped then, changed or not; a failed request leaves the rest cached.
    fn write_run(&mut self, lba: u32, data: &[u8]) -> bool {
        if self.replaced { return false; }
        let count = data.len() / SECTOR;
        let mut at = 0;
        while at < count {
            let (from, want) = (lba + at as u32, (count - at).min(BLOCK_MAX_SECTORS));
            let chunk = &data[at * SECTOR..(at + want) * SECTOR];
            let written = |tag: u32| tag >= from && tag - from < want as u32;
            self.identity.retain(|&(sector, _)| !written(sector)); // as `write` does: the medium may not hold them yet
            if self.ask(|device| (device.write(from as u64, chunk) == Ok(want)).then_some(())).is_none() { return false; }
            for index in 0..LINES { if written(self.tags[index]) { (self.tags[index], self.dirty[index]) = (u32::MAX, false); } }
            at += want;
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
    fn write_run(&mut self, lba: u32, data: &[u8]) -> bool { self.0.borrow_mut().write_run(lba, data) }
    fn sectors(&self) -> u64 { self.0.borrow().sectors() }
    fn writable(&self) -> bool { self.0.borrow().writable() }
}
