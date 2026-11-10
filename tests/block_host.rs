//! Host tests of the block protocol's server side (libmind/src/block_protocol.rs, idl/block.wit 2.0): writing needs
//! the write badge on the client's capability and a writable medium (Appendix B.6); the data must hold every sector
//! written; reads, clipping at the end, errors. A client of a window (307-DRV-0054) sees the window as the device and
//! nothing outside it; only init's windows badge sets one; a window not set gives nothing. Each client badge has a
//! buffer of its own (307-KRN-0177). A driver's failure reaches the client as io, invalid or not-found, never as peer
//! (000-KRN-0228).
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/arch/mod.rs"]
mod arch;
#[path = "../libmind/src/sys.rs"]
mod sys;
#[path = "../libmind/src/block_protocol.rs"]
mod block_protocol;
use abi::*;
use block_protocol::{badge_for, flush, read, sectors, window_of, writable, write, Buffers, Driver, DriverError, Outcome, Windows, BUFFERS, BADGE_WINDOW, BADGE_WINDOWS, BADGE_WRITE, WINDOWS};
use sys::Error;

struct Memory { data: Vec<u8>, protected: bool, flushes: usize, fail: Option<DriverError> }
impl Memory { fn new(sectors: usize) -> Self { Self { data: vec![0; sectors * BLOCK_SECTOR], protected: false, flushes: 0, fail: None } } }
impl Driver for Memory {
    fn sectors(&self) -> u64 { (self.data.len() / BLOCK_SECTOR) as u64 }
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> Outcome {
        let at = lba as usize * BLOCK_SECTOR;
        out[..count * BLOCK_SECTOR].copy_from_slice(&self.data[at..at + count * BLOCK_SECTOR]);
        self.fail.map_or(Ok(()), Err)
    }
    fn write(&mut self, lba: u64, count: usize, data: &[u8]) -> Outcome {
        if let Some(error) = self.fail { return Err(error); }
        let at = lba as usize * BLOCK_SECTOR;
        self.data[at..at + count * BLOCK_SECTOR].copy_from_slice(&data[..count * BLOCK_SECTOR]);
        Ok(())
    }
    fn flush(&mut self) -> Outcome { self.flushes += 1; self.fail.map_or(Ok(()), Err) }
    fn read_only(&self) -> bool { self.protected }
}

// A driver that only reads (the trait's defaults).
struct ReadOnly;
impl Driver for ReadOnly {
    fn sectors(&self) -> u64 { 16 }
    fn read(&mut self, _: u64, _: usize, _: &mut [u8]) -> Outcome { Ok(()) }
}

const WRITER: u16 = BADGE_WRITE;
// No window set: every client sees the whole device.
const WHOLE: Windows = Windows::new();

#[test]
fn a_driver_that_cannot_tell_reports_no_return() {
    // 000-KRN-0085: only a driver that notices its medium going and coming back answers true, and serve then ends it.
    assert!(!Memory::new(8).returned() && !ReadOnly.returned());
}

#[test]
fn writing_needs_the_badge() {
    let mut disk = Memory::new(64);
    let data = vec![0xA5u8; 4 * BLOCK_SECTOR];
    assert_eq!(write(&mut disk, &WHOLE, 0, &data, 4, 10), Err(Error::Rights), "no badge");
    assert_eq!(write(&mut disk, &WHOLE, 0x10, &data, 4, 10), Err(Error::Rights), "another badge");
    assert_eq!(write(&mut disk, &WHOLE, BADGE_WINDOWS, &data, 4, 10), Err(Error::Rights), "the windows badge does not write");
    assert_eq!(flush(&mut disk, &WHOLE, 0), Err(Error::Rights));
    assert!(disk.data.iter().all(|&b| b == 0) && disk.flushes == 0);
    assert_eq!(write(&mut disk, &WHOLE, WRITER, &data, 4, 10), Ok(4));
    assert_eq!(flush(&mut disk, &WHOLE, WRITER), Ok(()));
    assert_eq!(disk.flushes, 1);
    assert!(disk.data[10 * BLOCK_SECTOR..14 * BLOCK_SECTOR].iter().all(|&b| b == 0xA5));
    assert!(disk.data[14 * BLOCK_SECTOR..].iter().all(|&b| b == 0));
    // Reading needs no badge.
    let mut out = vec![0u8; 2 * BLOCK_SECTOR];
    assert_eq!(read(&mut disk, &WHOLE, 0, &mut out, 2, 12), Ok(2));
    assert!(out.iter().all(|&b| b == 0xA5));
}

