// What the page calls (215-PRT-0017): `mind_boot` once, then `mind_turn` whenever it may run the kernel. A turn resumes
// the frame the scheduler last chose: a task's runs on the `a64` emulator until it calls, faults or an interrupt is due,
// and is then handed to `scheduler::interrupt` as an exception entry would hand it on aarch64; the idle loop's runs
// `scheduler::idle`, and with nothing to do the turn returns, saying when the next tick is due.
use super::context::{frame, EC_SVC64, ELR, ESR, FAR, IRQ_KERNEL, IRQ_TASK, KIND, SIZE, SPSR, SP_EL0, SYNC_KERNEL, SYNC_TASK};
use super::{clock, cpu, host, interrupts};
use a64::mmu::{Devices, Mmu, Tlb};
use a64::{Access, Cpu, Exit, Fault, FaultKind};
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::Ordering;
use crate::abi::{BootInfo, BootVolume, ProgramImage, StatPhys, BOOT_HANDOFF_VERSION, BOOT_IMAGES, BOOT_PREFIX_IMAGES, PIXEL_BGR, VOLUME_ELTORITO};

#[repr(C, align(16))]
struct Frame([u8; SIZE]);
/// A task's entry: the frame the aarch64 vectors push on the CPU's exception stack.
static mut ENTRY: Frame = Frame([0; SIZE]);
/// The idle loop's frame (EL1h).
static mut IDLE: Frame = Frame([0; SIZE]);
/// The frame the next turn resumes; 0 before the boot.
static mut CURRENT: usize = 0;
static mut ROOT: u64 = 0;
static mut TLB: Tlb = Tlb::new();
/// The emulator's thread registers: no task saves them on aarch64 either.
static mut TPIDR: (u64, u64) = (0, 0);
/// Memory below this is never RAM to a task (the null page).
const RAM_BASE: u64 = 4096;
/// Instructions between looks at the clock.
const SLICE: u64 = 200_000;
const FPCR_AT: usize = 304 + 512;
const FPSR_AT: usize = 304 + 520;
const EL1H: u64 = 0x5;

unsafe extern "C" {
    static __heap_base: u8;
}

pub fn memory_bytes() -> usize { core::arch::wasm64::memory_size(0) * 65536 }
pub fn flush_translations() { unsafe { (*addr_of_mut!(TLB)).flush(); } }
pub fn set_root(root: u64) { unsafe { ROOT = root; } flush_translations(); }

/// The page's side of the emulator: its devices' registers, RNDR and the counter (CNTVCT in nanoseconds).
struct Page;
impl Devices for Page {
    fn read(&mut self, pa: u64, size: u32) -> Option<u64> { super::devices::read(pa, size) }
    fn write(&mut self, pa: u64, size: u32, value: u64) -> bool { super::devices::write(pa, size, value) }
    fn random(&mut self) -> Option<u64> { Some(unsafe { host::random() }) }
    fn counter(&mut self) -> u64 { clock::now_ns() }
    fn frequency(&mut self) -> u64 { 1_000_000_000 }
}

unsafe fn word(sp: usize, at: usize) -> u64 { core::ptr::read((sp + at) as *const u64) }
unsafe fn set_word(sp: usize, at: usize, value: u64) { core::ptr::write((sp + at) as *mut u64, value) }

unsafe fn load(sp: usize, cpu: &mut Cpu) {
    let f = frame(sp);
    cpu.x.copy_from_slice(&f[..31]);
    (cpu.sp, cpu.pc, cpu.nzcv) = (f[SP_EL0], f[ELR], f[SPSR] & 0xF000_0000);
    (cpu.fpcr, cpu.fpsr) = (word(sp, FPCR_AT), word(sp, FPSR_AT));
    (cpu.tpidr, cpu.tpidrro) = TPIDR;
}

unsafe fn store(cpu: &Cpu, sp: usize) {
    let f = frame(sp);
    f[..31].copy_from_slice(&cpu.x);
    (f[SP_EL0], f[ELR], f[SPSR]) = (cpu.sp, cpu.pc, cpu.nzcv);
    set_word(sp, FPCR_AT, cpu.fpcr);
    set_word(sp, FPSR_AT, cpu.fpsr);
    TPIDR = (cpu.tpidr, cpu.tpidrro);
}

// ESR and FAR as the aarch64 hardware reports an exit from EL0.
fn syndrome(exit: &Exit) -> (u64, u64) {
    const IL: u64 = 1 << 25;
    let status = |kind: FaultKind| match kind {
        FaultKind::Translation(level) => 0x04 | level as u64 & 3,
        FaultKind::AccessFlag(level) => 0x08 | level as u64 & 3,
        FaultKind::Permission(level) => 0x0C | level as u64 & 3,
        FaultKind::Alignment => 0x21,
        FaultKind::Device => 0x10,
    };
    match *exit {
        Exit::Svc(imm) => (EC_SVC64 << 26 | IL | imm as u64, 0),
        Exit::Brk(imm) => (0x3C << 26 | IL | imm as u64, 0),
        Exit::Undefined(_) => (IL, 0),
        Exit::Fault(Fault { address, access: Access::Fetch, kind: FaultKind::Alignment }) => (0x22 << 26 | IL, address),
        Exit::Fault(Fault { address, access: Access::Fetch, kind }) => (0x20 << 26 | IL | status(kind), address),
        Exit::Fault(Fault { address, access, kind }) => (0x24 << 26 | IL | status(kind) | if access == Access::Write { 1 << 6 } else { 0 }, address),
        Exit::Wait | Exit::Budget => (0, 0),
    }
}

/// Where the page may put the boot information, the programs, the volume and the frame pool: above the kernel's data.
#[no_mangle]
pub extern "C" fn mind_heap_base() -> u64 { addr_of!(__heap_base) as u64 }

