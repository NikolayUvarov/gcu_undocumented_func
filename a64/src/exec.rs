//! One A64 instruction at a time (215-PRT-0016): the base integer set an EL0 program of the aarch64 build uses.
//! Anything else is an undefined instruction, reported before any register or memory changes.
use crate::{Access, Bus, Cpu, Exit, Fault, FaultKind};

pub(crate) enum Stop { Exit(Exit) }
type Step = Result<(), Stop>;

impl From<Fault> for Stop { fn from(fault: Fault) -> Self { Stop::Exit(Exit::Fault(fault)) } }
fn undefined(word: u32) -> Stop { Stop::Exit(Exit::Undefined(word)) }

const N: u64 = 1 << 31;
const Z: u64 = 1 << 30;
const C: u64 = 1 << 29;
const V: u64 = 1 << 28;

#[inline(always)]
fn bits(w: u32, hi: u32, lo: u32) -> u32 { (w >> lo) & (u32::MAX >> (31 - (hi - lo))) }
#[inline(always)]
fn bit(w: u32, b: u32) -> bool { (w >> b) & 1 != 0 }
#[inline(always)]
fn sext(v: u64, width: u32) -> u64 { let s = 64 - width; (((v << s) as i64) >> s) as u64 }
#[inline(always)]
fn mask(sf: bool, v: u64) -> u64 { if sf { v } else { v & 0xFFFF_FFFF } }

impl Cpu {
    /// Register n, 31 reading as zero.
    #[inline(always)]
    fn r(&self, n: u32) -> u64 { if n == 31 { 0 } else { self.x[n as usize] } }
    /// Register n, 31 being SP.
    #[inline(always)]
    fn rsp(&self, n: u32) -> u64 { if n == 31 { self.sp } else { self.x[n as usize] } }
    #[inline(always)]
    fn set(&mut self, n: u32, v: u64) { if n != 31 { self.x[n as usize] = v; } }
    #[inline(always)]
    fn setsp(&mut self, n: u32, v: u64) { if n == 31 { self.sp = v; } else { self.x[n as usize] = v; } }
}

/// a + b + carry with the flags it sets; the result is cut to 32 bits when `sf` is clear.
fn add_with_carry(a: u64, b: u64, carry: bool, sf: bool) -> (u64, u64) {
    let (r, c, v, negative) = if sf {
        let (r1, c1) = a.overflowing_add(b);
        let (r, c2) = r1.overflowing_add(carry as u64);
        (r, c1 | c2, (!(a ^ b) & (a ^ r)) >> 63 != 0, (r as i64) < 0)
    } else {
        let (a, b) = (a as u32, b as u32);
        let (r1, c1) = a.overflowing_add(b);
        let (r, c2) = r1.overflowing_add(carry as u32);
        (r as u64, c1 | c2, (!(a ^ b) & (a ^ r)) >> 31 != 0, (r as i32) < 0)
    };
    let flags = (if negative { N } else { 0 }) | (if r == 0 { Z } else { 0 }) | (if c { C } else { 0 }) | (if v { V } else { 0 });
    (r, flags)
}

fn add_or_sub(a: u64, b: u64, sub: bool, sf: bool) -> (u64, u64) {
    if sub { add_with_carry(a, !b, true, sf) } else { add_with_carry(a, b, false, sf) }
}

/// N and Z of a logical result; C and V cleared.
fn nz(r: u64, sf: bool) -> u64 {
    let negative = if sf { (r as i64) < 0 } else { (r as u32 as i32) < 0 };
    (if negative { N } else { 0 }) | (if mask(sf, r) == 0 { Z } else { 0 })
}

fn condition(nzcv: u64, cond: u32) -> bool {
    let (n, z, c, v) = (nzcv & N != 0, nzcv & Z != 0, nzcv & C != 0, nzcv & V != 0);
    let result = match cond >> 1 {
        0 => z,
        1 => c,
        2 => n,
        3 => v,
        4 => c && !z,
        5 => n == v,
        6 => n == v && !z,
        _ => true,
    };
    if cond & 1 == 1 && cond != 0b1111 { !result } else { result }
}

