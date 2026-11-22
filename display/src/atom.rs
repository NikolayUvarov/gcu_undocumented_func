//! An interpreter of the ATOM BIOS's command tables, bounded, for the Ryzen's VBIOS (213-DRV-0086): host-only for
//! now, to list what tables 12, 4 and 76 would write for first light. No allocation; every byte it fetches is checked.
//!
//! The bytecode (opcodes, argument kinds, alignments, the table header, the parameter and work spaces, how a call
//! nests) is as Linux v7.0's `drivers/gpu/drm/amd/amdgpu/atom.c` and `atom.h` describe it (AMD and others, MIT); the
//! code is this project's. Where Linux carries on (an unknown opcode ends the table, an index out of range reads
//! 0xCDCDCDCD, an unimplemented opcode is skipped), this one stops with an error that names the place.
//!
//! Bounds: a table's index, offset and length are checked against the master table and the image before it runs;
//! every fetch lies inside the running table and every jump lands inside it; data reads lie inside the image; work,
//! parameter and scratch indices inside their spaces; a register only through `Reg::new` (the engine's bytes and the
//! SMU mailbox, never MSI-X) in direct mode, and a write only where the machine accepts it (step 3's guard); no PCI,
//! system I/O, indirect I/O, PLL or MC access; at most `MAX_STEPS` instructions, `MAX_DEPTH` nested calls, `MAX_DELAY_US`
//! of delays and, when set, a wall-clock limit a run (for the waits the VBIOS leaves unbounded).

use crate::dcn::{Reg, RegError};

/// Instructions a run may take: room for a table's own bounded waits (table 77 counts to 100 000 around 6 instructions),
/// not for an unbounded one.
pub const MAX_STEPS: u32 = 1_000_000;
pub const MAX_DEPTH: usize = 8;
pub const MAX_DELAY_US: u64 = 5_000_000;
/// The parameter space of a run, in dwords: the caller's bytes, then zeros (nested calls take a part of it).
pub const PS_DWORDS: usize = 64;
/// The firmware scratch (`FB` arguments), in dwords: Linux's 20 KiB when the VBIOS states no size, as this one.
pub const SCRATCH_DWORDS: usize = 5120;

const ROM_HEADER_POINTER: usize = 0x48;
const OP_EOT: u8 = 91;
const CASE_MAGIC: u8 = 0x63;
const CASE_END: u16 = 0x5A5A;

// Argument kinds (atom.h ATOM_ARG_*).
const ARG_REG: u8 = 0;
const ARG_PS: u8 = 1;
const ARG_WS: u8 = 2;
const ARG_FB: u8 = 3;
const ARG_ID: u8 = 4;
const ARG_IMM: u8 = 5;
const ARG_PLL: u8 = 6;
const ARG_MC: u8 = 7;
// The six destination kinds of an opcode group, in opcode order.
const DST_KINDS: [u8; 6] = [ARG_REG, ARG_PS, ARG_WS, ARG_FB, ARG_PLL, ARG_MC];
// Alignments: dword, word at 0/8/16, byte at 0/8/16/24.
const MASK: [u32; 8] = [0xFFFF_FFFF, 0xFFFF, 0xFF_FF00, 0xFFFF_0000, 0xFF, 0xFF00, 0xFF_0000, 0xFF00_0000];
const SHIFT: [u32; 8] = [0, 0, 8, 16, 0, 8, 16, 24];
// A destination's alignment from the source alignment field and the destination selector (atom.c's atom_dst_to_src).
const DST_TO_SRC: [[u8; 4]; 8] = [[0, 0, 0, 0], [1, 2, 3, 0], [1, 2, 3, 0], [1, 2, 3, 0], [4, 5, 6, 7], [4, 5, 6, 7], [4, 5, 6, 7], [4, 5, 6, 7]];
const DEF_DST: [u8; 8] = [0, 0, 1, 2, 0, 1, 2, 3];
// Special work-space indices.
const WS_QUOTIENT: u8 = 0x40;
const WS_REMAINDER: u8 = 0x41;
const WS_DATAPTR: u8 = 0x42;
const WS_SHIFT: u8 = 0x43;
const WS_OR_MASK: u8 = 0x44;
const WS_AND_MASK: u8 = 0x45;
const WS_FB_WINDOW: u8 = 0x46;
const WS_ATTRIBUTES: u8 = 0x47;
const WS_REGPTR: u8 = 0x48;

