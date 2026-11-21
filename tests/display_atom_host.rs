// Host cases of display's ATOM interpreter (213-DRV-0086): its opcodes and every bound on synthetic images built here,
// then the Ryzen's VBIOS tables 12, 4 and 76 with first light's parameters against a register model, listing what each
// would write. The VBIOS image is the vendor's and stays out of the repository: that case runs only when
// MIND_AMD_VBIOS names the file, and says so when it skips.
#![allow(dead_code)] // the driver uses parts these cases do not
#[path = "../display/src/timing.rs"] mod timing;
#[path = "../display/src/edid.rs"] mod edid;
#[path = "../display/src/dcn.rs"] mod dcn;
#[path = "../display/src/atom.rs"] mod atom;

use atom::{Error, Image, Interpreter, Machine, MAX_DEPTH, MAX_STEPS};
use dcn::{Reg, RegError, R};
use std::collections::HashMap;

// --- a register model ---

#[derive(Clone, Debug, PartialEq)]
enum Access { Read(u32, u32), Write(u32, u32, u32), Delay(u32), Enter(u8, usize) }

// Registers as memory, plus `answers`: bits that read as set whatever was written, where the hardware sets them (a
// status a table polls). Each answer is an assumption about the hardware, named where it is added.
#[derive(Clone, Copy, Debug)]
enum Answer {
    /// These bits read as set.
    Set(u32, u32),
    /// The bits of `ack` read as the bits of `ack >> 1` hold: an acknowledge that follows its request.
    AckFollowsReq(u32, u32),
}

#[derive(Default)]
struct Model { regs: HashMap<u32, u32>, log: Vec<Access>, answers: Vec<Answer> }
impl Machine for Model {
    fn read(&mut self, reg: Reg) -> u32 {
        let mut v = *self.regs.get(&reg.offset()).unwrap_or(&0);
        for a in &self.answers {
            match *a {
                Answer::Set(o, bits) if o == reg.offset() => v |= bits,
                Answer::AckFollowsReq(o, ack) if o == reg.offset() => v = (v & !ack) | ((v & (ack >> 1)) << 1),
                _ => {}
            }
        }
        self.log.push(Access::Read(reg.offset(), v));
        v
    }
    fn write(&mut self, reg: Reg, value: u32) {
        let old = self.regs.insert(reg.offset(), value).unwrap_or(0);
        self.log.push(Access::Write(reg.offset(), value, old));
    }
    fn delay_us(&mut self, us: u32) { self.log.push(Access::Delay(us)); }
    fn enter(&mut self, table: u8, depth: usize) { self.log.push(Access::Enter(table, depth)); }
}

// --- synthetic images ---

// A table: workspace dwords, parameter bytes, code (jump targets are offsets from the table's start, header included).
struct T { ws: u8, ps: u8, code: Vec<u8> }

fn image(tables: &[(u8, T)], data: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut b = vec![0u8; 0x400];
    b[0] = 0x55; b[1] = 0xAA;
    b[0x48..0x4A].copy_from_slice(&0x100u16.to_le_bytes());
    b[0x104..0x108].copy_from_slice(b"ATOM");
    b[0x11E..0x120].copy_from_slice(&0x200u16.to_le_bytes());
    b[0x120..0x122].copy_from_slice(&0x300u16.to_le_bytes());
    b[0x200..0x202].copy_from_slice(&(4u16 + 2 * 81).to_le_bytes()); // 81 command entries
    b[0x300..0x302].copy_from_slice(&(4u16 + 2 * 34).to_le_bytes()); // 34 data entries
    for (index, t) in tables {
        let at = b.len();
        let len = 6 + t.code.len();
        b.extend_from_slice(&(len as u16).to_le_bytes());
        b.extend_from_slice(&[1, 1, t.ws, t.ps]);
        b.extend_from_slice(&t.code);
        let e = 0x204 + 2 * *index as usize;
        b[e..e + 2].copy_from_slice(&(at as u16).to_le_bytes());
    }
    for (index, bytes) in data {
        let at = b.len();
        b.extend_from_slice(bytes);
        let e = 0x304 + 2 * *index as usize;
        b[e..e + 2].copy_from_slice(&(at as u16).to_le_bytes());
    }
    b
}

