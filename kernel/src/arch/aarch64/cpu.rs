// CPUs (issues 201, 203): the boot CPU, and the others the ACPI MADT lists, started through PSCI CPU_ON; per-CPU
// exception stacks, the vector base, SGIs between CPUs, and the processor operations the generic kernel uses.
use crate::abi::BootInfo;
use crate::memory::Region;
use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// A table bound, not the hardware's (GICv3 has none; GICv2 addresses 8 interfaces): issue 171.
pub const MAX: usize = 256;
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

/// The boot CPU is CPU 0; the other enabled CPUs of the MADT follow in its order.
pub unsafe fn prepare(info: &BootInfo) -> Result<(), &'static str> {
    let boot = info.apic_ids[0] as u64;
    IDS[0].store(boot, Ordering::Relaxed);
    let mut count = 1;
    for i in 0..super::acpi::CPU_COUNT.load(Ordering::Acquire) {
        let affinity = super::acpi::CPUS[i].load(Ordering::Relaxed) & 0xFF_FFFF;
        let interface = super::acpi::INTERFACES[i].load(Ordering::Relaxed) as usize; // GICv2: the SGI target
        if affinity == boot { super::board::set(&super::board::INTERFACE[0], interface); }
        else if count < MAX { IDS[count].store(affinity, Ordering::Relaxed); super::board::set(&super::board::INTERFACE[count], interface); count += 1; }
    }
    COUNT.store(count, Ordering::Release);
    load()?;
    crate::serial_print(if pan() { "MIND CORE KERNEL: PROTECTION: PXN PAN\n" } else { "MIND CORE KERNEL: PROTECTION: PXN\n" });
    ONLINE[0].store(true, Ordering::Release);
    Ok(())
}

// This CPU's exception stack (TPIDR_EL1 holds its top), FP/SIMD for programs (250-KRN-0056; enabled before the vectors,
// whose common entry saves the registers), the vectors, and the counter readable but nothing else of the timer at EL0.
unsafe fn load() -> Result<(), &'static str> {
    // From the frame pool: many CPUs would otherwise take a large part of the arena (issue 171).
    let stack = Region::task(STACK, 16)?;
    let top = stack.ptr() as usize + STACK;
    core::mem::forget(stack);
    asm!("msr tpidr_el1, {}", in(reg) top);
    asm!("msr cpacr_el1, {}", "isb", in(reg) 3u64 << 20); // FPEN: no trap at EL0 or EL1
    asm!("msr vbar_el1, {}", "isb", in(reg) core::ptr::addr_of!(super::context::exception_vectors) as usize);
    asm!("msr cntkctl_el1, {}", in(reg) 0b10u64); // EL0VCTEN
    // The kernel never reads or writes a program's page through the program's address (PAN) where the core has it; each
    // exception entry sets it again (SPAN clear). Programs' pages are PXN already (000-KRN-0039).
    if pan() {
        let sctlr: u64;
        asm!("mrs {}, sctlr_el1", out(reg) sctlr);
        asm!("msr sctlr_el1, {}", "isb", in(reg) sctlr & !(1 << 23));
        asm!("msr S3_0_C4_C2_3, {}", "isb", in(reg) 1u64 << 22);
    }
    Ok(())
}
/// Whether the core has Privileged Access Never (ID_AA64MMFR1_EL1.PAN).
pub fn pan() -> bool { let mmfr1: u64; unsafe { asm!("mrs {}, id_aa64mmfr1_el1", out(reg) mmfr1); } (mmfr1 >> 20) & 0xF != 0 }

// A secondary CPU starts at ap_boot with its MMU and caches off and x0 = its record: the boot CPU's translation
// registers and system control, its stack, its index and the Rust entry. It turns the MMU on like the boot CPU's and
// jumps; the kernel is identity-mapped, so addresses do not change.
#[repr(C, align(64))]
struct Record { mair: u64, tcr: u64, ttbr0: u64, sctlr: u64, stack: u64, index: u64, entry: u64 }
static mut RECORDS: [Record; MAX] = [const { Record { mair: 0, tcr: 0, ttbr0: 0, sctlr: 0, stack: 0, index: 0, entry: 0 } }; MAX];