/// Why a run stopped. `table` is the command table's index, `at` the image offset of the instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The image has no ATOM ROM header where its pointer says.
    NoRomHeader,
    /// A read past the image's end.
    OutOfImage { at: usize },
    /// No such command table, or its entry is empty.
    NoSuchTable { index: u8 },
    /// A table whose header or length does not fit the image.
    BadTable { index: u8 },
    /// A fetch or a jump outside the running table.
    OutOfTable { table: u8, at: usize },
    /// An opcode this interpreter does not run (0, 127 and above, REPEAT, SAVEREG, RESTOREREG) or a bad SWITCH case.
    Opcode { table: u8, at: usize, op: u8 },
    Workspace { table: u8, at: usize, index: u8 },
    Parameters { table: u8, at: usize, index: usize },
    Scratch { table: u8, at: usize, index: usize },
    /// A register the window refuses, by dword index as the table names it.
    Register { table: u8, at: usize, dword: u64, write: bool, why: RegError },
    /// Indirect, PCI or system I/O.
    IoMode { table: u8, at: usize, mode: u16 },
    /// A PLL or MC argument.
    Unsupported { table: u8, at: usize, kind: u8 },
    NoDataTable { table: u8, at: usize, index: u8 },
    StepLimit { table: u8, at: usize },
    TooDeep { table: u8 },
    DelayLimit { table: u8, at: usize },
    /// The run took longer than its wall-clock limit.
    TimeLimit { table: u8, at: usize },
    /// The machine could not read a register (not in its mapping).
    Unreadable { table: u8, at: usize, offset: u32 },
    /// The machine refused a write (step 3's guard).
    Refused { table: u8, at: usize, offset: u32 },
}

/// The registers and time a run acts on.
pub trait Machine {
    /// The register's value; None stops the run.
    fn read(&mut self, reg: Reg) -> Option<u32>;
    /// Writes the register; false (a refusal) stops the run before anything else is done.
    fn write(&mut self, reg: Reg, value: u32) -> bool;
    fn delay_us(&mut self, _us: u32) {}
    /// A monotonic clock in microseconds, for the wall-clock limit; one that never moves never stops a run.
    fn now_us(&mut self) -> u64 { 0 }
    /// A table starts, at a nesting depth (0 for the one called).
    fn enter(&mut self, _table: u8, _depth: usize) {}
}

/// A VBIOS image and its master tables.
#[derive(Clone, Copy)]
pub struct Image<'a> { bytes: &'a [u8], commands: usize, data: usize }

/// A command table's place and spaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableInfo { pub base: usize, pub len: usize, pub ws_dwords: usize, pub ps_bytes: usize, pub format: u8, pub content: u8 }

impl<'a> Image<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Image<'a>, Error> {
        let mut image = Image { bytes, commands: 0, data: 0 };
        let rom = image.u16(ROM_HEADER_POINTER)? as usize;
        if image.slice(rom + 4, 4)? != b"ATOM" { return Err(Error::NoRomHeader); }
        image.commands = image.u16(rom + 0x1E)? as usize;
        image.data = image.u16(rom + 0x20)? as usize;
        Ok(image)
    }
    pub fn len(&self) -> usize { self.bytes.len() }
    fn slice(&self, at: usize, n: usize) -> Result<&'a [u8], Error> {
        self.bytes.get(at..at.checked_add(n).ok_or(Error::OutOfImage { at })?).ok_or(Error::OutOfImage { at })
    }
    pub fn u8(&self, at: usize) -> Result<u8, Error> { Ok(self.slice(at, 1)?[0]) }
    pub fn u16(&self, at: usize) -> Result<u16, Error> { let s = self.slice(at, 2)?; Ok(u16::from_le_bytes([s[0], s[1]])) }
    pub fn u32(&self, at: usize) -> Result<u32, Error> { let s = self.slice(at, 4)?; Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]])) }

    /// Entries of a master list: (its size - 4) / 2.
    fn entries(&self, list: usize) -> Result<usize, Error> { Ok((self.u16(list)? as usize).saturating_sub(4) / 2) }

    /// The offset the master command table lists for `index`: None for an empty entry, an error past the list.
    pub fn command_entry(&self, index: u8) -> Result<Option<usize>, Error> {
        if index as usize >= self.entries(self.commands)? { return Err(Error::NoSuchTable { index }); }
        Ok(match self.u16(self.commands + 4 + 2 * index as usize)? { 0 => None, base => Some(base as usize) })
    }

    /// Command table `index`, its header and length checked against the image.
    pub fn command_table(&self, index: u8) -> Result<TableInfo, Error> {
        let base = self.command_entry(index)?.ok_or(Error::NoSuchTable { index })?;
        let bad = Error::BadTable { index };
        let len = self.u16(base).map_err(|_| bad)? as usize;
        if len < 6 || base + len > self.bytes.len() { return Err(bad); }
        Ok(TableInfo {
            base, len, ws_dwords: self.u8(base + 4)? as usize, ps_bytes: (self.u8(base + 5)? & 0x7F) as usize,
            format: self.u8(base + 2)?, content: self.u8(base + 3)?,
        })
    }

    /// Data table `index`'s offset, if the master data table lists one.
    pub fn data_table(&self, index: u8) -> Option<usize> {
        if index as usize >= self.entries(self.data).ok()? { return None; }
        match self.u16(self.data + 4 + 2 * index as usize).ok()? { 0 => None, o => Some(o as usize) }
    }
}