// Opcodes and argument bytes, as atom.c numbers them.
const MOVE_REG: u8 = 1; const MOVE_PS: u8 = 2; const MOVE_WS: u8 = 3; const MOVE_FB: u8 = 4; const MOVE_PLL: u8 = 5;
const AND_REG: u8 = 7; const OR_REG: u8 = 13; const SHIFT_LEFT_PS: u8 = 20; const SHIFT_RIGHT_PS: u8 = 26;
const MUL_PS: u8 = 32; const DIV_PS: u8 = 38; const ADD_PS: u8 = 44; const SUB_PS: u8 = 50;
const SETPORT_ATI: u8 = 55; const SETPORT_PCI: u8 = 56; const SETREGBLOCK: u8 = 58; const SETFBBASE: u8 = 59;
const COMPARE_PS: u8 = 61; const SWITCH: u8 = 66; const JUMP: u8 = 67; const JUMP_EQUAL: u8 = 68; const JUMP_BELOW: u8 = 69;
const JUMP_ABOVE: u8 = 70; const JUMP_NOTEQUAL: u8 = 73; const TEST_PS: u8 = 75; const DELAY_MS: u8 = 80; const DELAY_US: u8 = 81;
const CALLTABLE: u8 = 82; const REPEAT: u8 = 83; const CLEAR_REG: u8 = 84; const NOP: u8 = 90; const EOT: u8 = 91;
const MASK_REG: u8 = 92; const SETDATABLOCK: u8 = 102; const XOR_PS: u8 = 104; const SHL_REG: u8 = 109; const SHR_REG: u8 = 115;
const PROCESSDS: u8 = 122; const MUL32_PS: u8 = 123; const DIV32_PS: u8 = 125;
// attr: source kind | source alignment << 3 | destination selector << 6
const SRC_REG: u8 = 0; const SRC_PS: u8 = 1; const SRC_WS: u8 = 2; const SRC_FB: u8 = 3; const SRC_ID: u8 = 4; const SRC_IMM: u8 = 5;
const SRC_PLL: u8 = 6;
const DWORD: u8 = 0; const WORD0: u8 = 1; const BYTE0: u8 = 4; const BYTE8: u8 = 5;
fn attr(kind: u8, align: u8, sel: u8) -> u8 { kind | align << 3 | sel << 6 }
fn w(v: u16) -> [u8; 2] { v.to_le_bytes() }
fn d(v: u32) -> [u8; 4] { v.to_le_bytes() }
fn code(parts: &[&[u8]]) -> Vec<u8> { parts.concat() }

// OTG0_OTG_H_TOTAL, as the tables name a register: a dword index in BAR 5.
const H_TOTAL0: u16 = 0x4FEA;
fn h_total0() -> u32 { R::OtgHTotal.at(0).unwrap().offset() }

fn run(img: &[u8], table: u8, params: &[u8], m: &mut Model) -> Result<atom::Outcome, Error> {
    let image = Image::new(img)?;
    Interpreter::new(image).run(table, params, m)
}