#[test]
fn writable_tells_whether_the_client_can_write() {
    let mut disk = Memory::new(64);
    assert!(writable(WRITER, &disk, &WHOLE));
    assert!(!writable(0, &disk, &WHOLE));
    disk.protected = true;
    assert!(!writable(WRITER, &disk, &WHOLE));
    let data = vec![1u8; BLOCK_SECTOR];
    assert_eq!(write(&mut disk, &WHOLE, WRITER, &data, 1, 0), Err(Error::Rights), "write-protected medium");
    // A driver without a write path is read-only for everyone.
    assert!(!writable(WRITER, &ReadOnly, &WHOLE));
    assert_eq!(write(&mut ReadOnly, &WHOLE, WRITER, &data, 1, 0), Err(Error::Rights));
}

#[test]
fn requests_are_clipped_and_checked() {
    let mut disk = Memory::new(16);
    let data = vec![7u8; 8 * BLOCK_SECTOR];
    // At the end of the disk only what is there is written.
    assert_eq!(write(&mut disk, &WHOLE, WRITER, &data, 8, 12), Ok(4));
    // The sealed data must hold every sector asked for.
    assert_eq!(write(&mut disk, &WHOLE, WRITER, &data[..BLOCK_SECTOR], 2, 0), Err(Error::Invalid));
    assert_eq!(write(&mut disk, &WHOLE, WRITER, &data, 0, 0), Err(Error::Invalid), "no sectors");
    assert_eq!(write(&mut disk, &WHOLE, WRITER, &data, 1, 16), Err(Error::Invalid), "past the end");
    // Not beyond the attached buffer either.
    let mut small = vec![0u8; 2 * BLOCK_SECTOR];
    assert_eq!(read(&mut disk, &WHOLE, 0, &mut small, 8, 0), Ok(2));
    assert_eq!(read(&mut disk, &WHOLE, 0, &mut small, 1, 16), Err(Error::Invalid), "past the end");
    assert_eq!(read(&mut disk, &WHOLE, 0, &mut small, 0, 0), Err(Error::Invalid), "no sectors");
    assert_eq!(read(&mut disk, &WHOLE, 0, &mut [], 1, 0), Err(Error::Invalid), "an empty buffer");
    disk.fail = Some(DriverError::Io);
    assert_eq!(write(&mut disk, &WHOLE, WRITER, &data, 1, 0), Err(Error::Other(ERR_IO)), "a device error");
    assert_eq!(read(&mut disk, &WHOLE, 0, &mut small, 1, 0), Err(Error::Other(ERR_IO)));
}

#[test]
fn a_drivers_failure_reaches_the_client_as_io_invalid_or_not_found_never_peer() {
    // What a client is answered for each failure: only io is a drive's; peer stays a restart's (000-KRN-0228).
    let answers = [(DriverError::Io, Error::Other(ERR_IO)), (DriverError::Range, Error::Invalid), (DriverError::Gone, Error::NotFound)];
    let data = vec![0x5Au8; 2 * BLOCK_SECTOR];
    for (failure, answer) in answers {
        let mut disk = Memory::new(64);
        let mut windows = Windows::new();
        assert_eq!(windows.set(BADGE_WINDOWS, disk.sectors(), 1, 32, 16), Ok(()));
        disk.fail = Some(failure);
        for badge in [WRITER, badge_for(1, true)] {
            let mut target = vec![0u8; 2 * BLOCK_SECTOR];
            assert_eq!(read(&mut disk, &windows, badge, &mut target, 2, 0), Err(answer), "{failure:?}: read, badge {badge:#x}");
            assert_eq!(write(&mut disk, &windows, badge, &data, 2, 0), Err(answer), "{failure:?}: write, badge {badge:#x}");
            assert_eq!(flush(&mut disk, &windows, badge), Err(answer), "{failure:?}: flush, badge {badge:#x}");
        }
        assert!(disk.data.iter().all(|&b| b == 0), "{failure:?}: a failed write left nothing");
    }
    // The code the client decodes is ERR_IO, which libmind::sys gives back as the same error.
    assert_eq!(Error::from(DriverError::Io).code(), ERR_IO);
    assert_eq!(sys::check(ERR_IO), Err(Error::Other(ERR_IO)));
    // A driver that keeps the trait's write is never asked to write: it is read-only, so rights come first.
    assert_eq!(write(&mut ReadOnly, &WHOLE, WRITER, &data, 1, 0), Err(Error::Rights));
    assert_eq!(flush(&mut ReadOnly, &WHOLE, WRITER), Err(Error::Rights));
}

