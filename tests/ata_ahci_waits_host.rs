//! `ata` and `ahci` against stand-ins (000-DRV-0064): a drive that never leaves BSY fails its request at the deadline,
//! a request outside one command, the drive or the buffer is refused before the ports are touched, and AHCI's handoff
//! ends taken, silent or kept by the firmware's bits.
#![allow(dead_code)] // the drivers' probes and deadlines are not all driven here
#[path = "../ata/src/ide.rs"]
mod ide;
#[path = "../ahci/src/checks.rs"]
mod checks;

use std::cell::{Cell, RefCell};

/// A primary channel whose status is `status`, a clock that moves `step` ns at each reading and 1 ms at each sleep,
/// and the writes it saw.
struct Channel { status: Cell<u8>, step: u64, clock: Cell<u64>, sleeps: Cell<u32>, writes: RefCell<Vec<(u16, u8)>>, words: RefCell<Vec<u16>> }

impl Channel {
    fn new(status: u8) -> Self {
        Channel { status: Cell::new(status), step: 1_000, clock: Cell::new(0), sleeps: Cell::new(0), writes: RefCell::new(Vec::new()), words: RefCell::new(Vec::new()) }
    }
}

impl ide::Bus for Channel {
    fn in8(&self, port: u16) -> u8 { if port == ide::COMMAND || port == ide::CONTROL { self.status.get() } else { 0 } }
    fn out8(&self, port: u16, value: u8) { self.writes.borrow_mut().push((port, value)); }
    fn read_words(&self, _port: u16, words: &mut [u16]) -> bool { for (i, w) in words.iter_mut().enumerate() { *w = i as u16; } true }
    fn write_words(&self, _port: u16, words: &[u16]) -> bool { self.words.borrow_mut().extend_from_slice(words); true }
    fn now_ns(&self) -> u64 { self.clock.set(self.clock.get() + self.step); self.clock.get() }
    fn sleep(&self) { self.sleeps.set(self.sleeps.get() + 1); self.clock.set(self.clock.get() + 1_000_000); }
}

fn disk(status: u8) -> ide::Ide<Channel> { ide::Ide { bus: Channel::new(status), sectors: 1_000 } }

#[test]
fn a_drive_that_stays_busy_fails_at_the_deadline() {
    let d = disk(ide::BSY);
    let mut out = [0u8; 512];
    assert_eq!(d.read(0, 1, &mut out), Err(ide::Fault::Busy));
    let at = d.bus.clock.get();
    assert!(at >= ide::COMMAND_NS && at < ide::COMMAND_NS + 2_000_000, "{at}");
    assert!(d.bus.sleeps.get() as u64 >= (ide::COMMAND_NS - ide::SPIN_NS) / 1_000_000 / 2, "{}", d.bus.sleeps.get());
    // The command was never written: its start waits for the drive.
    assert!(d.bus.writes.borrow().iter().all(|&(port, _)| port != ide::COMMAND));
    let d = disk(ide::BSY);
    assert_eq!(d.flush(), Err(ide::Fault::Busy));
    assert!(d.bus.clock.get() < ide::COMMAND_NS + 2_000_000);
}

#[test]
fn a_request_is_checked_before_the_ports() {
    let d = disk(ide::DRQ);
    let mut out = [0u8; 512 * 300];
    for (lba, count, len) in [(0, 0, 512), (0, 257, 512 * 300), (0, 2, 512), (999, 2, 1024), (u64::MAX, 1, 512)] {
        assert_eq!(d.read(lba, count, &mut out[..len]), Err(ide::Fault::Refused), "{lba} {count} {len}");
        assert_eq!(d.write(lba, count, &out[..len]), Err(ide::Fault::Refused), "{lba} {count} {len}");
    }
    assert!(d.bus.writes.borrow().is_empty());
    // 256 sectors are one command, its count register 0.
    assert_eq!(d.read(0, 256, &mut out[..512 * 256]), Ok(()));
    assert!(d.bus.writes.borrow().contains(&(ide::COUNT, 0)));
}

