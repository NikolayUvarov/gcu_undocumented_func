//! The emulator's exits, faults, exclusive monitor and system registers, and the page-table walk (215-PRT-0016):
//! what the differential check of `native.rs` cannot reach, since a correct program never faults.
use a64::mmu::{Devices, Mmu, Tlb};
use a64::{run, Access, Bus, Cpu, Exit, Fault, FaultKind};

const BASE: u64 = 0x10_0000;

struct Flat { memory: Vec<u8>, random: Option<u64> }

impl Bus for Flat {
    fn read(&mut self, va: u64, size: u32, access: Access) -> Result<u64, Fault> {
        let at = va.wrapping_sub(BASE) as usize;
        if at + size as usize > self.memory.len() { return Err(Fault { address: va, access, kind: FaultKind::Translation(3) }); }
        let mut bytes = [0u8; 8];
        bytes[..size as usize].copy_from_slice(&self.memory[at..at + size as usize]);
        Ok(u64::from_le_bytes(bytes))
    }
    fn write(&mut self, va: u64, size: u32, value: u64) -> Result<(), Fault> {
        let at = va.wrapping_sub(BASE) as usize;
        if at + size as usize > self.memory.len() { return Err(Fault { address: va, access: Access::Write, kind: FaultKind::Translation(3) }); }
        self.memory[at..at + size as usize].copy_from_slice(&value.to_le_bytes()[..size as usize]);
        Ok(())
    }
    fn random(&mut self) -> Option<u64> { self.random }
}

fn program(words: &[u32]) -> (Cpu, Flat) {
    let mut flat = Flat { memory: vec![0; 0x10000], random: None };
    for (i, w) in words.iter().enumerate() { flat.memory[4 * i..4 * i + 4].copy_from_slice(&w.to_le_bytes()); }
    let mut cpu = Cpu::new();
    cpu.pc = BASE;
    cpu.sp = BASE + 0x10000;
    (cpu, flat)
}

// Encodings, from the Arm ARM.
fn movz(rd: u32, imm: u32, hw: u32) -> u32 { 0xD280_0000 | hw << 21 | imm << 5 | rd }
fn movk(rd: u32, imm: u32, hw: u32) -> u32 { 0xF280_0000 | hw << 21 | imm << 5 | rd }
fn add_imm(rd: u32, rn: u32, imm: u32) -> u32 { 0x9100_0000 | imm << 10 | rn << 5 | rd }
fn ldr(rt: u32, rn: u32) -> u32 { 0xF940_0000 | rn << 5 | rt }
fn str(rt: u32, rn: u32, offset: u32) -> u32 { 0xF900_0000 | (offset / 8) << 10 | rn << 5 | rt }
fn ldxr(rt: u32, rn: u32) -> u32 { 0xC85F_7C00 | rn << 5 | rt }
fn stxr(rs: u32, rt: u32, rn: u32) -> u32 { 0xC800_7C00 | rs << 16 | rn << 5 | rt }
fn svc(imm: u32) -> u32 { 0xD400_0001 | imm << 5 }
const RNDR_X0: u32 = 0xD53B_2400;
const WFI: u32 = 0xD503_207F;
const FADD: u32 = 0x1E60_2820;

#[test]
fn a_system_call_stops_past_itself_with_its_immediate() {
    let (mut cpu, mut flat) = program(&[movz(0, 0x1234, 0), movk(0, 0x5678, 1), svc(7)]);
    assert_eq!(run(&mut cpu, &mut flat, 100), (Exit::Svc(7), 3));
    assert_eq!(cpu.pc, BASE + 12);
    assert_eq!(cpu.x[0], 0x5678_1234);
}

#[test]
fn an_undefined_instruction_and_a_brk_stop_at_themselves() {
    let (mut cpu, mut flat) = program(&[movz(1, 9, 0), FADD]);
    assert_eq!(run(&mut cpu, &mut flat, 100), (Exit::Undefined(FADD), 1));
    assert_eq!(cpu.pc, BASE + 4);
    let (mut cpu, mut flat) = program(&[0xD420_0020]);
    assert_eq!(run(&mut cpu, &mut flat, 100), (Exit::Brk(1), 0));
    assert_eq!(cpu.pc, BASE);
}

#[test]
fn a_faulting_load_changes_no_register() {
    let (mut cpu, mut flat) = program(&[ldr(1, 2)]);
    cpu.x[1] = 77;
    cpu.x[2] = 0xDEAD_0000;
    let fault = Fault { address: 0xDEAD_0000, access: Access::Read, kind: FaultKind::Translation(3) };
    assert_eq!(run(&mut cpu, &mut flat, 100), (Exit::Fault(fault), 0));
    assert_eq!((cpu.pc, cpu.x[1]), (BASE, 77));
}