#[test]
fn arithmetic_on_the_parameter_space_and_the_quotient() {
    let t = T { ws: 2, ps: 8, code: code(&[
        &[ADD_PS, attr(SRC_IMM, DWORD, 0), 0], &d(5),              // PS0 += 5
        &[SUB_PS, attr(SRC_IMM, BYTE0, 0), 1, 3],                  // PS1 -= 3
        &[XOR_PS, attr(SRC_IMM, WORD0, 0), 2], &w(0xFFFF),         // PS2 ^= 0xFFFF
        &[MUL_PS, attr(SRC_IMM, DWORD, 0), 0], &d(3),              // quotient = PS0 * 3
        &[MOVE_PS, attr(SRC_WS, DWORD, 0), 3, 0x40],               // PS3 = quotient
        &[DIV_PS, attr(SRC_IMM, DWORD, 0), 0], &d(4),              // PS0 / 4
        &[MOVE_PS, attr(SRC_WS, DWORD, 0), 4, 0x40], &[MOVE_PS, attr(SRC_WS, DWORD, 0), 5, 0x41],
        &[MUL32_PS, attr(SRC_IMM, DWORD, 0), 6], &d(0x1_0000),     // 64-bit product of PS6 and 65536
        &[MOVE_PS, attr(SRC_WS, DWORD, 0), 7, 0x41],
        &[DIV32_PS, attr(SRC_IMM, DWORD, 0), 6], &d(0x1_0000),     // PS6 with the remainder dword on top, / 65536
        &[MOVE_PS, attr(SRC_WS, DWORD, 0), 8, 0x40],
        &[SHIFT_LEFT_PS, BYTE0 << 3, 9, 4], &[SHIFT_RIGHT_PS, BYTE0 << 3, 10, 4],
        &[NOP, EOT],
    ]) };
    let img = image(&[(1, t)], &[]);
    let mut params = Vec::new();
    for v in [10u32, 10, 0x1234, 0, 0, 0, 0x8000_0001, 0, 0, 0x1F, 0xF0] { params.extend_from_slice(&v.to_le_bytes()); }
    let o = run(&img, 1, &params, &mut Model::default()).unwrap();
    assert_eq!(o.ps[..3], [15, 7, 0xEDCB]);
    assert_eq!((o.ps[3], o.ps[4], o.ps[5]), (45, 3, 3));
    assert_eq!(o.ps[7], 0x8000);
    assert_eq!(o.ps[8], 0x8000_8000); // 0x8000_8000_0001 / 0x10000
    // SHIFT_LEFT and SHIFT_RIGHT on a byte destination keep the byte.
    assert_eq!((o.ps[9], o.ps[10]), (0xF0, 0x0F));
}

#[test]
fn registers_read_and_written_by_dword_index_and_alignment() {
    let t = T { ws: 0, ps: 0, code: code(&[
        &[MOVE_REG, attr(SRC_IMM, DWORD, 0)], &w(H_TOTAL0), &d(0x1122_3344),   // H_TOTAL0 = 0x11223344
        &[MOVE_REG, attr(SRC_IMM, BYTE8, 1)], &w(H_TOTAL0), &[0xAB],           // its bits 15:8 = 0xAB
        &[MOVE_PS, attr(SRC_REG, DWORD, 0), 0], &w(H_TOTAL0),                  // PS0 = H_TOTAL0
        &[AND_REG, attr(SRC_IMM, DWORD, 0)], &w(H_TOTAL0), &d(0xFFFF_00FF),
        &[OR_REG, attr(SRC_IMM, BYTE0, 3)], &w(H_TOTAL0), &[0x02],             // bits 31:24 |= 2
        &[MASK_REG, attr(SRC_IMM, BYTE0, 0)], &w(H_TOTAL0), &[0xF0], &[0x05],  // byte 0 = (byte & 0xF0) | 5
        &[EOT],
    ]) };
    let img = image(&[(2, t)], &[]);
    let mut m = Model::default();
    let o = run(&img, 2, &[], &mut m).unwrap();
    assert_eq!(o.ps[0], 0x1122_AB44);
    assert_eq!(m.regs[&h_total0()], 0x1322_0045);
    // The byte move read the register first, as Linux's does, and merged into it.
    assert_eq!(m.log[..2], [Access::Enter(2, 0), Access::Write(h_total0(), 0x1122_3344, 0)]);
    assert_eq!(m.log[2], Access::Read(h_total0(), 0x1122_3344));
    assert_eq!(m.log[3], Access::Write(h_total0(), 0x1122_AB44, 0x1122_3344));
}

#[test]
fn shl_shr_and_clear_keep_the_other_bits() {
    let t = T { ws: 0, ps: 0, code: code(&[
        &[MOVE_REG, attr(SRC_IMM, DWORD, 0)], &w(H_TOTAL0), &d(0xAABB_CC0F),
        &[SHL_REG, attr(SRC_IMM, BYTE0, 0)], &w(H_TOTAL0), &[4],   // byte 0: (whole << 4) & 0xFF
        &[SHR_REG, attr(SRC_IMM, BYTE8, 1)], &w(H_TOTAL0), &[4],   // byte 1: (whole >> 4) bits 15:8
        &[CLEAR_REG, WORD0 << 3], &w(H_TOTAL0),                   // word 0 cleared
        &[EOT],
    ]) };
    let img = image(&[(3, t)], &[]);
    let mut m = Model::default();
    run(&img, 3, &[], &mut m).unwrap();
    let writes: Vec<u32> = m.log.iter().filter_map(|a| if let Access::Write(_, v, _) = a { Some(*v) } else { None }).collect();
    assert_eq!(writes, vec![0xAABB_CC0F, 0xAABB_CCF0, 0xAABB_BCF0, 0xAABB_0000]);
}

