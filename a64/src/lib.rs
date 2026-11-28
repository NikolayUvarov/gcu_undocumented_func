//! User-mode A64 for MIND Core's kernel in a browser tab (215-PRT-0016): the base integer instructions an EL0 program
//! of the aarch64 build uses, run until a system call, a fault, an undefined instruction or the end of a budget.
//! Memory goes through a `Bus`; `mmu::Mmu` is one that walks the kernel's own page tables.
#![no_std]

mod exec;
pub mod mmu;

/// What an access was for, as a fault reports it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Access { Read, Write, Fetch }

/// Why an access failed: no translation at a level, a permission refused at a level, a misaligned exclusive or
/// instruction fetch, or a device that answered nothing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaultKind { Translation(u8), AccessFlag(u8), Permission(u8), Alignment, Device }

/// A failed access: its virtual address, what it was for and why. The instruction did not complete; PC names it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fault { pub address: u64, pub access: Access, pub kind: FaultKind }

/// The memory and the few system values a program reads.
pub trait Bus {
    /// `size` bytes (1, 2, 4 or 8) at `va`, little-endian.
    fn read(&mut self, va: u64, size: u32, access: Access) -> Result<u64, Fault>;
    /// `size` bytes (1, 2, 4 or 8) of `value` at `va`, little-endian.
    fn write(&mut self, va: u64, size: u32, value: u64) -> Result<(), Fault>;
    /// RNDR's value, or None when none is ready (the instruction then sets Z and returns 0).
    fn random(&mut self) -> Option<u64> { None }
    /// CNTVCT_EL0.
    fn counter(&mut self) -> u64 { 0 }
    /// CNTFRQ_EL0.
    fn frequency(&mut self) -> u64 { 0 }
}

/// Why `run` returned. For `Svc` and `Wait`, PC is past the instruction, as ELR_EL1 would hold; for every other exit
/// it names the instruction, which did not complete.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exit {
    Svc(u16),
    Brk(u16),
    Fault(Fault),
    Undefined(u32),
    /// `wfi` or `wfe`: nothing to do until an event.
    Wait,
    /// The budget of instructions ran out.
    Budget,
}

/// A task's user registers. NZCV is kept in bits 31..28, as the register reads.
#[derive(Clone, Debug, Default)]
pub struct Cpu {
    pub x: [u64; 31],
    pub sp: u64,
    pub pc: u64,
    pub nzcv: u64,
    pub tpidr: u64,
    pub tpidrro: u64,
    pub fpcr: u64,
    pub fpsr: u64,
    /// The exclusive monitor: the address and size of the last exclusive load.
    pub(crate) monitor: Option<(u64, u32)>,
}

impl Cpu {
    pub const fn new() -> Self {
        Self { x: [0; 31], sp: 0, pc: 0, nzcv: 0, tpidr: 0, tpidrro: 0, fpcr: 0, fpsr: 0, monitor: None }
    }
    /// Clears the exclusive monitor, as an exception return does.
    pub fn clear_exclusive(&mut self) { self.monitor = None; }
}

/// Runs `cpu` on `bus` for at most `budget` instructions. Returns why it stopped and how many instructions completed.
pub fn run<B: Bus>(cpu: &mut Cpu, bus: &mut B, budget: u64) -> (Exit, u64) {
    cpu.monitor = None;
    let mut done = 0;
    while done < budget {
        let pc = cpu.pc;
        if pc & 3 != 0 {
            return (Exit::Fault(Fault { address: pc, access: Access::Fetch, kind: FaultKind::Alignment }), done);
        }
        let word = match bus.read(pc, 4, Access::Fetch) {
            Ok(word) => word as u32,
            Err(fault) => return (Exit::Fault(fault), done),
        };
        match exec::step(cpu, bus, word) {
            Ok(()) => done += 1,
            Err(exec::Stop::Exit(exit)) => {
                if matches!(exit, Exit::Svc(_) | Exit::Wait) { done += 1; }
                return (exit, done);
            }
        }
    }
    (Exit::Budget, done)
}