#[test]
fn a_misaligned_pc_and_the_budget_stop_the_run() {
    let (mut cpu, mut flat) = program(&[0x1400_0000]); // b .
    assert_eq!(run(&mut cpu, &mut flat, 1000), (Exit::Budget, 1000));
    cpu.pc = BASE + 2;
    let fault = Fault { address: BASE + 2, access: Access::Fetch, kind: FaultKind::Alignment };
    assert_eq!(run(&mut cpu, &mut flat, 1000), (Exit::Fault(fault), 0));
}

#[test]
fn a_store_exclusive_succeeds_only_after_its_load() {
    let data = BASE + 0x8000;
    let (mut cpu, mut flat) = program(&[ldxr(1, 0), add_imm(1, 1, 1), stxr(2, 1, 0), stxr(3, 1, 0), svc(0)]);
    cpu.x[0] = data;
    flat.write(data, 8, 41).unwrap();
    assert_eq!(run(&mut cpu, &mut flat, 100).0, Exit::Svc(0));
    assert_eq!(flat.read(data, 8, Access::Read).unwrap(), 42);
    assert_eq!((cpu.x[2], cpu.x[3]), (0, 1), "the first held the monitor, the second found it cleared");
}

#[test]
fn a_run_clears_the_monitor_as_an_exception_return_does() {
    let data = BASE + 0x8000;
    let (mut cpu, mut flat) = program(&[ldxr(1, 0), svc(0), stxr(2, 1, 0), svc(0)]);
    cpu.x[0] = data;
    assert_eq!(run(&mut cpu, &mut flat, 100).0, Exit::Svc(0));
    assert_eq!(run(&mut cpu, &mut flat, 100).0, Exit::Svc(0));
    assert_eq!(cpu.x[2], 1);
}

#[test]
fn rndr_gives_the_page_s_entropy_or_sets_z() {
    let (mut cpu, mut flat) = program(&[RNDR_X0, svc(0)]);
    cpu.x[0] = 5;
    run(&mut cpu, &mut flat, 100);
    assert_eq!((cpu.x[0], cpu.nzcv), (0, 1 << 30));
    let (mut cpu, mut flat) = program(&[RNDR_X0, svc(0)]);
    flat.random = Some(0xC0FFEE);
    cpu.nzcv = 0xF000_0000;
    run(&mut cpu, &mut flat, 100);
    assert_eq!((cpu.x[0], cpu.nzcv), (0xC0FFEE, 0));
}

#[test]
fn wfi_waits_past_itself() {
    let (mut cpu, mut flat) = program(&[WFI]);
    assert_eq!(run(&mut cpu, &mut flat, 100), (Exit::Wait, 1));
    assert_eq!(cpu.pc, BASE + 4);
}

// The page tables: RAM at RAM_BASE, a level-0 table at its start, user pages from 0x80_0000_0000 (USER_IMAGE).
const RAM_BASE: u64 = 0x4000_0000;
const USER: u64 = 0x80_0000_0000;
const DEVICE: u64 = 0x1000_0000;

struct Mmio { reads: u32 }
impl Devices for Mmio {
    fn read(&mut self, pa: u64, _size: u32) -> Option<u64> { self.reads += 1; (pa == DEVICE).then_some(0xABCD) }
}

/// AP[2:1]: 0 EL1 only, 1 EL0 read and write, 3 EL0 read only.
fn page(pa: u64, ap: u64, uxn: bool) -> u64 { pa | 1 << 10 | ap << 6 | (uxn as u64) << 54 | 3 }

struct Machine { ram: Vec<u8>, tlb: Tlb, mmio: Mmio }