#[test]
fn a_ready_drive_reads_and_writes() {
    let d = disk(ide::DRQ);
    let mut out = [0u8; 1024];
    assert_eq!(d.read(5, 2, &mut out), Ok(()));
    assert_eq!((out[2], out[3], out[512 + 2]), (1, 0, 1));
    assert!(d.bus.writes.borrow().contains(&(ide::COMMAND, 0x20)) && d.bus.writes.borrow().contains(&(ide::LBA0, 5)));
    assert_eq!(d.write(5, 2, &out), Ok(()));
    assert_eq!(d.bus.words.borrow().len(), 512);
    assert_eq!(d.flush(), Ok(()));
    assert_eq!(disk(ide::ERR).read(0, 1, &mut out[..512]), Err(ide::Fault::Error));
}

#[test]
fn ahci_refuses_what_its_data_area_or_the_buffer_cannot_hold() {
    let room = 64 * 1024;
    assert_eq!(checks::data_bytes(128, 128 * 512, room), Some(65536));
    for (count, buffer) in [(0, 512), (129, 129 * 512), (2, 512), (0x1_0000, usize::MAX), (usize::MAX, usize::MAX)] {
        assert_eq!(checks::data_bytes(count, buffer, room), None, "{count} {buffer}");
    }
}

/// BOHC whose BOS clears at `released` ns and whose BB is set from `busy.0` to `busy.1`; the clock as above.
struct Firmware { clock: Cell<u64>, bohc: Cell<u32>, released: Option<u64>, busy: (u64, u64) }

impl checks::Clock for Firmware {
    fn now_ns(&self) -> u64 { self.clock.set(self.clock.get() + 1_000); self.clock.get() }
    fn sleep(&self) { self.clock.set(self.clock.get() + 1_000_000); }
}
impl checks::Bohc for Firmware {
    fn bohc(&self) -> u32 {
        let t = self.clock.get();
        let bos = if self.released.is_some_and(|r| t >= r) { 0 } else { checks::BOS };
        let bb = if t >= self.busy.0 && t < self.busy.1 { checks::BB } else { 0 };
        self.bohc.get() & checks::OOS | bos | bb
    }
    fn set_bohc(&self, value: u32) { self.bohc.set(value); }
}

fn handoff(released: Option<u64>, busy: (u64, u64)) -> checks::Handoff {
    let f = Firmware { clock: Cell::new(0), bohc: Cell::new(0), released, busy };
    let h = checks::handoff(&f);
    assert!(f.bohc.get() & checks::OOS != 0);
    h
}

#[test]
fn ahcis_handoff() {
    assert_eq!(handoff(Some(5_000_000), (0, 0)), checks::Handoff::Taken(5));
    // Not busy and silent: taken after 25 ms, as AHCI 10.6.3 allows.
    assert!(matches!(handoff(None, (0, 0)), checks::Handoff::Silent(25)));
    // Busy for a second, then released: waited for.
    assert!(matches!(handoff(Some(1_000_000_000), (0, 1_000_000_000)), checks::Handoff::Taken(ms) if (1_000..1_010).contains(&ms)));
    // Busy past 2 s more: left to the firmware.
    assert!(matches!(handoff(None, (0, u64::MAX)), checks::Handoff::Kept(ms) if (2_025..2_040).contains(&ms)));
}

#[test]
fn ahcis_wait_ends_at_its_deadline() {
    let f = Firmware { clock: Cell::new(0), bohc: Cell::new(0), released: None, busy: (0, 0) };
    assert!(!checks::wait(&f, checks::STOP_NS, || false));
    let at = f.clock.get();
    assert!(at >= checks::STOP_NS && at < checks::STOP_NS + 2_000_000, "{at}");
    assert!(checks::wait(&f, checks::STOP_NS, || true));
}