static mut INFO: core::mem::MaybeUninit<BootInfo> = core::mem::MaybeUninit::uninit();
static mut MAP: [StatPhys; 1] = [StatPhys { kind: 0, index: 0, start: 0, pages: 0 }];
const CONVENTIONAL: u32 = 7;

/// The boot up to init's start, from what the page laid out above `mind_heap_base`, as 64-bit words at `words`:
/// 0-3 the screen (address, width, height, stride in pixels, BGRX), 4-5 the kernel's heap, 6-7 the frame pool, 8-9 the
/// boot volume (a FAT image: a live boot, as from an ISO), 10 a count n, then n (address, length) pairs: the ELF files
/// of BOOT_FILES in order, length 0 for one the page has not. The kernel fills BootInfo itself, as the loader does.
#[no_mangle]
pub unsafe extern "C" fn mind_start(words: *const u64) {
    let w = |i: usize| core::ptr::read(words.add(i));
    let mut info: BootInfo = core::mem::zeroed();
    (info.fb_ptr, info.width, info.height, info.stride, info.pixel_format) = (w(0) as *mut u32, w(1) as usize, w(2) as usize, w(3) as usize, PIXEL_BGR);
    (info.heap_ptr, info.heap_len) = (w(4) as *mut u8, w(5) as usize);
    (*addr_of_mut!(MAP))[0] = StatPhys { kind: CONVENTIONAL, index: 0, start: w(6), pages: w(7) / 4096 };
    (info.memory_map, info.memory_map_len) = (addr_of!(MAP) as *const StatPhys, 1);
    (info.live_volume, info.live_volume_len) = (w(8), w(9));
    if w(9) != 0 { info.boot_volume = BootVolume { kind: VOLUME_ELTORITO, partition: 0, start: 0, sectors: w(9) / 512, signature: [0; 16] }; }
    for i in 0..(w(10) as usize).min(BOOT_IMAGES) {
        let image = ProgramImage { data: w(11 + 2 * i) as *const u8, len: w(12 + 2 * i) as usize };
        if i < BOOT_PREFIX_IMAGES { info.programs[i] = image; } else { info.extra_programs[i - BOOT_PREFIX_IMAGES] = image; }
    }
    (info.abi_version, info.cpu_count) = (BOOT_HANDOFF_VERSION, 1);
    (*addr_of_mut!(INFO)).write(info);
    crate::boot((*addr_of!(INFO)).as_ptr());
    let idle = addr_of_mut!(IDLE) as usize;
    frame(idle)[SPSR] = EL1H;
    frame(idle)[KIND] = SYNC_KERNEL;
    CURRENT = idle;
}

/// An input event from the page for device `device` (0 the tablet, 1 the keyboard), as evdev's type, code and value.
#[no_mangle]
pub extern "C" fn mind_input(device: u32, kind: u32, code: u32, value: u32) { super::devices::input(device as usize, kind as u16, code as u16, value); }

/// A byte typed on the page's console.
#[no_mangle]
pub extern "C" fn mind_console_in(byte: u32) { super::serial::put(byte as u8); }

/// Runs the kernel for about `budget_ms`: 0 when it has more to do at once, else the milliseconds until its next tick.
#[no_mangle]
pub unsafe extern "C" fn mind_turn(budget_ms: u64) -> u64 {
    let end = clock::now_ns() + budget_ms * 1_000_000;
    let entry = addr_of_mut!(ENTRY) as usize;
    while CURRENT != 0 {
        let sp = CURRENT;
        if frame(sp)[SPSR] & 0xF != 0 {
            // The idle loop: reclaim, then reschedule, take an interrupt, or give the page its thread back.
            crate::scheduler::reap();
            crate::scheduler::idle();
            let f = frame(sp);
            if cpu::RESCHEDULE.swap(false, Ordering::Relaxed) {
                (f[KIND], f[ESR]) = (SYNC_KERNEL, EC_SVC64 << 26);
            } else {
                cpu::WAITING.store(false, Ordering::Relaxed);
                let now = clock::now_ns();
                if !interrupts::pending(now) { return (interrupts::until_tick(now) / 1_000_000).max(1); }
                f[KIND] = IRQ_KERNEL;
            }
            CURRENT = crate::scheduler::interrupt(sp);
            continue;
        }
        // A task: run it until it enters the kernel, or an interrupt or the turn's end comes.
        let mut state = Cpu::new();
        load(sp, &mut state);
        let ram = core::slice::from_raw_parts_mut(RAM_BASE as *mut u8, memory_bytes() - RAM_BASE as usize);
        let mut page = Page;
        let mut mmu = Mmu { ram, ram_base: RAM_BASE, ttbr0: ROOT, tlb: &mut *addr_of_mut!(TLB), devices: &mut page };
        let (exit, ended) = loop {
            let (exit, _) = a64::run(&mut state, &mut mmu, SLICE);
            if !matches!(exit, Exit::Budget | Exit::Wait) { break (exit, false); }
            let now = clock::now_ns();
            if interrupts::pending(now) { break (exit, false); }
            if now >= end { break (exit, true); }
        };
        if sp != entry { core::ptr::copy_nonoverlapping(sp as *const u8, entry as *mut u8, SIZE); }
        store(&state, entry);
        if ended { CURRENT = entry; return 0; }
        let (esr, far) = syndrome(&exit);
        let f = frame(entry);
        (f[KIND], f[ESR], f[FAR]) = (if esr == 0 { IRQ_TASK } else { SYNC_TASK }, esr, far);
        CURRENT = crate::scheduler::interrupt(entry);
        if clock::now_ns() >= end { return 0; }
    }
    1
}