impl Machine {
    fn new() -> Self {
        let mut m = Machine { ram: vec![0; 1 << 20], tlb: Tlb::new(), mmio: Mmio { reads: 0 } };
        m.set(0x0000 + 8, RAM_BASE + 0x1000 | 3); // level 0, index 1: USER's 512 GiB
        m.set(0x1000, RAM_BASE + 0x2000 | 3);
        m.set(0x2000, RAM_BASE + 0x3000 | 3);
        m.pte(0, page(RAM_BASE + 0x1_0000, 3, false)); // code: read and execute
        m.pte(1, page(RAM_BASE + 0x1_1000, 1, true)); // data: read and write
        m.pte(2, page(RAM_BASE + 0x1_2000, 3, true)); // read only
        m.pte(3, page(RAM_BASE + 0x1_3000, 0, true)); // the kernel's
        m.pte(5, page(DEVICE, 1, true)); // a device's registers
        m.pte(6, page(RAM_BASE + 0x1_6000, 1, true) & !(1 << 10)); // access flag clear
        m
    }
    fn set(&mut self, offset: usize, value: u64) { self.ram[offset..offset + 8].copy_from_slice(&value.to_le_bytes()); }
    fn pte(&mut self, index: usize, value: u64) { self.set(0x3000 + 8 * index, value); }
    fn mmu(&mut self) -> Mmu<'_, Mmio> {
        Mmu { ram: &mut self.ram, ram_base: RAM_BASE, ttbr0: RAM_BASE, tlb: &mut self.tlb, devices: &mut self.mmio }
    }
}

#[test]
fn the_walk_maps_the_user_s_pages_and_runs_code_through_them() {
    let mut m = Machine::new();
    for (i, w) in [ldr(1, 2), str(1, 3, 8), svc(0)].iter().enumerate() {
        m.ram[0x1_0000 + 4 * i..0x1_0000 + 4 * i + 4].copy_from_slice(&w.to_le_bytes());
    }
    m.ram[0x1_1000..0x1_1008].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    let mut cpu = Cpu::new();
    cpu.pc = USER;
    cpu.x[2] = USER + 0x1000;
    cpu.x[3] = USER + 0x1000;
    assert_eq!(run(&mut cpu, &mut m.mmu(), 100).0, Exit::Svc(0));
    assert_eq!(&m.ram[0x1_1008..0x1_1010], &0x1122_3344_5566_7788u64.to_le_bytes());
}

#[test]
fn the_walk_refuses_what_the_tables_do_not_give_the_user() {
    let mut m = Machine::new();
    let mut mmu = m.mmu();
    let fault = |address, access, kind| Fault { address, access, kind };
    assert_eq!(mmu.write(USER + 0x2000, 8, 1), Err(fault(USER + 0x2000, Access::Write, FaultKind::Permission(3))));
    assert_eq!(mmu.write(USER, 4, 1), Err(fault(USER, Access::Write, FaultKind::Permission(3))));
    assert_eq!(mmu.read(USER + 0x3000, 8, Access::Read), Err(fault(USER + 0x3000, Access::Read, FaultKind::Permission(3))));
    assert_eq!(mmu.read(USER + 0x1000, 4, Access::Fetch), Err(fault(USER + 0x1000, Access::Fetch, FaultKind::Permission(3))));
    assert_eq!(mmu.read(USER + 0x4000, 8, Access::Read), Err(fault(USER + 0x4000, Access::Read, FaultKind::Translation(3))));
    assert_eq!(mmu.read(USER + 0x6000, 8, Access::Read), Err(fault(USER + 0x6000, Access::Read, FaultKind::AccessFlag(3))));
    assert_eq!(mmu.read(0x40_0000_0000, 8, Access::Read), Err(fault(0x40_0000_0000, Access::Read, FaultKind::Translation(0))));
    assert_eq!(mmu.read(1 << 48, 8, Access::Read), Err(fault(1 << 48, Access::Read, FaultKind::Translation(0))));
    assert_eq!(mmu.read(USER + 0x2000, 8, Access::Read), Ok(0));
    assert_eq!(mmu.read(USER + 0x5000, 4, Access::Read), Ok(0xABCD), "a device's register");
}

#[test]
fn a_write_across_two_pages_changes_nothing_when_the_second_refuses() {
    let mut m = Machine::new();
    let mut mmu = m.mmu();
    let fault = Fault { address: USER + 0x2000, access: Access::Write, kind: FaultKind::Permission(3) };
    assert_eq!(mmu.write(USER + 0x1FFC, 8, u64::MAX), Err(fault));
    assert_eq!(&m.ram[0x1_1FFC..0x1_2000], &[0, 0, 0, 0]);
}

#[test]
fn a_changed_table_is_seen_after_the_kernel_flushes_the_tlb() {
    let mut m = Machine::new();
    m.ram[0x1_1000] = 1;
    m.ram[0x1_7000] = 2;
    assert_eq!(m.mmu().read(USER + 0x1000, 1, Access::Read), Ok(1));
    m.pte(1, page(RAM_BASE + 0x1_7000, 1, true));
    assert_eq!(m.mmu().read(USER + 0x1000, 1, Access::Read), Ok(1), "the old translation, until a flush");
    m.tlb.flush();
    assert_eq!(m.mmu().read(USER + 0x1000, 1, Access::Read), Ok(2));
}