fn shift(v: u64, kind: u32, amount: u32, sf: bool) -> u64 {
    if sf {
        match kind { 0 => v << amount, 1 => v >> amount, 2 => ((v as i64) >> amount) as u64, _ => v.rotate_right(amount) }
    } else {
        let v = v as u32;
        (match kind { 0 => v << amount, 1 => v >> amount, 2 => ((v as i32) >> amount) as u32, _ => v.rotate_right(amount) }) as u64
    }
}

fn extend(v: u64, option: u32, amount: u32) -> u64 {
    let e = match option {
        0 => v & 0xFF,
        1 => v & 0xFFFF,
        2 => v & 0xFFFF_FFFF,
        3 => v,
        4 => v as u8 as i8 as i64 as u64,
        5 => v as u16 as i16 as i64 as u64,
        6 => v as u32 as i32 as i64 as u64,
        _ => v,
    };
    e << amount
}

fn ones(n: u32) -> u64 { if n >= 64 { u64::MAX } else { (1u64 << n) - 1 } }

fn rotate(v: u64, r: u32, size: u32) -> u64 {
    if size == 64 { v.rotate_right(r) } else { (v as u32).rotate_right(r) as u64 }
}

/// The Arm ARM's DecodeBitMasks: the logical immediate (wmask) and the bitfield's mask of the top bits (tmask).
fn bit_masks(n: u32, imms: u32, immr: u32, immediate: bool) -> Option<(u64, u64)> {
    let combined = (n << 6) | (!imms & 0x3F);
    if combined == 0 { return None; }
    let len = 31 - combined.leading_zeros();
    if len < 1 { return None; }
    let esize = 1u32 << len;
    let levels = esize - 1;
    let (s, r) = (imms & levels, immr & levels);
    if immediate && s == levels { return None; }
    let d = s.wrapping_sub(r) & levels;
    let welem = ones(s + 1);
    let telem = ones(d + 1);
    let element = if r == 0 { welem } else { ((welem >> r) | (welem << (esize - r))) & ones(esize) };
    let (mut wmask, mut tmask, mut i) = (0u64, 0u64, 0u32);
    while i < 64 {
        wmask |= element << i;
        tmask |= telem << i;
        i += esize;
    }
    Some((wmask, tmask))
}

pub(crate) fn step<B: Bus>(cpu: &mut Cpu, bus: &mut B, w: u32) -> Step {
    let pc = cpu.pc;
    let mut next = pc.wrapping_add(4);
    match (w >> 25) & 0xF {
        0b1000 | 0b1001 => immediate(cpu, w, pc)?,
        0b1010 | 0b1011 => if let Some(target) = branch_or_system(cpu, bus, w, pc)? { next = target; },
        0b0100 | 0b0110 | 0b1100 | 0b1110 => load_store(cpu, bus, w, pc)?,
        0b0101 | 0b1101 => register(cpu, w)?,
        _ => return Err(undefined(w)), // SVE, SME, FP and SIMD: none in the build
    }
    cpu.pc = next;
    Ok(())
}

