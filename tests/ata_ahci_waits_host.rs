//! `ata` and `ahci` against stand-ins (000-DRV-0064): a drive that never leaves BSY fails its request at the deadline,
//! a request outside one command, the drive or the buffer is refused before the ports are touched, and AHCI's handoff
//! ends taken, silent or kept by the firmware's bits. AHCI's ports against a model of their registers (307-DRV-0068):
//! FIS receive on before SIG is read, SIG awaited to its deadline, a COMRESET when none came, a port that never answers
//! skipped, and a port the firmware had started still found.
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

/// An AHCI port as QEMU models it (AHCI 1.3.1, 3.3.9): SIG holds `signature` only once the device's D2H register FIS
/// is received, which needs FB set and FRE on. The FIS comes `answer` ns after FRE goes on (None: never), and
/// `after_reset` ns after a COMRESET ends with FRE on. CR and FR follow ST and FRE, or stay set if `stuck`.
struct Sata {
    signature: u32, answer: Option<u64>, after_reset: Option<u64>, stuck: bool,
    cmd: Cell<u32>, clb: Cell<u64>, fb: Cell<u64>, sctl: Cell<u32>, ssts: Cell<u32>,
    received: Cell<bool>, armed: Cell<Option<u64>>, resets: Cell<u32>,
}

impl Sata {
    fn new(signature: u32, answer: Option<u64>, after_reset: Option<u64>) -> Self {
        Sata { signature, answer, after_reset, stuck: false, cmd: Cell::new(0), clb: Cell::new(0), fb: Cell::new(0),
               sctl: Cell::new(0x300), ssts: Cell::new(0x113), received: Cell::new(false), armed: Cell::new(None), resets: Cell::new(0) }
    }
    /// As OVMF leaves a port: started on its own areas, the signature received.
    fn started(self) -> Self {
        self.cmd.set(checks::CMD_ST | checks::CMD_FRE); self.clb.set(0x7F00_0000); self.fb.set(0x7F00_1000); self.received.set(true);
        self
    }
    fn empty() -> Self { let s = Sata::new(0, None, None); s.ssts.set(0); s }
}

/// Our command list and FIS area, above 4 GiB so that the upper words count.
const LIST: u64 = 0x1_2345_0000; const FIS: u64 = 0x1_2345_0400;
const ATAPI: u32 = 0xEB14_0101;

/// An HBA of such ports, the clock as above, and every access in order: (port, register, the value written or None for a
/// read, the time).
struct Hba { clock: Cell<u64>, ports: Vec<Sata>, log: RefCell<Vec<(usize, usize, Option<u32>, u64)>> }

impl Hba {
    fn new(ports: Vec<Sata>) -> Self { Hba { clock: Cell::new(0), ports, log: RefCell::new(Vec::new()) } }
    /// Port `p`'s writes to `offset`: their place in the log, the value and the time.
    fn writes(&self, p: usize, offset: usize) -> Vec<(usize, u32, u64)> {
        self.log.borrow().iter().enumerate().filter_map(|(i, &(q, o, v, t))| (q == p && o == offset).then_some(v.map(|v| (i, v, t))).flatten()).collect()
    }
    fn first_read(&self, p: usize, offset: usize) -> Option<usize> {
        self.log.borrow().iter().position(|&(q, o, v, _)| q == p && o == offset && v.is_none())
    }
    fn disk(&self, implemented: u32) -> (Option<usize>, Vec<(usize, checks::Answer)>) {
        let mut heard = Vec::new();
        let port = checks::disk_port(self, implemented, LIST, FIS, |p, a| heard.push((p, a)));
        (port, heard)
    }
}

impl checks::Clock for Hba {
    fn now_ns(&self) -> u64 { self.clock.set(self.clock.get() + 1_000); self.clock.get() }
    fn sleep(&self) { self.clock.set(self.clock.get() + 1_000_000); }
}

