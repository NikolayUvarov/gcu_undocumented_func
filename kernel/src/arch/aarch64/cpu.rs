// CPUs (issue 201: the boot CPU; the others start through PSCI in issue 203): per-CPU exception stacks, the
// vector base, and the processor operations the generic kernel uses.
use crate::abi::BootInfo;
use crate::memory::Region;
use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

pub const MAX: usize = 8;
pub static COUNT: AtomicUsize = AtomicUsize::new(1);
pub static ONLINE: [AtomicBool; MAX] = [const { AtomicBool::new(false) }; MAX];
pub static TICKS: [AtomicU64; MAX] = [const { AtomicU64::new(0) }; MAX];
// PID and name (16 bytes) of the task running on each CPU, 0 when idle: read by the panic handler without locks.
pub static RUNNING: [[AtomicU64; 3]; MAX] = [const { [const { AtomicU64::new(0) }; 3] }; MAX];
static IDS: [AtomicU64; MAX] = [const { AtomicU64::new(0) }; MAX]; // MPIDR affinity of each CPU
const STACK: usize = 64 * 1024;

pub fn id() -> usize {
    let mpidr: u64; unsafe { asm!("mrs {}, mpidr_el1", out(reg) mpidr); }
    let affinity = mpidr & 0xFF_FFFF;
    (0..COUNT.load(Ordering::Acquire)).find(|&i| IDS[i].load(Ordering::Relaxed) == affinity).unwrap_or(0)
}
/// The CPU's hardware ID (MPIDR affinity; the APIC ID on x86).
pub fn apic_id(index: usize) -> u32 { IDS[index].load(Ordering::Relaxed) as u32 }

pub unsafe fn prepare(info: &BootInfo) -> Result<(), &'static str> {
    COUNT.store(1, Ordering::Release);
    IDS[0].store(info.apic_ids[0] as u64, Ordering::Relaxed);
    load()?;
    ONLINE[0].store(true, Ordering::Release);
    Ok(())
}

// This CPU's exception stack (TPIDR_EL1 holds its top), the vectors, no FP/SIMD at EL0 or EL1, and the counter
// readable but nothing else of the timer at EL0.
unsafe fn load() -> Result<(), &'static str> {
    let stack = Region::new(STACK, 16)?;
    let top = stack.ptr() as usize + STACK;
    core::mem::forget(stack);
    asm!("msr tpidr_el1, {}", in(reg) top);
    asm!("msr vbar_el1, {}", "isb", in(reg) core::ptr::addr_of!(super::context::exception_vectors) as usize);
    asm!("msr cpacr_el1, {}", "isb", in(reg) 0u64);
    asm!("msr cntkctl_el1, {}", in(reg) 0b10u64); // EL0VCTEN
    Ok(())
}

pub unsafe fn start(_info: &BootInfo) {}

// One CPU so far: nothing to signal (SGIs between CPUs: issue 203).
pub unsafe fn tick_others() {}
pub unsafe fn wake(_index: usize) {}
pub fn stop_others() {}

pub fn halt_all() -> ! {
    unsafe { asm!("msr daifset, #0xf"); }
    stop_others();
    loop { unsafe { asm!("wfi"); } }
}

pub fn cycles() -> u64 { super::clock::cycles() }
/// Enters the scheduler from kernel mode, as a system call does.
pub unsafe fn reschedule() { asm!("svc #0"); }
/// Sleeps until an interrupt with interrupts enabled only meanwhile.
pub unsafe fn wait_for_interrupt() { asm!("msr daifclr, #2", "wfi", "msr daifset, #2"); }
pub unsafe fn disable_interrupts() { asm!("msr daifset, #2"); }
pub unsafe fn halt_here() -> ! { asm!("msr daifset, #0xf"); loop { asm!("wfi"); } }
/// Code was written to memory a task will execute (program images, the exit page).
pub fn code_written() { unsafe { asm!("dsb ish", "ic ialluis", "dsb ish", "isb"); } }

/// What the processor offers programs (BootInfo.cpu_features): RNDR when ID_AA64ISAR0_EL1 lists it.
pub fn features() -> u64 {
    let isar0: u64; unsafe { asm!("mrs {}, id_aa64isar0_el1", out(reg) isar0); }
    if isar0 >> 60 != 0 { crate::abi::FEATURE_ENTROPY } else { 0 }
}