/// Data processing with an immediate: PC-relative addresses, add and subtract, logical, move wide, bitfield, extract.
fn immediate(cpu: &mut Cpu, w: u32, pc: u64) -> Step {
    let (sf, rd, rn) = (bit(w, 31), w & 31, (w >> 5) & 31);
    match bits(w, 25, 23) {
        0b000 | 0b001 => {
            let imm = sext(((bits(w, 23, 5) as u64) << 2) | bits(w, 30, 29) as u64, 21);
            let v = if bit(w, 31) { (pc & !0xFFF).wrapping_add(imm << 12) } else { pc.wrapping_add(imm) };
            cpu.set(rd, v);
        }
        0b010 => {
            let imm = (bits(w, 21, 10) as u64) << if bit(w, 22) { 12 } else { 0 };
            let (r, flags) = add_or_sub(cpu.rsp(rn), imm, bit(w, 30), sf);
            if bit(w, 29) { cpu.nzcv = flags; cpu.set(rd, r); } else { cpu.setsp(rd, r); }
        }
        0b100 => {
            let n = bit(w, 22) as u32;
            if !sf && n == 1 { return Err(undefined(w)); }
            let (imm, _) = bit_masks(n, bits(w, 15, 10), bits(w, 21, 16), true).ok_or(undefined(w))?;
            let (a, imm, opc) = (cpu.r(rn), mask(sf, imm), bits(w, 30, 29));
            let r = mask(sf, match opc { 0 | 3 => a & imm, 1 => a | imm, _ => a ^ imm });
            if opc == 3 { cpu.nzcv = nz(r, sf); cpu.set(rd, r); } else { cpu.setsp(rd, r); }
        }
        0b101 => {
            let hw = bits(w, 22, 21);
            if !sf && hw >= 2 { return Err(undefined(w)); }
            let imm = (bits(w, 20, 5) as u64) << (hw * 16);
            match bits(w, 30, 29) {
                0 => cpu.set(rd, mask(sf, !imm)),
                2 => cpu.set(rd, imm),
                3 => { let old = cpu.r(rd); cpu.set(rd, mask(sf, (old & !(0xFFFFu64 << (hw * 16))) | imm)); }
                _ => return Err(undefined(w)),
            }
        }
        0b110 => {
            let (n, opc, immr, imms) = (bit(w, 22), bits(w, 30, 29), bits(w, 21, 16), bits(w, 15, 10));
            if n != sf || opc == 3 || (!sf && (immr >= 32 || imms >= 32)) { return Err(undefined(w)); }
            let (wmask, tmask) = bit_masks(n as u32, imms, immr, false).ok_or(undefined(w))?;
            let size = if sf { 64 } else { 32 };
            let src = cpu.r(rn);
            let rotated = rotate(src, immr, size);
            let r = match opc {
                0 => { let top = if (src >> imms) & 1 == 1 { u64::MAX } else { 0 }; (top & !tmask) | (rotated & wmask & tmask) }
                1 => { let dst = cpu.r(rd); let bottom = (dst & !wmask) | (rotated & wmask); (dst & !tmask) | (bottom & tmask) }
                _ => rotated & wmask & tmask,
            };
            cpu.set(rd, mask(sf, r));
        }
        0b111 => {
            let lsb = bits(w, 15, 10);
            if bit(w, 22) != sf || bits(w, 30, 29) != 0 || bit(w, 21) || (!sf && lsb >= 32) { return Err(undefined(w)); }
            let (high, low) = (cpu.r(rn), cpu.r((w >> 16) & 31));
            let r = if sf {
                if lsb == 0 { low } else { (low >> lsb) | (high << (64 - lsb)) }
            } else {
                ((((high as u32 as u64) << 32) | low as u32 as u64) >> lsb) as u32 as u64
            };
            cpu.set(rd, r);
        }
        _ => return Err(undefined(w)), // add and subtract with tags (MTE)
    }
    Ok(())
}

/// Branches, exceptions and system instructions. Returns the target of a branch taken.
fn branch_or_system<B: Bus>(cpu: &mut Cpu, bus: &mut B, w: u32, pc: u64) -> Result<Option<u64>, Stop> {
    let offset = |field: u32, width: u32| pc.wrapping_add(sext(field as u64, width) << 2);
    if w & 0x7C00_0000 == 0x1400_0000 {
        if bit(w, 31) { cpu.x[30] = pc.wrapping_add(4); }
        return Ok(Some(offset(bits(w, 25, 0), 26)));
    }
    if w & 0x7E00_0000 == 0x3400_0000 {
        let zero = mask(bit(w, 31), cpu.r(w & 31)) == 0;
        return Ok((zero != bit(w, 24)).then(|| offset(bits(w, 23, 5), 19)));
    }
    if w & 0x7E00_0000 == 0x3600_0000 {
        let b = (bits(w, 31, 31) << 5) | bits(w, 23, 19);
        let set = (cpu.r(w & 31) >> b) & 1 == 1;
        return Ok((set == bit(w, 24)).then(|| offset(bits(w, 18, 5), 14)));
    }
    if w & 0xFF00_0010 == 0x5400_0000 {
        return Ok(condition(cpu.nzcv, w & 15).then(|| offset(bits(w, 23, 5), 19)));
    }
    if w & 0xFF00_0000 == 0xD400_0000 {
        let imm = bits(w, 20, 5) as u16;
        return match (bits(w, 23, 21), bits(w, 4, 0)) {
            (0, 1) => { cpu.pc = pc.wrapping_add(4); Err(Stop::Exit(Exit::Svc(imm))) }
            (1, 0) => Err(Stop::Exit(Exit::Brk(imm))),
            _ => Err(undefined(w)),
        };
    }
    if w & 0xFFC0_0000 == 0xD500_0000 {
        system(cpu, bus, w)?;
        return Ok(None);
    }
    if w & 0xFE00_0000 == 0xD600_0000 {
        let target = cpu.r((w >> 5) & 31);
        return match w & 0xFFFF_FC1F {
            0xD61F_0000 | 0xD65F_0000 => Ok(Some(target)),
            0xD63F_0000 => { cpu.x[30] = pc.wrapping_add(4); Ok(Some(target)) }
            _ => Err(undefined(w)),
        };
    }
    Err(undefined(w))
}

