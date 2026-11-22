//! Step 2 of 213-DRV-0095: HDMI-A-1's EDID read over its DDC line (DC_GPIO_DDC1, the VBIOS's I2C line 0) through DCN
//! 2.1's I2C engine, in Display Core's order (dce_i2c_hw.c, hw_ddc.c, hw_gpio.c; Linux v7.0): the pads to hardware I2C,
//! the engine taken, its speed and set-up, then each 128-byte block as a write of its offset and a read. Then the
//! engine is given back as `release_engine` does, and every register written is put back as it was, so that the
//! engine is left as step 1's batch 11 found it. A failure (the engine held, no grant, a NACK, a timeout, no answer
//! within the limit, a bad block) ends the step there with the same give-back: no output is touched, and steps 3 and 4
//! go on as without it. `tests/display_ddc_host.rs` checks it on a model of the engine.

use crate::batches::{Bus, Out, Store};
use crate::dcn::{field::DC_HPD_SENSE, Field, Reg, R};
use crate::edid::{Edid, BLOCK};
use crate::light::{ddc_writable, Clock, Name, Poke, Writable};
use core::fmt;

const fn f(shift: u8, width: u8) -> Field { Field { shift, width } }
// DC_I2C_CONTROL (dcn_2_1_0_sh_mask.h).
const GO: Field = f(0, 1);
const SOFT_RESET: Field = f(1, 1);
const SEND_RESET: Field = f(2, 1);
const SW_STATUS_RESET: Field = f(3, 1);
const DDC_SELECT: Field = f(8, 3);
const TRANSACTION_COUNT: Field = f(20, 2);
// DC_I2C_ARBITRATION.
const RW_CNTL_STATUS: Field = f(2, 2);
const NO_QUEUED_SW_GO: Field = f(4, 1);
const SW_USE_I2C_REG_REQ: Field = f(20, 1);
const SW_DONE_USING_I2C_REG: Field = f(21, 1);
// DC_I2C_SW_STATUS and DC_I2C_DDC1_HW_STATUS.
const STATUS: Field = f(0, 2);
const SW_DONE: u32 = 1 << 2;
const SW_ABORTED: u32 = 1 << 4;
const SW_TIMEOUT: u32 = 1 << 5;
const SW_STOPPED_ON_NACK: u32 = 1 << 8;
// DC_I2C_DDC1_SPEED.
const THRESHOLD: Field = f(0, 2);
const START_STOP_TIMING_CNTL: Field = f(8, 2);
const PRESCALE: Field = f(16, 16);
// DC_I2C_DDC1_SETUP.
const DATA_DRIVE_EN: Field = f(0, 1);
const DATA_DRIVE_SEL: Field = f(1, 1);
const SEND_RESET_LENGTH: Field = f(2, 1);
const ENABLE: Field = f(6, 1);
const CLK_DRIVE_EN: Field = f(7, 1);
const INTRA_BYTE_DELAY: Field = f(8, 8);
const INTRA_TRANSACTION_DELAY: Field = f(16, 8);
const TIME_LIMIT: Field = f(24, 8);
// DC_I2C_TRANSACTIONn.
const RW: Field = f(0, 1);
const STOP_ON_NACK: Field = f(8, 1);
const START: Field = f(12, 1);
const STOP: Field = f(13, 1);
const COUNT: Field = f(16, 10);
// DC_I2C_DATA: read or write, a byte, the buffer's index and whether a write sets it.
const DATA_RW: u32 = 1;
const DATA: Field = f(8, 8);
const INDEX: Field = f(16, 10);
const INDEX_WRITE: u32 = 1 << 31;
// MICROSECOND_TIME_BASE_DIV.
const TIME_BASE: Field = f(0, 7);
const XTAL_REF_DIV: Field = f(8, 7);
// DC_GPIO_DDC1_MASK: each pin's mask (0: the hardware drives it), its pull-down enable, and the pad's AUX mode.
const CLK_MASK: Field = f(0, 1);
const CLK_PD_EN: Field = f(4, 1);
const DATA_MASK: Field = f(8, 1);
const DATA_PD_EN: Field = f(12, 1);
const AUX_PAD1_MODE: Field = f(16, 1);