#[test]
fn compare_jumps_test_and_switch() {
    // PS0 == 2: SWITCH to case 2 sets PS1 = 0x22; then COMPARE PS0 with 3 and JUMP_BELOW over a store; TEST bit 1.
    let mut c = vec![];
    c.extend_from_slice(&[SWITCH, attr(SRC_PS, DWORD, 0), 0]);
    c.extend_from_slice(&[0x63]); c.extend_from_slice(&d(1)); let case1 = c.len(); c.extend_from_slice(&w(0));
    c.extend_from_slice(&[0x63]); c.extend_from_slice(&d(2)); let case2 = c.len(); c.extend_from_slice(&w(0));
    c.extend_from_slice(&w(0x5A5A));
    c.extend_from_slice(&[EOT]); // no case
    let l1 = c.len(); c.extend_from_slice(&[MOVE_PS, attr(SRC_IMM, DWORD, 0), 1]); c.extend_from_slice(&d(0x11)); c.push(EOT);
    let l2 = c.len(); c.extend_from_slice(&[MOVE_PS, attr(SRC_IMM, DWORD, 0), 1]); c.extend_from_slice(&d(0x22));
    c.extend_from_slice(&[COMPARE_PS, attr(SRC_IMM, BYTE0, 0), 0, 3]);
    let jb = c.len() + 1; c.extend_from_slice(&[JUMP_BELOW, 0, 0]);
    c.extend_from_slice(&[MOVE_PS, attr(SRC_IMM, DWORD, 0), 2]); c.extend_from_slice(&d(0xBAD));
    let after = c.len();
    c.extend_from_slice(&[TEST_PS, attr(SRC_IMM, BYTE0, 0), 0, 2]);
    let jne = c.len() + 1; c.extend_from_slice(&[JUMP_NOTEQUAL, 0, 0]);
    c.extend_from_slice(&[MOVE_PS, attr(SRC_IMM, DWORD, 0), 3]); c.extend_from_slice(&d(0xBAD));
    let end = c.len(); c.push(EOT);
    for (at, to) in [(case1, l1), (case2, l2), (jb, after), (jne, end)] { c[at..at + 2].copy_from_slice(&((to + 6) as u16).to_le_bytes()); }
    let img = image(&[(4, T { ws: 0, ps: 0, code: c })], &[]);
    let o = run(&img, 4, &2u32.to_le_bytes(), &mut Model::default()).unwrap();
    assert_eq!(o.ps[..4], [2, 0x22, 0, 0]);
    let o = run(&img, 4, &5u32.to_le_bytes(), &mut Model::default()).unwrap();
    assert_eq!(o.ps[1], 0, "no case for 5");
}

#[test]
fn a_call_nests_its_parameters_after_the_callers() {
    // Table 5 (8 parameter bytes) calls 6, whose PS0 is the caller's PS2; 6 writes it and calls an empty entry.
    let callee = T { ws: 0, ps: 0, code: code(&[&[MOVE_PS, attr(SRC_IMM, DWORD, 0), 0], &d(7), &[CALLTABLE, 40, EOT]]) };
    let caller = T { ws: 0, ps: 8, code: code(&[&[CALLTABLE, 6, EOT]]) };
    let img = image(&[(5, caller), (6, callee)], &[]);
    let mut m = Model::default();
    let o = run(&img, 5, &[], &mut m).unwrap();
    assert_eq!(o.ps[2], 7);
    assert_eq!(o.calls, 2);
    assert_eq!(m.log, vec![Access::Enter(5, 0), Access::Enter(6, 1)]);
}