/// Hints, barriers and the system registers a program may use; the rest are the kernel's and undefined here.
fn system<B: Bus>(cpu: &mut Cpu, bus: &mut B, w: u32) -> Step {
    if w & 0xFFFF_F01F == 0xD503_201F {
        // WFE and WFI complete, as when an event wakes them: the task resumes past them.
        return match bits(w, 11, 5) { 2 | 3 => { cpu.pc = cpu.pc.wrapping_add(4); Err(Stop::Exit(Exit::Wait)) } _ => Ok(()) };
    }
    if w & 0xFFFF_F01F == 0xD503_301F {
        if bits(w, 7, 5) == 2 { cpu.monitor = None; } // CLREX; DSB, DMB and ISB order nothing on one CPU
        return Ok(());
    }
    if w & 0xFFD0_0000 == 0xD510_0000 {
        let register = (2 + bits(w, 19, 19), bits(w, 18, 16), bits(w, 15, 12), bits(w, 11, 8), bits(w, 7, 5));
        let rt = w & 31;
        if bit(w, 21) {
            let v = match register {
                (3, 3, 4, 2, 0) => cpu.nzcv,
                (3, 3, 4, 4, 0) => cpu.fpcr,
                (3, 3, 4, 4, 1) => cpu.fpsr,
                (3, 3, 13, 0, 2) => cpu.tpidr,
                (3, 3, 13, 0, 3) => cpu.tpidrro,
                (3, 3, 14, 0, 0) => bus.frequency(),
                (3, 3, 14, 0, 1) | (3, 3, 14, 0, 2) => bus.counter(),
                (3, 3, 2, 4, 0) | (3, 3, 2, 4, 1) => match bus.random() {
                    Some(v) => { cpu.nzcv = 0; v }
                    None => { cpu.nzcv = Z; 0 }
                },
                (3, 3, 0, 0, 7) => 1 << 4,       // DCZID_EL0: DC ZVA prohibited, so nothing uses it
                (3, 3, 0, 0, 1) => 0x8444_C004,  // CTR_EL0: 64-byte lines
                _ => return Err(undefined(w)),
            };
            cpu.set(rt, v);
        } else {
            let v = cpu.r(rt);
            match register {
                (3, 3, 4, 2, 0) => cpu.nzcv = v & 0xF000_0000,
                (3, 3, 4, 4, 0) => cpu.fpcr = v,
                (3, 3, 4, 4, 1) => cpu.fpsr = v,
                (3, 3, 13, 0, 2) => cpu.tpidr = v,
                _ => return Err(undefined(w)),
            }
        }
        return Ok(());
    }
    Err(undefined(w))
}

fn load_store<B: Bus>(cpu: &mut Cpu, bus: &mut B, w: u32, pc: u64) -> Step {
    if bit(w, 26) { return Err(undefined(w)); } // SIMD and FP registers
    if w & 0x3F00_0000 == 0x0800_0000 { return exclusive(cpu, bus, w); }
    if w & 0x3B00_0000 == 0x1800_0000 {
        let address = pc.wrapping_add(sext(bits(w, 23, 5) as u64, 19) << 2);
        let rt = w & 31;
        match bits(w, 31, 30) {
            0 => { let v = bus.read(address, 4, Access::Read)?; cpu.set(rt, v); }
            1 => { let v = bus.read(address, 8, Access::Read)?; cpu.set(rt, v); }
            2 => { let v = bus.read(address, 4, Access::Read)?; cpu.set(rt, sext(v, 32)); }
            _ => {} // PRFM
        }
        return Ok(());
    }
    if w & 0x3A00_0000 == 0x2800_0000 { return pair(cpu, bus, w); }
    if w & 0x3B00_0000 == 0x3800_0000 || w & 0x3B00_0000 == 0x3900_0000 { return single(cpu, bus, w); }
    Err(undefined(w))
}