/// The VBIOS image in an ACPI VFCT table (AMD's layout, as Linux's `amdgpu_bios.c` reads it): the table's length at 4,
/// `VBIOSImageOffset` at 52 leading to an image header whose vendor and device are at 12 and 14 and image length at 24,
/// the image after its 28 bytes. Returns (vendor, device, image) for an image that starts with the ROM signature 55AA.
pub fn vfct_image(table: &[u8]) -> Option<(u16, u16, &[u8])> {
    fn u16_in(t: &[u8], at: usize) -> Option<u16> { t.get(at..at.checked_add(2)?).map(|b| u16::from_le_bytes([b[0], b[1]])) }
    fn u32_in(t: &[u8], at: usize) -> Option<usize> { t.get(at..at.checked_add(4)?).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize) }
    if !table.starts_with(b"VFCT") { return None; }
    let table = &table[..u32_in(table, 4)?.min(table.len())];
    let header = u32_in(table, 52)?;
    let (vendor, device) = (u16_in(table, header.checked_add(12)?)?, u16_in(table, header.checked_add(14)?)?);
    let length = u32_in(table, header.checked_add(24)?)?;
    let start = header.checked_add(28)?;
    let image = table.get(start..start.checked_add(length)?)?;
    image.starts_with(&[0x55, 0xAA]).then_some((vendor, device, image))
}

/// What a run did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The parameter space after the run: tables return results in it.
    pub ps: [u32; PS_DWORDS],
    pub steps: u32,
    pub delay_us: u64,
    pub calls: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Io { Mm, Pci, SysIo, Indirect(u16) }

/// The interpreter's state across tables: the firmware scratch persists from run to run, as Linux's does.
pub struct Interpreter<'a> {
    image: Image<'a>,
    scratch: [u32; SCRATCH_DWORDS],
    ps: [u32; PS_DWORDS],
    data_block: u32,
    reg_block: u32,
    fb_base: u32,
    io: Io,
    io_attr: u32,
    divmul: [u32; 2],
    shift: u32,
    equal: bool,
    above: bool,
    steps: u32,
    delay_us: u64,
    calls: u32,
    time_limit_us: u64,
    started_us: u64,
}

/// Instructions between two looks at the clock.
const CLOCK_EVERY: u32 = 64;

/// One running table.
struct Frame { table: u8, info: TableInfo, ws: [u32; 256], ps_base: usize, at: usize }