#[test]
fn data_blocks_and_their_reads() {
    let data = { let mut v = vec![0u8; 8]; v[4..8].copy_from_slice(&d(0xC0FFEE)); v };
    let c = code(&[
        &[SETDATABLOCK, 7], &[MOVE_PS, attr(SRC_ID, DWORD, 0), 0], &w(4),     // PS0 = the data table's dword at 4
        &[MOVE_WS, attr(SRC_WS, DWORD, 0), 0, 0x42],                         // WS0 = the data pointer
        &[MOVE_PS, attr(SRC_WS, DWORD, 0), 1, 0],
        &[SETDATABLOCK, 255], &[MOVE_PS, attr(SRC_ID, WORD0, 0), 2], &w(0),   // PS2 = this table's own length
        &[EOT],
    ]);
    let len = 6 + c.len() as u32;
    let img = image(&[(7, T { ws: 1, ps: 0, code: c })], &[(7, data)]);
    let o = run(&img, 7, &[], &mut Model::default()).unwrap();
    assert_eq!(o.ps[0], 0xC0FFEE);
    assert_eq!(o.ps[1] as usize, img.len() - 8);
    assert_eq!(o.ps[2], len);
}

#[test]
fn delays_are_counted_and_bounded() {
    let t = T { ws: 0, ps: 0, code: code(&[&[DELAY_MS, 2, DELAY_US, 50, EOT]]) };
    let img = image(&[(8, t)], &[]);
    let mut m = Model::default();
    assert_eq!(run(&img, 8, &[], &mut m).unwrap().delay_us, 2050);
    assert!(m.log.contains(&Access::Delay(2000)) && m.log.contains(&Access::Delay(50)));
    let spin = T { ws: 0, ps: 0, code: code(&[&[DELAY_MS, 255], &[JUMP], &w(6)]) };
    let img = image(&[(8, spin)], &[]);
    assert!(matches!(run(&img, 8, &[], &mut Model::default()), Err(Error::DelayLimit { table: 8, .. })));
}

// --- the bounds ---

fn fails(t: T, params: &[u8]) -> Error { run(&image(&[(9, t)], &[]), 9, params, &mut Model::default()).unwrap_err() }

#[test]
fn no_register_outside_the_window_and_never_msix() {
    // A table names a register by a 16-bit dword index plus the register block: MSI-X (dwords 0x10800 to 0x10FFF)
    // only through the block.
    let write = |block: u16, dword: u16| T { ws: 0, ps: 0, code: code(&[&[SETREGBLOCK], &w(block), &[MOVE_REG, attr(SRC_IMM, DWORD, 0)], &w(dword), &d(1), &[EOT]]) };
    assert!(matches!(fails(write(0x8000, 0x8800), &[]), Error::Register { dword: 0x10800, write: true, why: RegError::MsiX, .. }));
    assert!(matches!(fails(write(0xFFFF, 0x1000), &[]), Error::Register { dword: 0x10FFF, why: RegError::MsiX, .. }));
    assert!(matches!(fails(write(0, 0), &[]), Error::Register { dword: 0, why: RegError::Outside, .. }));
    assert!(matches!(fails(write(0, 0x8000), &[]), Error::Register { why: RegError::Outside, .. }));
    assert!(matches!(fails(write(0xFFFF, 0xFFFF), &[]), Error::Register { dword: 0x1FFFE, why: RegError::Outside, .. }));
    let mut m = Model::default();
    run(&image(&[(9, write(0x3000, H_TOTAL0 - 0x3000))], &[]), 9, &[], &mut m).unwrap();
    assert_eq!(m.log[1], Access::Write(h_total0(), 1, 0));
    // Through the register block: the block's base plus the index is what is checked.
    let blocked = T { ws: 0, ps: 0, code: code(&[&[SETREGBLOCK], &w(0x8000), &[MOVE_PS, attr(SRC_REG, DWORD, 0), 0], &w(0x8800), &[EOT]]) };
    assert!(matches!(fails(blocked, &[]), Error::Register { dword: 0x10800, write: false, why: RegError::MsiX, .. }));
    // Indirect, PCI and PLL access stop the run.
    let indirect = T { ws: 0, ps: 0, code: code(&[&[SETPORT_ATI], &w(1), &[MOVE_PS, attr(SRC_REG, DWORD, 0), 0], &w(H_TOTAL0), &[EOT]]) };
    assert!(matches!(fails(indirect, &[]), Error::IoMode { mode: 1, .. }));
    let pci = T { ws: 0, ps: 0, code: code(&[&[SETPORT_PCI, 0], &[MOVE_PS, attr(SRC_REG, DWORD, 0), 0], &w(H_TOTAL0), &[EOT]]) };
    assert!(matches!(fails(pci, &[]), Error::IoMode { mode: 0x100, .. }));
    let pll = T { ws: 0, ps: 0, code: code(&[&[MOVE_PS, attr(SRC_PLL, DWORD, 0), 0, 1, EOT]]) };
    assert!(matches!(fails(pll, &[]), Error::Unsupported { kind: 6, .. }));
    let pll_dst = T { ws: 0, ps: 0, code: code(&[&[MOVE_PLL, attr(SRC_IMM, BYTE0, 0), 1, 1, EOT]]) };
    assert!(matches!(fails(pll_dst, &[]), Error::Unsupported { kind: 6, .. }));
}