/// One register: an unsigned or signed offset, pre- or post-indexed, or a register offset.
fn single<B: Bus>(cpu: &mut Cpu, bus: &mut B, w: u32) -> Step {
    let (size, opc, rt, rn) = (bits(w, 31, 30), bits(w, 23, 22), w & 31, (w >> 5) & 31);
    let base = cpu.rsp(rn);
    let (address, writeback) = if bit(w, 24) {
        (base.wrapping_add((bits(w, 21, 10) as u64) << size), None)
    } else if bit(w, 21) {
        let option = bits(w, 15, 13);
        if bits(w, 11, 10) != 2 || option & 2 == 0 { return Err(undefined(w)); } // LSE atomics: none in the build
        let amount = if bit(w, 12) { size } else { 0 };
        (base.wrapping_add(extend(cpu.r((w >> 16) & 31), option, amount)), None)
    } else {
        let imm = sext(bits(w, 20, 12) as u64, 9);
        match bits(w, 11, 10) {
            1 => (base, Some(base.wrapping_add(imm))),
            3 => { let a = base.wrapping_add(imm); (a, Some(a)) }
            _ => (base.wrapping_add(imm), None), // unscaled, and unprivileged, which is the same at EL0
        }
    };
    let bytes = 1u32 << size;
    match opc {
        0 => bus.write(address, bytes, cpu.r(rt))?,
        1 => { let v = bus.read(address, bytes, Access::Read)?; cpu.set(rt, v); }
        2 if size == 3 => {} // PRFM
        2 => { let v = bus.read(address, bytes, Access::Read)?; cpu.set(rt, sext(v, bytes * 8)); }
        _ => {
            if size >= 2 { return Err(undefined(w)); }
            let v = bus.read(address, bytes, Access::Read)?;
            cpu.set(rt, sext(v, bytes * 8) & 0xFFFF_FFFF);
        }
    }
    if let Some(v) = writeback { cpu.setsp(rn, v); }
    Ok(())
}

/// Two registers: STP, LDP, LDPSW, STNP and LDNP, by offset, pre- or post-indexed.
fn pair<B: Bus>(cpu: &mut Cpu, bus: &mut B, w: u32) -> Step {
    let (opc, load) = (bits(w, 31, 30), bit(w, 22));
    let (rt, rt2, rn) = (w & 31, (w >> 10) & 31, (w >> 5) & 31);
    if opc == 3 || (opc == 1 && !load) { return Err(undefined(w)); } // STGP (MTE) and the reserved form
    let scale = if opc == 2 { 3 } else { 2 };
    let bytes = 1u32 << scale;
    let offset = sext(bits(w, 21, 15) as u64, 7) << scale;
    let base = cpu.rsp(rn);
    let (address, writeback) = match bits(w, 24, 23) {
        1 => (base, Some(base.wrapping_add(offset))),
        3 => { let a = base.wrapping_add(offset); (a, Some(a)) }
        _ => (base.wrapping_add(offset), None),
    };
    let second = address.wrapping_add(bytes as u64);
    if load {
        let (mut a, mut b) = (bus.read(address, bytes, Access::Read)?, bus.read(second, bytes, Access::Read)?);
        if opc == 1 { a = sext(a, 32); b = sext(b, 32); }
        cpu.set(rt, a);
        cpu.set(rt2, b);
    } else {
        let (a, b) = (cpu.r(rt), cpu.r(rt2));
        bus.write(address, bytes, a)?;
        bus.write(second, bytes, b)?;
    }
    if let Some(v) = writeback { cpu.setsp(rn, v); }
    Ok(())
}