impl checks::Ports for Hba {
    fn reg(&self, p: usize, offset: usize) -> u32 {
        let s = &self.ports[p];
        let delay = if s.resets.get() == 0 { s.answer } else { s.after_reset };
        if let (Some(at), Some(d)) = (s.armed.get(), delay) { if self.clock.get() >= at + d { s.received.set(true); s.armed.set(None); } }
        self.log.borrow_mut().push((p, offset, None, self.clock.get()));
        let cmd = s.cmd.get();
        match offset {
            checks::CMD => cmd | if cmd & checks::CMD_ST != 0 || s.stuck { checks::CMD_CR } else { 0 } | if cmd & checks::CMD_FRE != 0 || s.stuck { checks::CMD_FR } else { 0 },
            checks::TFD => if s.received.get() { 0x50 } else { 0x7F },
            checks::SIG => if s.received.get() { s.signature } else { checks::NO_SIGNATURE },
            checks::SSTS => s.ssts.get(),
            checks::SCTL => s.sctl.get(),
            _ => 0,
        }
    }
    fn set(&self, p: usize, offset: usize, value: u32) {
        let s = &self.ports[p];
        let now = self.clock.get();
        self.log.borrow_mut().push((p, offset, Some(value), now));
        let low = |a: u64| a & !0xFFFF_FFFF | value as u64; let high = |a: u64| a & 0xFFFF_FFFF | (value as u64) << 32;
        match offset {
            checks::CMD => {
                let (before, after) = (s.cmd.get(), value & !(checks::CMD_CR | checks::CMD_FR));
                s.cmd.set(after);
                if before & checks::CMD_FRE == 0 && after & checks::CMD_FRE != 0 && s.fb.get() != 0 && !s.received.get() { s.armed.set(Some(now)); }
                if after & checks::CMD_FRE == 0 { s.armed.set(None); }
            }
            checks::CLB => s.clb.set(low(s.clb.get())),
            o if o == checks::CLB + 4 => s.clb.set(high(s.clb.get())),
            checks::FB => s.fb.set(low(s.fb.get())),
            o if o == checks::FB + 4 => s.fb.set(high(s.fb.get())),
            checks::SCTL => {
                let before = s.sctl.get() & 0xF;
                s.sctl.set(value);
                // DET 1: the link down, TFD 7Fh and the signature gone; DET 1 to 0: the link up, the FIS on its way.
                if value & 0xF == 1 { s.received.set(false); s.armed.set(None); s.ssts.set(s.ssts.get() & !0xF); }
                if before == 1 && value & 0xF == 0 {
                    s.resets.set(s.resets.get() + 1); s.ssts.set(s.ssts.get() | 3);
                    if s.cmd.get() & checks::CMD_FRE != 0 && s.fb.get() != 0 { s.armed.set(Some(now)); }
                }
            }
            _ => {}
        }
    }
}

#[test]
fn ahcis_port_receives_its_fis_before_sig_is_read() {
    let hba = Hba::new(vec![Sata::new(checks::DISK, Some(0), None)]);
    assert_eq!(hba.disk(1), (Some(0), vec![(0, checks::Answer::Signature(checks::DISK, false))]));
    let fre = hba.writes(0, checks::CMD).into_iter().find(|&(_, v, _)| v & checks::CMD_FRE != 0).expect("FRE set").0;
    let sig = hba.first_read(0, checks::SIG).expect("SIG read");
    assert!(fre < sig, "FRE at {fre}, SIG first read at {sig}");
    for offset in [checks::CLB, checks::CLB + 4, checks::FB, checks::FB + 4, checks::SERR] {
        assert!(hba.writes(0, offset).iter().any(|&(i, _, _)| i < fre), "{offset:#x} not written before FRE");
    }
    assert_eq!((hba.ports[0].clb.get(), hba.ports[0].fb.get()), (LIST, FIS));
    assert!(hba.writes(0, checks::SCTL).is_empty());
    // The disk keeps FIS receive on for start(); ST is start()'s.
    assert_eq!(hba.ports[0].cmd.get(), checks::CMD_FRE);
}

#[test]
fn ahcis_signature_is_awaited_to_its_deadline() {
    let hba = Hba::new(vec![Sata::new(checks::DISK, Some(3_000_000_000), None)]);
    assert_eq!(hba.disk(1).1, vec![(0, checks::Answer::Signature(checks::DISK, false))]);
    let at = hba.clock.get();
    assert!((3_000_000_000..3_002_000_000).contains(&at), "{at}");
    assert!(hba.writes(0, checks::SCTL).is_empty());
}