#[test]
fn every_fetch_and_jump_stays_in_its_table() {
    // A jump to the header, one past the end, a table that runs off its end, a PROCESSDS past it.
    let to = |target: u16| T { ws: 0, ps: 0, code: code(&[&[JUMP], &w(target), &[EOT]]) };
    assert!(matches!(fails(to(2), &[]), Error::OutOfTable { table: 9, .. }));
    assert!(matches!(fails(to(6 + 4), &[]), Error::OutOfTable { .. }));
    assert!(matches!(fails(T { ws: 0, ps: 0, code: vec![NOP, NOP] }, &[]), Error::OutOfTable { .. }));
    assert!(matches!(fails(T { ws: 0, ps: 0, code: code(&[&[PROCESSDS], &w(40), &[EOT]]) }, &[]), Error::OutOfTable { .. }));
    // An operand cut by the table's end.
    assert!(matches!(fails(T { ws: 0, ps: 0, code: vec![MOVE_REG, attr(SRC_IMM, DWORD, 0), 0xEA] }, &[]), Error::OutOfTable { .. }));
    // A loop: the step limit.
    match fails(to(6), &[]) { Error::StepLimit { table: 9, .. } => {}, e => panic!("{:?}", e) }
    assert_eq!(MAX_STEPS, 1_000_000);
}

#[test]
fn spaces_opcodes_calls_and_the_image_are_bounded() {
    assert!(matches!(fails(T { ws: 2, ps: 0, code: code(&[&[MOVE_WS, attr(SRC_IMM, BYTE0, 0), 5, 1, EOT]]) }, &[]), Error::Workspace { index: 5, .. }));
    assert!(matches!(fails(T { ws: 0, ps: 0, code: code(&[&[MOVE_PS, attr(SRC_IMM, BYTE0, 0), 64, 1, EOT]]) }, &[]), Error::Parameters { index: 64, .. }));
    assert!(matches!(fails(T { ws: 0, ps: 0, code: code(&[&[SETFBBASE, attr(SRC_IMM, DWORD, 0)], &d(20 * 1024), &[MOVE_FB, attr(SRC_IMM, BYTE0, 0), 0, 1, EOT]]) }, &[]), Error::Scratch { .. }));
    for op in [0u8, REPEAT, 100, 101, 127, 255] {
        assert!(matches!(fails(T { ws: 0, ps: 0, code: vec![op, EOT] }, &[]), Error::Opcode { op: o, .. } if o == op), "op {}", op);
    }
    assert!(matches!(fails(T { ws: 0, ps: 0, code: vec![SETDATABLOCK, 9, EOT] }, &[]), Error::NoDataTable { index: 9, .. }));
    assert!(matches!(fails(T { ws: 0, ps: 0, code: code(&[&[MOVE_PS, attr(SRC_ID, DWORD, 0), 0], &w(0xFFF0), &[EOT]]) }, &[]), Error::OutOfImage { .. }));
    // A table that calls itself: the depth limit.
    assert!(matches!(fails(T { ws: 0, ps: 0, code: vec![CALLTABLE, 9, EOT] }, &[]), Error::TooDeep { table: 9 }));
    assert_eq!(MAX_DEPTH, 8);
    // A call past the master list, a missing table, a table longer than the image, too many parameters.
    assert!(matches!(fails(T { ws: 0, ps: 0, code: vec![CALLTABLE, 200, EOT] }, &[]), Error::NoSuchTable { index: 200 }));
    let img = image(&[(9, T { ws: 0, ps: 0, code: vec![EOT] })], &[]);
    assert!(matches!(run(&img, 10, &[], &mut Model::default()), Err(Error::NoSuchTable { index: 10 })));
    let mut long = img.clone();
    let base = u16::from_le_bytes([long[0x204 + 18], long[0x205 + 18]]) as usize;
    long[base..base + 2].copy_from_slice(&0x7000u16.to_le_bytes());
    assert!(matches!(run(&long, 9, &[], &mut Model::default()), Err(Error::BadTable { index: 9 })));
    assert!(matches!(run(&img, 9, &[0; 260], &mut Model::default()), Err(Error::Parameters { .. })));
    let mut no_header = img.clone();
    no_header[0x104] = b'X';
    assert!(matches!(Image::new(&no_header), Err(Error::NoRomHeader)));
    assert!(matches!(Image::new(&img[..0x40]), Err(Error::OutOfImage { .. })));
}