/// The exclusives (LDXR, STXR and their acquire, release and pair forms) and the ordered LDAR and STLR.
fn exclusive<B: Bus>(cpu: &mut Cpu, bus: &mut B, w: u32) -> Step {
    let (size, ordered, load, pairs) = (bits(w, 31, 30), bit(w, 23), bit(w, 22), bit(w, 21));
    let (rs, rt2, rn, rt) = ((w >> 16) & 31, (w >> 10) & 31, (w >> 5) & 31, w & 31);
    let address = cpu.rsp(rn);
    let bytes = 1u32 << size;
    if ordered {
        if pairs { return Err(undefined(w)); } // CAS (LSE): none in the build
        if load {
            let v = bus.read(address, bytes, Access::Read)?;
            cpu.set(rt, v);
        } else {
            bus.write(address, bytes, cpu.r(rt))?;
        }
        return Ok(());
    }
    if pairs && size < 2 { return Err(undefined(w)); }
    let total = if pairs { bytes * 2 } else { bytes };
    if address % total as u64 != 0 {
        let access = if load { Access::Read } else { Access::Write };
        return Err(Stop::Exit(Exit::Fault(Fault { address, access, kind: FaultKind::Alignment })));
    }
    if load {
        let a = bus.read(address, bytes, Access::Read)?;
        let b = if pairs { Some(bus.read(address.wrapping_add(bytes as u64), bytes, Access::Read)?) } else { None };
        cpu.set(rt, a);
        if let Some(b) = b { cpu.set(rt2, b); }
        cpu.monitor = Some((address, total));
    } else {
        let held = cpu.monitor == Some((address, total));
        if held {
            bus.write(address, bytes, cpu.r(rt))?;
            if pairs { bus.write(address.wrapping_add(bytes as u64), bytes, cpu.r(rt2))?; }
        }
        cpu.monitor = None;
        cpu.set(rs, if held { 0 } else { 1 });
    }
    Ok(())
}