// enum dc_i2c_status (dce_i2c_hw.h).
const IDLE: u32 = 0;
const USED_BY_SW: u32 = 1;
const USED_BY_HW: u32 = 2;

/// The EDID's I2C address (VESA E-DDC).
pub const ADDRESS: u32 = 0x50;
/// Display Core's speed for DCN 2.1 (`dc->caps.i2c_speed_in_khz`, dcn21_resource.c).
pub const SPEED_KHZ: u32 = 100;
/// The time base's source when it reads 0: the 48 MHz crystal (REFCLK_CNTL 0, step 1's batch 10).
pub const CRYSTAL_KHZ: u32 = 48_000;
/// `I2C_SETUP_TIME_LIMIT_DCN`.
pub const SETUP_TIME_LIMIT: u32 = 3;
/// How long one block may take: about 12 ms at 100 kHz, so this bounds an engine that never answers.
pub const WAIT_LIMIT_US: u64 = 500_000;
const POLL_US: u32 = 100;
/// The blocks read: the base block and one extension (the CTA's); more would need E-DDC's segment pointer.
pub const MAX_BLOCKS: usize = 2;
const MAX_WRITES: usize = 48;
/// The registers put back after the read and checked against batch 11: all but the arbitration (given back by its
/// own bits) and the buffer.
const RESTORED: [(R, u8); 6] = [(R::DcGpioDdc1Mask, 0), (R::DcI2cControl, 0), (R::DcI2cDdc1Speed, 0), (R::DcI2cDdc1Setup, 0),
                                (R::DcI2cTransaction, 0), (R::DcI2cTransaction, 1)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// Step 1's batch 11 was not read: the engine as found is not known, so nothing is written.
    NotRead,
    /// HPD 1 shows no monitor.
    NoMonitor,
    /// The engine is held by the hardware or the DMCU (arbitration, DDC1's hardware status).
    Held { arbitration: u32, hw_status: u32 },
    /// The engine was asked for and not granted.
    NotGranted { status: u32 },
    /// A transfer was still running before a block's.
    Busy { block: u8, status: u32 },
    /// The monitor did not acknowledge.
    Nack { block: u8, status: u32 },
    /// The engine's own timeout or abort.
    Timeout { block: u8, status: u32 },
    Aborted { block: u8, status: u32 },
    /// No end of the transfer within `WAIT_LIMIT_US`.
    NoAnswer { block: u8, status: u32 },
    Unreadable(u32),
    Refused(u32),
    /// The base block's header, or a block's checksum.
    Header,
    Checksum { block: u8 },
    JournalFull,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Failure::NotRead => f.write_str("STEP 1'S BATCH 11 WAS NOT READ: THE ENGINE AS FOUND IS NOT KNOWN"),
            Failure::NoMonitor => f.write_str("HPD 1 SHOWS NO MONITOR"),
            Failure::Held { arbitration, hw_status } => write!(f, "THE ENGINE IS HELD (ARBITRATION {:08X}, DDC1 HW STATUS {:08X})", arbitration, hw_status),
            Failure::NotGranted { status } => write!(f, "THE ENGINE WAS NOT GRANTED (STATUS {})", status),
            Failure::Busy { block, status } => write!(f, "BLOCK {}: A TRANSFER STILL RUNS (SW STATUS {:08X})", block, status),
            Failure::Nack { block, status } => write!(f, "BLOCK {}: NO ACKNOWLEDGE (SW STATUS {:08X})", block, status),
            Failure::Timeout { block, status } => write!(f, "BLOCK {}: THE ENGINE TIMED OUT (SW STATUS {:08X})", block, status),
            Failure::Aborted { block, status } => write!(f, "BLOCK {}: THE TRANSFER WAS ABORTED (SW STATUS {:08X})", block, status),
            Failure::NoAnswer { block, status } => write!(f, "BLOCK {}: NO END WITHIN {} MS (SW STATUS {:08X})", block, WAIT_LIMIT_US / 1000, status),
            Failure::Unreadable(o) => write!(f, "{:05X} COULD NOT BE READ", o),
            Failure::Refused(o) => write!(f, "A WRITE TO {:05X}, OUTSIDE STEP 2'S REGISTERS, WAS REFUSED", o),
            Failure::Header => f.write_str("THE BASE BLOCK HAS NO EDID HEADER"),
            Failure::Checksum { block } => write!(f, "BLOCK {}'S CHECKSUM IS WRONG", block),
            Failure::JournalFull => f.write_str("MORE WRITES THAN STEP 2 KEEPS"),
        }
    }
}