// --- the Ryzen's VBIOS ---

// A name for a register a table touched: the map's own, else its segment and dword as the header numbers them.
fn name(offset: u32) -> String {
    for r in dcn::ALL {
        for i in 0..r.instances() {
            if r.at(i).map(|x| x.offset()) == Some(offset) {
                return if r.block().is_empty() { r.name().to_string() } else { format!("{}{}_{}", r.block(), i, r.name()) };
            }
        }
    }
    let dword = offset / 4;
    let (seg, base) = if dword >= 0x34C0 { (2, 0x34C0) } else if dword >= 0xC0 { (1, 0xC0) } else { (0, 0x12) };
    format!("segment {} dword {:#06x}", seg, dword - base)
}

#[test]
fn the_ryzens_tables_for_first_light() {
    let Ok(path) = std::env::var("MIND_AMD_VBIOS") else {
        println!("SKIPPED: MIND_AMD_VBIOS is not set; set it to the Ryzen's VBIOS image (113-CEZANNE-021, 55296 bytes, not in the repository) to run tables 12, 4 and 76");
        return;
    };
    let Ok(bytes) = std::fs::read(&path) else { println!("SKIPPED: MIND_AMD_VBIOS names {}, which cannot be read", path); return };
    assert_eq!(bytes.len(), 55296, "not the Ryzen's image");
    assert!(bytes.windows(15).any(|w| w == b"113-CEZANNE-021"), "not the Ryzen's image");
    let image = Image::new(&bytes).unwrap();
    for (index, format, content) in [(12, 1, 7), (4, 1, 5), (76, 1, 6)] {
        let t = image.command_table(index).unwrap();
        assert_eq!((t.format, t.content), (format, content), "table {}", index);
    }
    // First light's three calls, with the parameters dcn.rs builds for them.
    use dcn::{first_light, Output, Pattern, Pipe, Protected, Signal, Step};
    let plan = first_light(Pipe { otg: 1, opp: 1, dig_fe: 0 }, Output { phy: 0, hpd: 0, encoder_id: 0x1E, connector_id: 0x0C },
        &timing::CEA_1080P60, Signal::Dvi, Pattern::ColorSquares, Protected { otg: 0, opp: 0, dig_fe: 4, phy: 4 }).unwrap();
    let mut interp = Interpreter::new(image);
    let mut m = Model::default();
    // Statuses the hardware sets, which tables 12 and 76 poll with no timeout of their own (a loop on them is what a
    // run on the machine would wait in). PHY A's RDPCSTX0 at segment 2 (DPCS_BASE) of dpcs_2_1_0_offset.h.
    let rdpcs0 = |dword: u32| (0x34C0 + dword) * 4;
    m.answers.push(Answer::Set(rdpcs0(0x2940), 1 << 28)); // RDPCSTX_PHY_CNTL0.RDPCS_SRAM_INIT_DONE
    m.answers.push(Answer::AckFollowsReq(rdpcs0(0x2943), 0x2020_2020)); // RDPCSTX_PHY_CNTL3: each lane's TX_ACK, its TX_REQ
    m.answers.push(Answer::Set(rdpcs0(0x294C), 1 << 7)); // RDPCSTX_PHY_CNTL12.RDPCS_PHY_DP_MPLLB_STATE: the PLL is on
    m.answers.push(Answer::AckFollowsReq(rdpcs0(0x2928), 1 << 2)); // DPCSTX_TX_CLOCK_CNTL: SYMCLK_CLOCK_ON, its SYMCLK_EN
    // RDPCSTX_CLOCK_CNTL: SYMCLK_DIV2_CLOCK_ON follows SYMCLK_DIV2_EN (awaited set at 0xBBE4); SRAMCLK_CLOCK_ON is awaited
    // clear after SRAMCLK_EN is set (0xB84A), so the model leaves it as written.
    m.answers.push(Answer::AckFollowsReq(rdpcs0(0x2931), 1 << 10));
    for step in plan.steps() {
        let Step::Call(table) = step else { continue };
        let (params, n) = table.parameters();
        let first = m.log.len();
        let result = interp.run(table.index(), &params[..n], &mut m);
        println!("== table {} ({:?})", table.index(), table);
        let mut writes = 0;
        for a in &m.log[first..] {
            match a {
                Access::Write(o, v, old) => { writes += 1; println!("  WRITE {:05X} {:<40} = {:08X} (was {:08X})", o, name(*o), v, old); }
                Access::Read(o, v) => println!("  read  {:05X} {:<40} = {:08X}", o, name(*o), v),
                Access::Delay(us) => println!("  delay {} us", us),
                Access::Enter(t, depth) => println!("  enter table {} at depth {}", t, depth),
            }
        }
        match result {
            Ok(o) => println!("  done: {} steps, {} calls, {} us of delays, {} writes; parameters after: {:08X?}", o.steps, o.calls, o.delay_us, writes, &o.ps[..8]),
            Err(e) => println!("  STOPPED: {:?} after {} writes", e, writes),
        }
        // Whatever happened, nothing outside the windows was touched: the model only ever saw a Reg.
        for a in &m.log[first..] {
            if let Access::Read(o, _) | Access::Write(o, _, _) = a { assert!(!dcn::MSIX_BYTES.contains(o) && Reg::new(*o).is_ok()); }
        }
        assert!(result.is_ok(), "table {} did not end: {:?}", table.index(), result);
    }
    // Every write is on PHY A's path or OTG 1's pixel clock (segment, first dword, dwords; dcn_2_1_0_offset.h and
    // dpcs_2_1_0_offset.h): never the firmware's pipe 0, DIG E or PHY E.
    let path: [(&str, u32, u32, u32); 6] = [
        ("PHY A's DPCSTX0 and RDPCSTX0", 2, 0x2928, 0xD8), ("DIG0", 2, 0x2068, 0x100), ("UNIPHYA", 2, 0x286D, 2),
        ("PHYPLLA_PIXCLK_RESYNC_CNTL", 1, 0x40, 1), ("OTG1_PIXEL_RATE_CNTL, OTG1_PHYPLL_PIXEL_RATE_CNTL", 1, 0x84, 4),
        ("SYMCLKA_CLOCK_ENABLE", 1, 0xA0, 1),
    ];
    let base = [0x12u32, 0xC0, 0x34C0];
    let firmware = Protected { otg: 0, opp: 0, dig_fe: 4, phy: 4 };
    let phy_e = (0x34C0 + 0x2C88) * 4..(0x34C0 + 0x2C88 + 0xD8) * 4;
    for a in &m.log {
        let Access::Write(o, _, _) = a else { continue };
        let dword = o / 4;
        assert!(path.iter().any(|(_, s, first, n)| (base[*s as usize] + first..base[*s as usize] + first + n).contains(&dword)),
            "a write off PHY A's path: {:05X} {}", o, name(*o));
        assert!(!firmware.registers().any(|r| r.offset() == *o) && !phy_e.contains(o), "a write to the firmware's output: {:05X}", o);
    }
}