/// Data processing on registers.
fn register(cpu: &mut Cpu, w: u32) -> Step {
    let (sf, rd, rn, rm) = (bit(w, 31), w & 31, (w >> 5) & 31, (w >> 16) & 31);
    if w & 0x1F00_0000 == 0x0A00_0000 {
        let amount = bits(w, 15, 10);
        if !sf && amount >= 32 { return Err(undefined(w)); }
        let mut b = shift(cpu.r(rm), bits(w, 23, 22), amount, sf);
        if bit(w, 21) { b = !b; }
        let (a, opc) = (cpu.r(rn), bits(w, 30, 29));
        let r = mask(sf, match opc { 0 | 3 => a & b, 1 => a | b, _ => a ^ b });
        if opc == 3 { cpu.nzcv = nz(r, sf); }
        cpu.set(rd, r);
        return Ok(());
    }
    if w & 0x1F20_0000 == 0x0B00_0000 {
        let (kind, amount) = (bits(w, 23, 22), bits(w, 15, 10));
        if kind == 3 || (!sf && amount >= 32) { return Err(undefined(w)); }
        let (r, flags) = add_or_sub(cpu.r(rn), shift(cpu.r(rm), kind, amount, sf), bit(w, 30), sf);
        if bit(w, 29) { cpu.nzcv = flags; }
        cpu.set(rd, r);
        return Ok(());
    }
    if w & 0x1F20_0000 == 0x0B20_0000 {
        let amount = bits(w, 12, 10);
        if amount > 4 || bits(w, 23, 22) != 0 { return Err(undefined(w)); }
        let (r, flags) = add_or_sub(cpu.rsp(rn), extend(cpu.r(rm), bits(w, 15, 13), amount), bit(w, 30), sf);
        if bit(w, 29) { cpu.nzcv = flags; cpu.set(rd, r); } else { cpu.setsp(rd, r); }
        return Ok(());
    }
    if w & 0x1FE0_FC00 == 0x1A00_0000 {
        let (a, b, carry) = (cpu.r(rn), cpu.r(rm), cpu.nzcv & C != 0);
        let (r, flags) = if bit(w, 30) { add_with_carry(a, !b, carry, sf) } else { add_with_carry(a, b, carry, sf) };
        if bit(w, 29) { cpu.nzcv = flags; }
        cpu.set(rd, r);
        return Ok(());
    }
    if w & 0x1FE0_0410 == 0x1A40_0000 {
        if !bit(w, 29) { return Err(undefined(w)); }
        let a = cpu.r(rn);
        let b = if bit(w, 11) { bits(w, 20, 16) as u64 } else { cpu.r(rm) };
        cpu.nzcv = if condition(cpu.nzcv, bits(w, 15, 12)) { add_or_sub(a, b, bit(w, 30), sf).1 } else { (bits(w, 3, 0) as u64) << 28 };
        return Ok(());
    }
    if w & 0x1FE0_0800 == 0x1A80_0000 {
        if bit(w, 29) { return Err(undefined(w)); }
        let r = if condition(cpu.nzcv, bits(w, 15, 12)) {
            cpu.r(rn)
        } else {
            let b = cpu.r(rm);
            match (bit(w, 30), bit(w, 10)) { (false, false) => b, (false, true) => b.wrapping_add(1), (true, false) => !b, _ => b.wrapping_neg() }
        };
        cpu.set(rd, mask(sf, r));
        return Ok(());
    }
    if w & 0x5FE0_0000 == 0x1AC0_0000 {
        if bit(w, 29) { return Err(undefined(w)); }
        let (a, b) = (mask(sf, cpu.r(rn)), mask(sf, cpu.r(rm)));
        let width = if sf { 64 } else { 32 };
        let r = match bits(w, 15, 10) {
            2 => if b == 0 { 0 } else { a / b },
            3 => if b == 0 {
                0
            } else if sf {
                (a as i64).wrapping_div(b as i64) as u64
            } else {
                (a as u32 as i32).wrapping_div(b as u32 as i32) as u32 as u64
            },
            k @ 8..=11 => shift(a, k - 8, (b % width) as u32, sf),
            _ => return Err(undefined(w)), // CRC32 and the PAC forms: none in the build
        };
        cpu.set(rd, mask(sf, r));
        return Ok(());
    }
    if w & 0x5FFF_0000 == 0x5AC0_0000 {
        if bit(w, 29) { return Err(undefined(w)); }
        let a = cpu.r(rn);
        let r = match (bits(w, 15, 10), sf) {
            (0, true) => a.reverse_bits(),
            (0, false) => (a as u32).reverse_bits() as u64,
            (1, _) => mask(sf, ((a & 0x00FF_00FF_00FF_00FF) << 8) | ((a >> 8) & 0x00FF_00FF_00FF_00FF)),
            (2, true) => (((a >> 32) as u32).swap_bytes() as u64) << 32 | (a as u32).swap_bytes() as u64,
            (2, false) => (a as u32).swap_bytes() as u64,
            (3, true) => a.swap_bytes(),
            (4, true) => a.leading_zeros() as u64,
            (4, false) => (a as u32).leading_zeros() as u64,
            (5, true) => (if (a as i64) < 0 { !a } else { a }).leading_zeros() as u64 - 1,
            (5, false) => { let a = a as u32; (if (a as i32) < 0 { !a } else { a }).leading_zeros() as u64 - 1 }
            _ => return Err(undefined(w)),
        };
        cpu.set(rd, r);
        return Ok(());
    }
    if w & 0x1F00_0000 == 0x1B00_0000 {
        if bits(w, 30, 29) != 0 { return Err(undefined(w)); }
        let (a, b, c) = (cpu.r(rn), cpu.r(rm), cpu.r((w >> 10) & 31));
        let r = match (bits(w, 23, 21), bit(w, 15)) {
            (0, false) => c.wrapping_add(a.wrapping_mul(b)),
            (0, true) => c.wrapping_sub(a.wrapping_mul(b)),
            (1, minus) if sf => {
                let p = (a as i32 as i64).wrapping_mul(b as i32 as i64) as u64;
                if minus { c.wrapping_sub(p) } else { c.wrapping_add(p) }
            }
            (2, false) if sf => ((a as i64 as i128 * b as i64 as i128) >> 64) as u64,
            (5, minus) if sf => {
                let p = (a as u32 as u64).wrapping_mul(b as u32 as u64);
                if minus { c.wrapping_sub(p) } else { c.wrapping_add(p) }
            }
            (6, false) if sf => ((a as u128 * b as u128) >> 64) as u64,
            _ => return Err(undefined(w)),
        };
        cpu.set(rd, mask(sf, r));
        return Ok(());
    }
    Err(undefined(w))
}