#[test]
fn a_windows_client_sees_its_window_as_the_device() {
    // A disk of 64 sectors; window 2 is sectors 20 to 29.
    let mut disk = Memory::new(64);
    let mut windows = Windows::new();
    assert_eq!(windows.set(BADGE_WINDOWS, disk.sectors(), 2, 20, 10), Ok(()));
    let (reader, writer) = (badge_for(2, false), badge_for(2, true));
    assert_eq!((window_of(reader), window_of(writer), writer & BADGE_WRITE), (2, 2, BADGE_WRITE));
    assert_eq!(sectors(&disk, &windows, writer), Ok(10));
    assert_eq!(sectors(&disk, &windows, WRITER), Ok(64), "window 0 is the whole device");
    // Its sector 0 is the device's sector 20; a write clipped at the window's end, not the device's.
    let data = vec![0x5Au8; 4 * BLOCK_SECTOR];
    assert_eq!(write(&mut disk, &windows, writer, &data, 4, 0), Ok(4));
    assert_eq!(write(&mut disk, &windows, writer, &data, 4, 8), Ok(2));
    let marked: Vec<usize> = (0..64).filter(|&k| disk.data[k * BLOCK_SECTOR] == 0x5A).collect();
    assert_eq!(marked, [20, 21, 22, 23, 28, 29]);
    // Nothing past its end, though the device goes on.
    assert_eq!(write(&mut disk, &windows, writer, &data, 1, 10), Err(Error::Invalid));
    let mut out = vec![0u8; 8 * BLOCK_SECTOR];
    assert_eq!(read(&mut disk, &windows, reader, &mut out, 1, 10), Err(Error::Invalid));
    assert_eq!(read(&mut disk, &windows, reader, &mut out, 8, 6), Ok(4), "clipped at the window's end");
    assert!(out[..2 * BLOCK_SECTOR].iter().all(|&b| b == 0) && out[2 * BLOCK_SECTOR..4 * BLOCK_SECTOR].iter().all(|&b| b == 0x5A));
    // The reader may not write; the writer flushes the drive.
    assert_eq!(write(&mut disk, &windows, reader, &data, 1, 0), Err(Error::Rights));
    assert!(writable(writer, &disk, &windows) && !writable(reader, &disk, &windows));
    assert_eq!(flush(&mut disk, &windows, writer), Ok(()));
    // Set again, the window moves: the same client then reaches sectors 40 to 49.
    assert_eq!(windows.set(BADGE_WINDOWS, disk.sectors(), 2, 40, 10), Ok(()));
    assert_eq!(write(&mut disk, &windows, writer, &data, 1, 0), Ok(1));
    assert_eq!(disk.data[40 * BLOCK_SECTOR], 0x5A);
    // Another window's client sees its own.
    assert_eq!(windows.set(BADGE_WINDOWS, disk.sectors(), 7, 0, 64), Ok(()));
    assert_eq!(sectors(&disk, &windows, badge_for(7, false)), Ok(64));
}

#[test]
fn a_window_not_set_gives_nothing() {
    // A restarted driver holds no window: its clients get not-found for every call, never the whole device.
    let mut disk = Memory::new(64);
    let windows = Windows::new();
    let data = vec![1u8; BLOCK_SECTOR];
    let mut out = vec![0u8; BLOCK_SECTOR];
    for window in 1..=WINDOWS as u8 {
        let badge = badge_for(window, true);
        assert_eq!(sectors(&disk, &windows, badge), Err(Error::NotFound));
        assert_eq!(read(&mut disk, &windows, badge, &mut out, 1, 0), Err(Error::NotFound));
        assert_eq!(write(&mut disk, &windows, badge, &data, 1, 0), Err(Error::NotFound));
        assert_eq!(flush(&mut disk, &windows, badge), Err(Error::NotFound));
        assert!(!writable(badge, &disk, &windows));
    }
    assert!(disk.data.iter().all(|&b| b == 0) && disk.flushes == 0);
    // The window bits are the badge's bits 1 to 3, apart from the write bit.
    assert_eq!(BADGE_WINDOW, 0b1110);
    assert_eq!(badge_for(8, true), BADGE_WRITE, "no ninth window");
}