/// What the read gave: the blocks, as read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Read { pub bytes: [u8; MAX_BLOCKS * BLOCK], pub blocks: usize }
impl Read { pub fn bytes(&self) -> &[u8] { &self.bytes[..self.blocks * BLOCK] } }

struct Step2<'a, E: Bus + Poke + Clock> {
    e: &'a mut E,
    out: &'a mut dyn Out,
    // Each write's register, its value before, and whether the give-back puts it back.
    journal: [Option<(Writable, u32, bool)>; MAX_WRITES],
    len: usize,
    granted: bool,
}

// Every register and instance here is one of the map's.
fn at(r: R, i: u8) -> Reg { match r.at(i) { Some(x) => x, None => unreachable!() } }

impl<E: Bus + Poke + Clock> Step2<'_, E> {
    fn read(&mut self, r: R, i: u8) -> Result<u32, Failure> {
        let x = at(r, i);
        self.e.read(x).ok_or(Failure::Unreadable(x.offset()))
    }
    // A read-modify-write, logged, its value before kept; `back`: the give-back writes that value back.
    fn update(&mut self, r: R, i: u8, fields: &[(Field, u32)], back: bool) -> Result<u32, Failure> {
        let x = at(r, i);
        let w = ddc_writable(x).map_err(|_| Failure::Refused(x.offset()))?;
        let old = self.read(r, i)?;
        let v = fields.iter().fold(old, |v, (f, val)| (v & !f.mask()) | ((val << f.shift) & f.mask()));
        if self.len == MAX_WRITES { return Err(Failure::JournalFull); }
        self.journal[self.len] = Some((w, old, back));
        self.len += 1;
        self.e.write(w, v);
        self.out.line(format_args!("D WROTE {} = {:08X} (WAS {:08X})", Name(x), v, old));
        Ok(v)
    }
    // A write of DC_I2C_DATA as Display Core's REG_SET: no read first (a read of it moves its index), nothing kept.
    fn buffer(&mut self, v: u32) -> Result<(), Failure> {
        let x = at(R::DcI2cData, 0);
        let w = ddc_writable(x).map_err(|_| Failure::Refused(x.offset()))?;
        self.e.write(w, v);
        self.out.line(format_args!("D WROTE {} = {:08X}", Name(x), v));
        Ok(())
    }

    fn transfer(&mut self) -> Result<Read, Failure> {
        use R::*;
        let (arbitration, hw_status) = (self.read(DcI2cArbitration, 0)?, self.read(DcI2cDdc1HwStatus, 0)?);
        let holder = RW_CNTL_STATUS.get(arbitration);
        if holder == USED_BY_HW || STATUS.get(hw_status) == USED_BY_HW { return Err(Failure::Held { arbitration, hw_status }); }
        // The pads, as hw_gpio's GPIO_MODE_HARDWARE (each pin's mask 0) and hw_ddc's I2C mode (pull-downs enabled,
        // the pad out of AUX mode) leave them.
        self.update(DcGpioDdc1Mask, 0, &[(CLK_MASK, 0), (DATA_MASK, 0), (CLK_PD_EN, 1), (DATA_PD_EN, 1), (AUX_PAD1_MODE, 0)], true)?;
        // setup_engine: out of soft reset, the engine taken.
        self.update(DcI2cControl, 0, &[(SOFT_RESET, 0)], true)?;
        if holder != USED_BY_SW {
            self.update(DcI2cArbitration, 0, &[(SW_USE_I2C_REG_REQ, 1)], false)?;
            let status = RW_CNTL_STATUS.get(self.read(DcI2cArbitration, 0)?);
            self.granted = status == USED_BY_SW;
            if !self.granted { return Err(Failure::NotGranted { status }); }
        }
        self.granted = true;
        // The speed from the microsecond time base, as set_speed computes it.
        let base = self.read(MicrosecondTimeBaseDiv, 0)?;
        let (div, xtal) = (TIME_BASE.get(base), XTAL_REF_DIV.get(base));
        let reference = if div == 0 { CRYSTAL_KHZ } else { div * 1000 };
        let prescale = reference / if xtal == 0 { 2 } else { xtal } / SPEED_KHZ;
        self.update(DcI2cDdc1Speed, 0, &[(PRESCALE, prescale), (THRESHOLD, 2), (START_STOP_TIMING_CNTL, 2)], true)?;
        self.update(DcI2cControl, 0, &[(GO, 0), (SEND_RESET, 0), (SW_STATUS_RESET, 1), (TRANSACTION_COUNT, 0), (DDC_SELECT, 0)], true)?;
        // Display Core writes its reset length 9 into this one-bit field, which keeps 1.
        self.update(DcI2cDdc1Setup, 0, &[(TIME_LIMIT, SETUP_TIME_LIMIT), (SEND_RESET_LENGTH, 1), (ENABLE, 1)], true)?;
        self.update(DcI2cArbitration, 0, &[(NO_QUEUED_SW_GO, 0)], false)?;
        let mut read = Read { bytes: [0; MAX_BLOCKS * BLOCK], blocks: 1 };
        let mut block = 0;
        while block < read.blocks {
            let mut bytes = [0u8; BLOCK];
            self.block(block as u8, &mut bytes)?;
            read.bytes[block * BLOCK..][..BLOCK].copy_from_slice(&bytes);
            if block == 0 {
                if bytes[..8] != [0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0] { return Err(Failure::Header); }
                read.blocks = 1 + (bytes[126] as usize).min(MAX_BLOCKS - 1);
            }
            block += 1;
        }
        Ok(read)
    }

    // One block: process_transaction twice (the offset written without a stop, then 128 bytes read), then
    // execute_transaction, the status polled, the reply read out of the buffer.
    fn block(&mut self, block: u8, bytes: &mut [u8; BLOCK]) -> Result<(), Failure> {
        use R::*;
        let status = self.read(DcI2cSwStatus, 0)?;
        if STATUS.get(status) == USED_BY_SW { return Err(Failure::Busy { block, status }); }
        let (write, read) = (ADDRESS << 1, ADDRESS << 1 | 1);
        self.update(DcI2cTransaction, 0, &[(STOP_ON_NACK, 1), (START, 1), (RW, 0), (COUNT, 1), (STOP, 0)], true)?;
        let first = DATA.mask() & (write << DATA.shift) | INDEX_WRITE;
        self.buffer(first)?;
        self.buffer((first & !INDEX_WRITE & !DATA.mask()) | ((block as u32 * BLOCK as u32) << DATA.shift))?;
        self.update(DcI2cTransaction, 1, &[(STOP_ON_NACK, 1), (START, 1), (RW, 1), (COUNT, BLOCK as u32), (STOP, 1)], true)?;
        self.buffer(read << DATA.shift)?;
        self.update(DcI2cControl, 0, &[(SW_STATUS_RESET, 1)], true)?;
        self.update(DcI2cDdc1Setup, 0, &[(DATA_DRIVE_EN, 0), (CLK_DRIVE_EN, 0), (DATA_DRIVE_SEL, 0), (INTRA_TRANSACTION_DELAY, 0), (INTRA_BYTE_DELAY, 0)], true)?;
        self.update(DcI2cControl, 0, &[(SOFT_RESET, 0), (SW_STATUS_RESET, 0), (SEND_RESET, 0), (GO, 0), (TRANSACTION_COUNT, 1)], true)?;
        // GO: the clock taken right after its write and the status polled at once; the write logged after the transfer,
        // so that no log line lies between GO and the first read of the status (boot 78 measured the poll only).
        let (status, us, polls) = self.go(block)?;
        if status & SW_STOPPED_ON_NACK != 0 { return Err(Failure::Nack { block, status }); }
        if status & SW_TIMEOUT != 0 { return Err(Failure::Timeout { block, status }); }
        if status & SW_ABORTED != 0 { return Err(Failure::Aborted { block, status }); }
        self.out.line(format_args!("D BLOCK {}: FROM GO TO THE TRANSFER'S END {} US, OVER {} READS OF THE STATUS {} US APART; SW STATUS {:08X}{}",
            block, us, polls, POLL_US, status, if status & SW_DONE != 0 { " (DONE)" } else { "" }));
        // The reply after the three bytes written: both addresses and the offset.
        self.buffer((3 << INDEX.shift) | DATA_RW | INDEX_WRITE)?;
        for b in bytes.iter_mut() { *b = DATA.get(self.read(DcI2cData, 0)?) as u8; }
        let sum = bytes.iter().fold(0u8, |s, b| s.wrapping_add(*b));
        if sum != 0 { return Err(Failure::Checksum { block }); }
        self.out.line(format_args!("D BLOCK {}: {} BYTES READ, CHECKSUM GOOD", block, BLOCK));
        Ok(())
    }

    // DC_I2C_GO written and the status polled until the engine no longer has the transfer: (status, us from GO, reads).
    fn go(&mut self, block: u8) -> Result<(u32, u64, u32), Failure> {
        let x = at(R::DcI2cControl, 0);
        let w = ddc_writable(x).map_err(|_| Failure::Refused(x.offset()))?;
        let old = self.read(R::DcI2cControl, 0)?;
        let v = (old & !GO.mask()) | GO.mask();
        if self.len == MAX_WRITES { return Err(Failure::JournalFull); }
        self.journal[self.len] = Some((w, old, true));
        self.len += 1;
        self.e.write(w, v);
        let start = self.e.now_us();
        let mut polls = 0;
        let result = loop {
            let s = self.read(R::DcI2cSwStatus, 0);
            polls += 1;
            match s {
                Ok(s) if STATUS.get(s) != USED_BY_SW => break Ok(s),
                Ok(s) if self.e.now_us().saturating_sub(start) > WAIT_LIMIT_US => break Err(Failure::NoAnswer { block, status: s }),
                Ok(_) => self.e.delay_us(POLL_US),
                Err(why) => break Err(why),
            }
        };
        let us = self.e.now_us().saturating_sub(start);
        self.out.line(format_args!("D WROTE {} = {:08X} (WAS {:08X}), LOGGED AFTER THE TRANSFER", Name(x), v, old));
        result.map(|s| (s, us, polls))
    }

    // release_engine, then every register written put back, last first; then each checked against batch 11.
    fn give_back(&mut self, store: &Store) {
        use R::*;
        if self.len == 0 { return; }
        let reset = match self.read(DcI2cSwStatus, 0) { Ok(s) => STATUS.get(s) == USED_BY_SW, Err(_) => false };
        let fields: &[(Field, u32)] = if reset { &[(SOFT_RESET, 1), (SW_STATUS_RESET, 1)] } else { &[(SW_STATUS_RESET, 1)] };
        let _ = self.update(DcI2cControl, 0, fields, true);
        if self.granted {
            // The engine given back; Display Core's workaround for an engine still held after it, once.
            let _ = self.update(DcI2cArbitration, 0, &[(SW_DONE_USING_I2C_REG, 1)], false);
            if self.read(DcI2cArbitration, 0).map(|a| RW_CNTL_STATUS.get(a)) == Ok(USED_BY_SW) {
                let _ = self.update(DcI2cArbitration, 0, &[(SW_DONE_USING_I2C_REG, 1)], false);
            }
        }
        // The arbitration's own field set back, if changed (NO_QUEUED_SW_GO).
        if let Some(found) = store.get(DcI2cArbitration, 0) {
            if self.read(DcI2cArbitration, 0).map(|a| NO_QUEUED_SW_GO.get(a)) != Ok(NO_QUEUED_SW_GO.get(found)) {
                let _ = self.update(DcI2cArbitration, 0, &[(NO_QUEUED_SW_GO, NO_QUEUED_SW_GO.get(found))], false);
            }
        }
        let mut restored = 0;
        for i in (0..self.len).rev() {
            let Some((w, old, true)) = self.journal[i] else { continue };
            self.e.write(w, old);
            restored += 1;
            self.out.line(format_args!("D RESTORED {} = {:08X}", Name(w.reg()), old));
        }
        let mut same = 0;
        for (r, n) in RESTORED {
            match (self.read(r, n), store.get(r, n)) {
                (Ok(now), Some(found)) if now == found => same += 1,
                (Ok(now), Some(found)) => self.out.line(format_args!("D NOT AS FOUND: {} = {:08X}, BATCH 11 READ {:08X}", Name(at(r, n)), now, found)),
                _ => self.out.line(format_args!("D NOT CHECKED: {}", Name(at(r, n)))),
            }
        }
        let holder = self.read(DcI2cArbitration, 0).map(|a| RW_CNTL_STATUS.get(a)).ok();
        let found = store.field(DcI2cArbitration, 0, RW_CNTL_STATUS);
        out_held(self.out, holder, found);
        self.out.line(format_args!("D THE ENGINE GIVEN BACK: {} WRITES PUT BACK; {} OF {} REGISTERS AS BATCH 11 READ THEM", restored, same, RESTORED.len()));
    }
}