#[test]
fn ahci_resets_a_port_whose_signature_does_not_come() {
    let hba = Hba::new(vec![Sata::new(checks::DISK, None, Some(5_000_000))]);
    assert_eq!(hba.disk(1), (Some(0), vec![(0, checks::Answer::Signature(checks::DISK, true))]));
    // DET 1 at the deadline, for at least 1 ms, then 0, the other bits kept; ST never set meanwhile, FRE kept on.
    let sctl = hba.writes(0, checks::SCTL);
    assert_eq!(sctl.iter().map(|&(_, v, _)| v).collect::<Vec<_>>(), vec![0x301, 0x300]);
    let ((on, _, t1), (off, _, t2)) = (sctl[0], sctl[1]);
    assert!(t1 >= checks::SIGNATURE_NS && t1 < checks::SIGNATURE_NS + 2_000_000, "{t1}");
    assert!(t2 - t1 >= checks::COMRESET_NS, "{t1} {t2}");
    assert!(hba.writes(0, checks::CMD).iter().all(|&(i, v, _)| i < on || v & checks::CMD_ST == 0));
    assert_eq!(hba.ports[0].resets.get(), 1);
    // SERR cleared once the link is back, and the signature 5 ms after it.
    assert!(hba.writes(0, checks::SERR).iter().any(|&(i, _, _)| i > off));
    assert!(hba.clock.get() - t2 >= 5_000_000 && hba.clock.get() - t2 < 8_000_000, "{}", hba.clock.get() - t2);
}

#[test]
fn ahci_skips_a_port_that_never_answers() {
    let hba = Hba::new(vec![Sata::new(checks::DISK, None, None), Sata::new(checks::DISK, Some(0), None)]);
    let silent = checks::Answer::Silent { ssts: 0x113, tfd: 0x7F, sig: checks::NO_SIGNATURE };
    assert_eq!(hba.disk(0b11), (Some(1), vec![(0, silent), (1, checks::Answer::Signature(checks::DISK, false))]));
    // Two waits and one COMRESET, then left with FIS receive off.
    assert_eq!(hba.ports[0].resets.get(), 1);
    let at = hba.clock.get();
    assert!(at >= 2 * checks::SIGNATURE_NS + checks::COMRESET_NS && at < 2 * checks::SIGNATURE_NS + 10_000_000, "{at}");
    assert_eq!(hba.ports[0].cmd.get() & checks::CMD_FRE, 0);
    // Nothing anywhere: no disk, after the same.
    let hba = Hba::new(vec![Sata::new(checks::DISK, None, None)]);
    assert_eq!(hba.disk(1).0, None);
}

#[test]
fn ahci_still_finds_a_port_the_firmware_started() {
    let hba = Hba::new(vec![Sata::new(checks::DISK, None, None).started()]);
    assert_eq!(hba.disk(1), (Some(0), vec![(0, checks::Answer::Signature(checks::DISK, false))]));
    // Stopped, ST before FRE, before its areas became ours; no COMRESET, and at once. (CR and FR, read-only, are written
    // back as read.)
    let cmd = hba.writes(0, checks::CMD);
    let (st, fre) = (checks::CMD_ST, checks::CMD_FRE);
    assert_eq!(cmd.iter().map(|&(_, v, _)| v & (st | fre)).collect::<Vec<_>>(), vec![fre, 0, fre]);
    assert!(cmd[1].0 < hba.writes(0, checks::CLB)[0].0 && cmd[1].0 < hba.writes(0, checks::FB)[0].0);
    assert_eq!((hba.ports[0].clb.get(), hba.ports[0].fb.get()), (LIST, FIS));
    assert!(hba.writes(0, checks::SCTL).is_empty());
    assert!(hba.clock.get() < 1_000_000, "{}", hba.clock.get());
}

#[test]
fn ahci_passes_ports_without_a_disk() {
    let stuck = Sata { stuck: true, ..Sata::new(checks::DISK, Some(0), None) }.started();
    let hba = Hba::new(vec![Sata::empty(), Sata::new(ATAPI, Some(0), None), stuck, Sata::new(checks::DISK, Some(0), None)]);
    assert_eq!(hba.disk(0b1111), (Some(3), vec![(1, checks::Answer::Signature(ATAPI, false)), (2, checks::Answer::Stuck), (3, checks::Answer::Signature(checks::DISK, false))]));
    // No device: SSTS read, nothing written. A device that is no disk: stopped again. A port that does not stop: its
    // areas left as they were, and not asked to stop twice.
    assert!(hba.log.borrow().iter().all(|&(p, o, v, _)| p != 0 || (o == checks::SSTS && v.is_none())));
    assert_eq!(hba.ports[1].cmd.get() & checks::CMD_FRE, 0);
    assert!(hba.writes(2, checks::CLB).is_empty() && hba.writes(2, checks::FB).is_empty());
    assert_eq!(hba.writes(2, checks::CMD).len(), 1);
    // An implemented port only.
    assert_eq!(hba.disk(0b0111).0, None);
}