impl<'a> Interpreter<'a> {
    pub fn new(image: Image<'a>) -> Interpreter<'a> {
        Interpreter { image, scratch: [0; SCRATCH_DWORDS], ps: [0; PS_DWORDS], data_block: 0, reg_block: 0, fb_base: 0,
            io: Io::Mm, io_attr: 0, divmul: [0; 2], shift: 0, equal: false, above: false, steps: 0, delay_us: 0, calls: 0,
            time_limit_us: u64::MAX, started_us: 0 }
    }

    /// The wall-clock limit of each run from now on, in microseconds of the machine's clock.
    pub fn set_time_limit(&mut self, us: u64) { self.time_limit_us = us; }

    /// Runs command table `index` with `params` (little-endian bytes, at most `PS_DWORDS` dwords) on `machine`.
    pub fn run(&mut self, index: u8, params: &[u8], machine: &mut impl Machine) -> Result<Outcome, Error> {
        if params.len() > PS_DWORDS * 4 { return Err(Error::Parameters { table: index, at: 0, index: params.len() / 4 }); }
        self.ps = [0; PS_DWORDS];
        for (i, chunk) in params.chunks(4).enumerate() {
            let mut b = [0u8; 4];
            b[..chunk.len()].copy_from_slice(chunk);
            self.ps[i] = u32::from_le_bytes(b);
        }
        // What Linux resets before each table it is asked to run.
        self.data_block = 0; self.reg_block = 0; self.fb_base = 0; self.io = Io::Mm; self.divmul = [0; 2];
        self.steps = 0; self.delay_us = 0; self.calls = 0;
        self.started_us = machine.now_us();
        self.call(index, 0, 0, machine)?;
        Ok(Outcome { ps: self.ps, steps: self.steps, delay_us: self.delay_us, calls: self.calls })
    }

    fn call(&mut self, index: u8, ps_base: usize, depth: usize, machine: &mut impl Machine) -> Result<(), Error> {
        if depth >= MAX_DEPTH { return Err(Error::TooDeep { table: index }); }
        let info = self.image.command_table(index)?;
        if ps_base > PS_DWORDS { return Err(Error::Parameters { table: index, at: info.base, index: ps_base }); }
        machine.enter(index, depth);
        self.calls += 1;
        let mut f = Frame { table: index, info, ws: [0; 256], ps_base, at: info.base + 6 };
        loop {
            let at = f.at;
            if self.steps >= MAX_STEPS { return Err(Error::StepLimit { table: index, at }); }
            if self.steps % CLOCK_EVERY == 0 && machine.now_us().saturating_sub(self.started_us) > self.time_limit_us {
                return Err(Error::TimeLimit { table: index, at });
            }
            self.steps += 1;
            let op = self.fetch8(&mut f)?;
            if op == OP_EOT { return Ok(()); }
            self.execute(&mut f, at, op, depth, machine)?;
        }
    }

    // --- fetches inside the running table ---

    fn inside(&self, f: &Frame, at: usize, n: usize) -> Result<(), Error> {
        if at < f.info.base + 6 || at + n > f.info.base + f.info.len { return Err(Error::OutOfTable { table: f.table, at }); }
        Ok(())
    }
    fn fetch8(&self, f: &mut Frame) -> Result<u8, Error> { self.inside(f, f.at, 1)?; let v = self.image.u8(f.at)?; f.at += 1; Ok(v) }
    fn fetch16(&self, f: &mut Frame) -> Result<u16, Error> { self.inside(f, f.at, 2)?; let v = self.image.u16(f.at)?; f.at += 2; Ok(v) }
    fn fetch32(&self, f: &mut Frame) -> Result<u32, Error> { self.inside(f, f.at, 4)?; let v = self.image.u32(f.at)?; f.at += 4; Ok(v) }
    fn peek16(&self, f: &Frame) -> Result<u16, Error> { self.inside(f, f.at, 2)?; self.image.u16(f.at) }
    fn jump(&self, f: &mut Frame, target: u16) -> Result<(), Error> {
        let to = f.info.base + target as usize;
        self.inside(f, to, 1)?;
        f.at = to;
        Ok(())
    }

    // --- registers ---

    fn register(&self, f: &Frame, at: usize, dword: u64, write: bool) -> Result<Reg, Error> {
        match self.io {
            Io::Mm => {}
            Io::Pci => return Err(Error::IoMode { table: f.table, at, mode: 0x100 }),
            Io::SysIo => return Err(Error::IoMode { table: f.table, at, mode: 0x200 }),
            Io::Indirect(port) => return Err(Error::IoMode { table: f.table, at, mode: port }),
        }
        let byte = dword.checked_mul(4).filter(|b| *b <= u32::MAX as u64);
        let why = match byte { Some(b) => match Reg::new(b as u32) { Ok(reg) => return Ok(reg), Err(e) => e }, None => RegError::Outside };
        Err(Error::Register { table: f.table, at, dword, write, why })
    }

    // --- arguments ---

    fn immediate(&self, f: &mut Frame, align: u8) -> Result<u32, Error> {
        match align { 0 => self.fetch32(f), 1..=3 => Ok(self.fetch16(f)? as u32), _ => Ok(self.fetch8(f)? as u32) }
    }

    /// A source of kind `attr & 7` and alignment `(attr >> 3) & 7`: the aligned value, and the whole value read.
    fn source(&mut self, f: &mut Frame, at: usize, attr: u8, machine: &mut impl Machine) -> Result<(u32, u32), Error> {
        let (kind, align) = (attr & 7, (attr >> 3) & 7);
        let whole = match kind {
            ARG_REG => {
                let dword = self.fetch16(f)? as u64 + self.reg_block as u64;
                let reg = self.register(f, at, dword, false)?;
                machine.read(reg).ok_or(Error::Unreadable { table: f.table, at, offset: reg.offset() })?
            }
            ARG_PS => {
                let i = f.ps_base + self.fetch8(f)? as usize;
                *self.ps.get(i).ok_or(Error::Parameters { table: f.table, at, index: i })?
            }
            ARG_WS => {
                let i = self.fetch8(f)?;
                match i {
                    WS_QUOTIENT => self.divmul[0], WS_REMAINDER => self.divmul[1], WS_DATAPTR => self.data_block,
                    WS_SHIFT => self.shift, WS_OR_MASK => 1u32.checked_shl(self.shift).unwrap_or(0),
                    WS_AND_MASK => !1u32.checked_shl(self.shift).unwrap_or(0), WS_FB_WINDOW => self.fb_base,
                    WS_ATTRIBUTES => self.io_attr, WS_REGPTR => self.reg_block,
                    _ if (i as usize) < f.info.ws_dwords => f.ws[i as usize],
                    _ => return Err(Error::Workspace { table: f.table, at, index: i }),
                }
            }
            ARG_ID => {
                let offset = self.fetch16(f)? as usize + self.data_block as usize;
                self.image.u32(offset)?
            }
            ARG_FB => {
                let i = (self.fb_base / 4) as usize + self.fetch8(f)? as usize;
                *self.scratch.get(i).ok_or(Error::Scratch { table: f.table, at, index: i })?
            }
            ARG_IMM => { let v = self.immediate(f, align)?; return Ok((v, v)); }
            _ => return Err(Error::Unsupported { table: f.table, at, kind }),
        };
        Ok(((whole & MASK[align as usize]) >> SHIFT[align as usize], whole))
    }

    /// The destination's alignment for `attr`.
    fn dst_align(attr: u8) -> u8 { DST_TO_SRC[((attr >> 3) & 7) as usize][((attr >> 6) & 3) as usize] }

    fn destination(&mut self, f: &mut Frame, at: usize, kind: u8, attr: u8, machine: &mut impl Machine) -> Result<(u32, u32), Error> {
        self.source(f, at, kind | Self::dst_align(attr) << 3, machine)
    }

    fn skip_destination(&self, f: &mut Frame, kind: u8) -> Result<(), Error> {
        match kind { ARG_REG | ARG_ID => { self.fetch16(f)?; } _ => { self.fetch8(f)?; } }
        Ok(())
    }

    /// Stores `value` at the destination whose operand starts at `operand`, merged into `saved`'s other bits.
    fn store(&mut self, f: &mut Frame, at: usize, operand: usize, kind: u8, attr: u8, value: u32, saved: u32, machine: &mut impl Machine) -> Result<(), Error> {
        let align = Self::dst_align(attr) as usize;
        let merged = ((value << SHIFT[align]) & MASK[align]) | (saved & !MASK[align]);
        let resume = f.at;
        f.at = operand;
        match kind {
            ARG_REG => {
                let dword = self.fetch16(f)? as u64 + self.reg_block as u64;
                let reg = self.register(f, at, dword, true)?;
                if !machine.write(reg, merged) { return Err(Error::Refused { table: f.table, at, offset: reg.offset() }); }
            }
            ARG_PS => {
                let i = f.ps_base + self.fetch8(f)? as usize;
                *self.ps.get_mut(i).ok_or(Error::Parameters { table: f.table, at, index: i })? = merged;
            }
            ARG_WS => {
                let i = self.fetch8(f)?;
                match i {
                    WS_QUOTIENT => self.divmul[0] = merged, WS_REMAINDER => self.divmul[1] = merged,
                    WS_DATAPTR => self.data_block = merged, WS_SHIFT => self.shift = merged,
                    WS_OR_MASK | WS_AND_MASK => {}, WS_FB_WINDOW => self.fb_base = merged,
                    WS_ATTRIBUTES => self.io_attr = merged, WS_REGPTR => self.reg_block = merged,
                    _ if (i as usize) < f.info.ws_dwords => f.ws[i as usize] = merged,
                    _ => return Err(Error::Workspace { table: f.table, at, index: i }),
                }
            }
            ARG_FB => {
                let i = (self.fb_base / 4) as usize + self.fetch8(f)? as usize;
                *self.scratch.get_mut(i).ok_or(Error::Scratch { table: f.table, at, index: i })? = merged;
            }
            _ => return Err(Error::Unsupported { table: f.table, at, kind }),
        }
        f.at = resume;
        Ok(())
    }

    // --- instructions ---

    fn execute(&mut self, f: &mut Frame, at: usize, op: u8, depth: usize, m: &mut impl Machine) -> Result<(), Error> {
        let group = |first: u8| DST_KINDS[(op - first) as usize];
        match op {
            1..=6 => {
                let kind = group(1);
                let attr = self.fetch8(f)?;
                let operand = f.at;
                let saved = if (attr >> 3) & 7 != 0 { self.destination(f, at, kind, attr, m)?.1 } else { self.skip_destination(f, kind)?; 0 };
                let (src, _) = self.source(f, at, attr, m)?;
                self.store(f, at, operand, kind, attr, src, saved, m)
            }
            7..=18 | 43..=54 | 103..=108 => {
                let kind = match op { 7..=12 => group(7), 13..=18 => group(13), 43..=48 => group(43), 49..=54 => group(49), _ => group(103) };
                let attr = self.fetch8(f)?;
                let operand = f.at;
                let (dst, saved) = self.destination(f, at, kind, attr, m)?;
                let (src, _) = self.source(f, at, attr, m)?;
                let value = match op { 7..=12 => dst & src, 13..=18 => dst | src, 43..=48 => dst.wrapping_add(src), 49..=54 => dst.wrapping_sub(src), _ => dst ^ src };
                self.store(f, at, operand, kind, attr, value, saved, m)
            }
            19..=30 => {
                let kind = if op < 25 { group(19) } else { group(25) };
                let raw = self.fetch8(f)? & 0x38;
                let attr = raw | DEF_DST[(raw >> 3) as usize] << 6;
                let operand = f.at;
                let (dst, saved) = self.destination(f, at, kind, attr, m)?;
                let by = self.immediate(f, 4)?;
                let value = if op < 25 { dst.checked_shl(by).unwrap_or(0) } else { dst.checked_shr(by).unwrap_or(0) };
                self.store(f, at, operand, kind, attr, value, saved, m)
            }
            31..=42 => {
                let kind = if op < 37 { group(31) } else { group(37) };
                let attr = self.fetch8(f)?;
                let (dst, _) = self.destination(f, at, kind, attr, m)?;
                let (src, _) = self.source(f, at, attr, m)?;
                if op < 37 { self.divmul[0] = dst.wrapping_mul(src); }
                else if src == 0 { self.divmul = [0, 0]; }
                else { self.divmul = [dst / src, dst % src]; }
                Ok(())
            }
            55 => { let port = self.fetch16(f)?; self.io = if port == 0 { Io::Mm } else { Io::Indirect(port) }; Ok(()) }
            56 => { self.fetch8(f)?; self.io = Io::Pci; Ok(()) }
            57 => { self.fetch8(f)?; self.io = Io::SysIo; Ok(()) }
            58 => { self.reg_block = self.fetch16(f)? as u32; Ok(()) }
            59 => { let attr = self.fetch8(f)?; self.fb_base = self.source(f, at, attr, m)?.0; Ok(()) }
            60..=65 | 74..=79 => {
                let kind = if op < 66 { group(60) } else { group(74) };
                let attr = self.fetch8(f)?;
                let (dst, _) = self.destination(f, at, kind, attr, m)?;
                let (src, _) = self.source(f, at, attr, m)?;
                if op < 66 { self.equal = dst == src; self.above = dst > src; } else { self.equal = dst & src == 0; }
                Ok(())
            }
            66 => {
                let attr = self.fetch8(f)?;
                let (src, _) = self.source(f, at, attr, m)?;
                loop {
                    if self.peek16(f)? == CASE_END { f.at += 2; return Ok(()); }
                    if self.fetch8(f)? != CASE_MAGIC { return Err(Error::Opcode { table: f.table, at: f.at - 1, op: 66 }); }
                    let (value, _) = self.source(f, at, (attr & 0x38) | ARG_IMM, m)?;
                    let target = self.fetch16(f)?;
                    if value == src { return self.jump(f, target); }
                }
            }
            67..=73 => {
                let target = self.fetch16(f)?;
                let (e, a) = (self.equal, self.above);
                let taken = match op { 67 => true, 68 => e, 69 => !(a || e), 70 => a, 71 => !a, 72 => a || e, _ => !e };
                if taken { self.jump(f, target)?; }
                Ok(())
            }
            80 | 81 => {
                let n = self.fetch8(f)? as u32;
                let us = if op == 80 { n * 1000 } else { n };
                self.delay_us += us as u64;
                if self.delay_us > MAX_DELAY_US { return Err(Error::DelayLimit { table: f.table, at }); }
                m.delay_us(us);
                Ok(())
            }
            82 => {
                let index = self.fetch8(f)?;
                // An empty entry is skipped, as Linux skips it; one past the list stops the run.
                if self.image.command_entry(index)?.is_none() { return Ok(()); }
                self.call(index, f.ps_base + f.info.ps_bytes / 4, depth + 1, m)
            }
            84..=89 => {
                let kind = group(84);
                let raw = self.fetch8(f)? & 0x38;
                let attr = raw | DEF_DST[(raw >> 3) as usize] << 6;
                let operand = f.at;
                let (_, saved) = self.destination(f, at, kind, attr, m)?;
                self.store(f, at, operand, kind, attr, 0, saved, m)
            }
            90 | 99 => Ok(()),
            92..=97 => {
                let kind = group(92);
                let attr = self.fetch8(f)?;
                let operand = f.at;
                let (dst, saved) = self.destination(f, at, kind, attr, m)?;
                let mask = self.immediate(f, (attr >> 3) & 7)?;
                let (src, _) = self.source(f, at, attr, m)?;
                self.store(f, at, operand, kind, attr, (dst & mask) | src, saved, m)
            }
            98 | 121 => { self.fetch8(f)?; Ok(()) }
            102 => {
                let index = self.fetch8(f)?;
                self.data_block = match index {
                    0 => 0,
                    255 => f.info.base as u32,
                    _ => self.image.data_table(index).ok_or(Error::NoDataTable { table: f.table, at, index })? as u32,
                };
                Ok(())
            }
            109..=120 => {
                let kind = if op < 115 { group(109) } else { group(115) };
                let attr = self.fetch8(f)?;
                let align = Self::dst_align(attr) as usize;
                let operand = f.at;
                let (_, saved) = self.destination(f, at, kind, attr, m)?;
                let (by, _) = self.source(f, at, attr, m)?;
                let whole = if op < 115 { saved.checked_shl(by).unwrap_or(0) } else { saved.checked_shr(by).unwrap_or(0) };
                self.store(f, at, operand, kind, attr, (whole & MASK[align]) >> SHIFT[align], saved, m)
            }
            122 => {
                let skip = self.fetch16(f)? as usize;
                self.inside(f, f.at, skip)?;
                f.at += skip;
                Ok(())
            }
            123..=126 => {
                let kind = if op % 2 == 1 { ARG_PS } else { ARG_WS };
                let attr = self.fetch8(f)?;
                let (dst, _) = self.destination(f, at, kind, attr, m)?;
                let (src, _) = self.source(f, at, attr, m)?;
                if op < 125 {
                    let p = dst as u64 * src as u64;
                    self.divmul = [p as u32, (p >> 32) as u32];
                } else if src == 0 {
                    self.divmul = [0, 0];
                } else {
                    let q = (dst as u64 | (self.divmul[1] as u64) << 32) / src as u64;
                    self.divmul = [q as u32, (q >> 32) as u32];
                }
                Ok(())
            }
            _ => Err(Error::Opcode { table: f.table, at, op }),
        }
    }
}