fn out_held(out: &mut dyn Out, now: Option<u32>, found: Option<u32>) {
    let name = |s: Option<u32>| match s { Some(IDLE) => "NOBODY", Some(USED_BY_SW) => "SOFTWARE", Some(USED_BY_HW) => "THE DMCU", Some(_) => "BOTH", None => "-" };
    out.line(format_args!("D THE ENGINE NOW HELD BY {}; BATCH 11 FOUND IT HELD BY {}", name(now), name(found)));
}

/// Step 2: the EDID's blocks, or why not; the engine given back as found either way.
pub fn read<E: Bus + Poke + Clock>(store: &Store, e: &mut E, out: &mut dyn Out) -> Result<Read, Failure> {
    out.line(format_args!("D STEP 2: HDMI-A-1'S EDID OVER DDC1 THROUGH THE I2C ENGINE, AT {} KHZ, IN DISPLAY CORE'S ORDER; THE ENGINE PUT BACK AS FOUND AFTER", SPEED_KHZ));
    let result = if store.get(R::DcI2cArbitration, 0).is_none() || store.get(R::DcI2cDdc1Setup, 0).is_none() { Err(Failure::NotRead) }
        else if store.field(R::HpdIntStatus, 0, DC_HPD_SENSE) != Some(1) { Err(Failure::NoMonitor) }
        else {
            let mut s = Step2 { e, out: &mut *out, journal: [None; MAX_WRITES], len: 0, granted: false };
            let r = s.transfer();
            if let Err(why) = r { s.out.line(format_args!("D STOPPED: {}; THE ENGINE IS GIVEN BACK", why)); }
            s.give_back(store);
            r
        };
    match &result {
        Ok(r) => out.line(format_args!("D STEP 2 DONE: {} BLOCKS READ ({} BYTES)", r.blocks, r.blocks * BLOCK)),
        Err(why) => out.line(format_args!("D STEP 2 NOT DONE: {}; STEPS 3 AND 4 GO ON WITHOUT IT", why)),
    }
    result
}