global_asm!(r#"
    .section .text.ap_boot,"ax"
    .balign 64
    .global ap_boot, ap_boot_end
ap_boot:
    msr daifset, #0xf
    ldr x1, [x0, #0]
    msr mair_el1, x1
    ldr x1, [x0, #8]
    msr tcr_el1, x1
    ldr x1, [x0, #16]
    msr ttbr0_el1, x1
    isb
    tlbi vmalle1
    dsb nsh
    ldr x1, [x0, #24]
    msr sctlr_el1, x1
    isb
    ldr x1, [x0, #32]
    mov sp, x1
    ldr x2, [x0, #48]
    ldr x0, [x0, #40]
    br x2
ap_boot_end:
    .previous
"#);
unsafe extern "C" { static ap_boot: u8; static ap_boot_end: u8; }

// Writes the data cache lines of [start, start + len) back to memory, for a CPU that reads it with caches off.
unsafe fn clean(start: usize, len: usize) {
    let mut line = start & !63;
    while line < start + len { asm!("dc cvac, {}", in(reg) line); line += 64; }
    asm!("dsb sy");
}

/// Writes the cache lines of [start, start + len) back to memory: the kernel's text on a write-back framebuffer.
pub unsafe fn write_back(start: usize, len: usize) { clean(start, len) }

/// Starts the other CPUs through PSCI CPU_ON and waits until each is online (a second at most each).
pub unsafe fn start(_info: &BootInfo) {
    let (mair, tcr, sctlr): (u64, u64, u64);
    asm!("mrs {}, mair_el1", "mrs {}, tcr_el1", "mrs {}, sctlr_el1", out(reg) mair, out(reg) tcr, out(reg) sctlr);
    let code = core::ptr::addr_of!(ap_boot) as usize;
    clean(code, core::ptr::addr_of!(ap_boot_end) as usize - code);
    for index in 1..COUNT.load(Ordering::Acquire) {
        let Ok(stack) = Region::task(STACK, 16) else { break };
        let top = stack.ptr() as u64 + STACK as u64;
        core::mem::forget(stack);
        let record = &mut *core::ptr::addr_of_mut!(RECORDS[index]);
        *record = Record { mair, tcr, ttbr0: crate::mmu::kernel_root() as u64, sctlr, stack: top, index: index as u64, entry: ap_entry as *const () as u64 };
        clean(record as *const Record as usize, core::mem::size_of::<Record>());
        let affinity = IDS[index].load(Ordering::Relaxed);
        let status = super::acpi::psci(super::acpi::PSCI_CPU_ON, affinity, code as u64, record as *const Record as u64);
        if status != 0 { crate::serial_print("MIND CORE KERNEL: PSCI CPU_ON REFUSED\n"); continue; }
        let deadline = cycles() + super::clock::tsc_hz();
        while !ONLINE[index].load(Ordering::Acquire) && cycles() < deadline { core::hint::spin_loop(); }
        if !ONLINE[index].load(Ordering::Acquire) { crate::serial_print("MIND CORE KERNEL: A CPU DID NOT COME ONLINE\n"); }
    }
}

extern "C" fn ap_entry(index: usize) -> ! {
    unsafe {
        if load().is_err() { halt_here(); }
        crate::interrupts::load();
        ONLINE[index].store(true, Ordering::Release);
    }
    loop { crate::scheduler::idle(); }
}

// SGIs to the other online CPUs: the tick (from the boot CPU's timer), a wake-up, a stop.
// The tick goes only to CPUs that run a task (time slices); an idle CPU sleeps until a wake SGI (issue 171).
pub unsafe fn tick_others() {
    for i in 1..COUNT.load(Ordering::Acquire) {
        if ONLINE[i].load(Ordering::Acquire) && RUNNING[i][0].load(Ordering::Relaxed) != 0 { crate::interrupts::sgi(i, IDS[i].load(Ordering::Relaxed), crate::interrupts::SGI_TICK); }
    }
}
pub unsafe fn wake(index: usize) {
    if ONLINE[index].load(Ordering::Acquire) { crate::interrupts::sgi(index, IDS[index].load(Ordering::Relaxed), crate::interrupts::SGI_WAKE); }
}
pub fn stop_others() {
    let this = id();
    for i in 0..COUNT.load(Ordering::Acquire) {
        if i != this && ONLINE[i].load(Ordering::Acquire) { unsafe { crate::interrupts::sgi(i, IDS[i].load(Ordering::Relaxed), crate::interrupts::SGI_STOP); } }
    }
}

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