#[test]
fn only_the_windows_badge_sets_a_window() {
    let disk = Memory::new(64);
    let mut windows = Windows::new();
    for badge in [0, BADGE_WRITE, badge_for(1, true), BADGE_WINDOWS | BADGE_WRITE, BADGE_WINDOWS | badge_for(3, false), 0x7fff] {
        assert_eq!(windows.set(badge, disk.sectors(), 1, 0, 8), Err(Error::Rights), "badge {:#x}", badge);
    }
    assert_eq!(windows, Windows::new(), "nothing set");
    // Out of range: no window 0 or 8, no empty window, none past the device, none whose end overflows.
    for (index, first, count) in [(0, 0, 8), (8, 0, 8), (1, 0, 0), (1, 60, 5), (1, 64, 1), (1, u64::MAX, 2)] {
        assert_eq!(windows.set(BADGE_WINDOWS, disk.sectors(), index, first, count), Err(Error::Invalid), "{} {} {}", index, first, count);
    }
    assert_eq!(windows, Windows::new());
    assert_eq!(windows.set(BADGE_WINDOWS, disk.sectors(), 1, 60, 4), Ok(()), "up to the device's last sector");
    assert_eq!(sectors(&disk, &windows, badge_for(1, false)), Ok(4));
}

#[test]
fn each_client_badge_reads_into_a_buffer_of_its_own() {
    // vfs_server's whole disk (BADGE_WRITE), blockstore's window 1 and init's look at the table (0) on one driver: a read
    // for one lands in its own buffer, whoever attached last (307-KRN-0177).
    let mut disk = Memory::new(64);
    let mut windows = Windows::new();
    assert_eq!(windows.set(BADGE_WINDOWS, disk.sectors(), 1, 32, 32), Ok(()));
    for lba in 0..64 { disk.data[lba * BLOCK_SECTOR] = lba as u8; }
    let mut buffers: Buffers<Vec<u8>> = Buffers::new();
    let (vfs, store, init) = (WRITER, badge_for(1, true), 0);
    for badge in [vfs, store, init] { assert_eq!(buffers.attach(badge, || Ok(vec![0u8; 2 * BLOCK_SECTOR])), Ok(())); }
    for (badge, lba, sector) in [(vfs, 3, 3u8), (store, 3, 35), (init, 0, 0)] {
        let target = buffers.get(badge).unwrap();
        assert_eq!(read(&mut disk, &windows, badge, target, 1, lba), Ok(1));
        assert_eq!(target[0], sector, "badge {badge:#x}'s read, window-relative for the store");
    }
    assert_eq!((buffers.get(vfs).unwrap()[0], buffers.get(store).unwrap()[0]), (3, 35), "each read stayed in its client's buffer");
    // Attaching again replaces the badge's own buffer and leaves the others'.
    assert_eq!(buffers.attach(store, || Ok(vec![0xEE; BLOCK_SECTOR])), Ok(()));
    assert_eq!((buffers.get(vfs).unwrap()[0], buffers.get(store).unwrap()[0], buffers.get(store).unwrap().len()), (3, 0xEE, BLOCK_SECTOR));
    // A badge that attached none has none, and with every place another badge's, attach is refused and changes nothing.
    assert!(buffers.get(0x10).is_none());
    for badge in 0x20..0x20 + (BUFFERS - 3) as u16 { assert_eq!(buffers.attach(badge, || Ok(vec![1])), Ok(())); }
    let mut called = false;
    assert_eq!(buffers.attach(0x40, || { called = true; Ok(vec![2]) }), Err(Error::Other(ERR_LIMIT)));
    assert!(!called && buffers.get(0x40).is_none() && buffers.get(vfs).unwrap()[0] == 3);
    assert_eq!(buffers.attach(vfs, || Ok(vec![9])), Ok(()), "its own place is always free to it");
    // A mapping that fails leaves the badge without a buffer, as before the change.
    assert_eq!(buffers.attach(vfs, || Err(Error::Invalid)), Err(Error::Invalid));
    assert!(buffers.get(vfs).is_none());
}