/// FNV-1a over the bytes: the EDID is named by it in the log, its bytes are not written out.
pub fn hash(bytes: &[u8]) -> u64 { bytes.iter().fold(0xCBF2_9CE4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x0100_0000_01B3)) }

/// What the EDID says, for the mode's choice (213-DRV-0095): the preferred timing, the extensions, whether the sink
/// takes HDMI and to which clock, every timing it gives in full with whether DVI's single link carries it, its standard
/// timings, and the largest timing within DVI's 165 MHz nearer its shape than 1920x1080.
pub fn describe(read: &Read, out: &mut dyn Out) -> Option<Edid> {
    let edid = match Edid::parse(read.bytes()) {
        Ok(e) => e,
        Err(why) => { out.line(format_args!("D THE EDID DOES NOT PARSE: {:?}", why)); return None; }
    };
    let name = core::str::from_utf8(edid.name()).unwrap_or("?");
    out.line(format_args!("D EDID {}.{}, FNV-1A {:016X}: {}{}{} PRODUCT {:04X}, {}, \"{}\"; {} EXTENSIONS ANNOUNCED, {} BLOCKS READ",
        edid.version, edid.revision, hash(read.bytes()), edid.manufacturer[0] as char, edid.manufacturer[1] as char, edid.manufacturer[2] as char,
        edid.product, edid.year, name, edid.extensions, read.blocks));
    let khz = |t: &crate::timing::Timing| t.pixel_khz;
    let hz = |t: &crate::timing::Timing| { let m = t.refresh_millihertz(); (m / 1000, m % 1000) };
    match edid.preferred() {
        Some(t) => out.line(format_args!("D PREFERRED: {}x{} AT {}.{:03} HZ, {} KHZ, IN {}x{}", t.h_active, t.v_active, hz(&t).0, hz(&t).1, khz(&t), t.h_total(), t.v_total())),
        None => out.line(format_args!("D NO PREFERRED TIMING")),
    }
    match edid.hdmi {
        Some(h) => out.line(format_args!("D AN HDMI SINK: TMDS AT MOST {} MHZ (HDMI 1.4 BLOCK {} MHZ, HDMI FORUM BLOCK {} MHZ)", edid.max_tmds_khz() / 1000, h.max_tmds_mhz, h.forum_max_tmds_mhz)),
        None => out.line(format_args!("D NO HDMI BLOCK: A DVI SINK, TMDS AT MOST {} MHZ", edid.max_tmds_khz() / 1000)),
    }
    for (i, t) in edid.timings().enumerate() {
        out.line(format_args!("D TIMING {}: {}x{}{} AT {}.{:03} HZ, {} KHZ; DVI {}", i + 1, t.h_active, t.v_active, if t.interlaced { "I" } else { "" },
            hz(&t).0, hz(&t).1, khz(&t), if t.pixel_khz <= crate::timing::DVI_SINGLE_LINK_KHZ { "CARRIES IT" } else { "DOES NOT" }));
    }
    for s in edid.standards() { out.line(format_args!("D STANDARD TIMING: {}x{} AT {} HZ (ITS CLOCK NOT STATED)", s.width, s.height, s.hz)); }
    out.line(format_args!("D CTA VICS: {:?}", &edid.vics[..edid.vic_count]));
    match edid.nearer_shape_within(crate::timing::DVI_SINGLE_LINK_KHZ, (1920, 1080)) {
        Some(t) => out.line(format_args!("D WITHIN DVI'S {} MHZ AND NEARER ITS SHAPE THAN 1920x1080: {}x{} AT {}.{:03} HZ, {} KHZ",
            crate::timing::DVI_SINGLE_LINK_KHZ / 1000, t.h_active, t.v_active, hz(&t).0, hz(&t).1, khz(&t))),
        None => out.line(format_args!("D WITHIN DVI'S {} MHZ, NO TIMING IT GIVES IN FULL IS NEARER ITS SHAPE THAN 1920x1080", crate::timing::DVI_SINGLE_LINK_KHZ / 1000)),
    }
    out.line(format_args!("D THE MODE STAYS 1920x1080 AT 60 HZ IN THIS BUILD: 213-DRV-0095'S NEXT STEP CHOOSES FROM THIS"));
    Some(edid)
}
