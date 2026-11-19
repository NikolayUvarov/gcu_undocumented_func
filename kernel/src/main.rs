#![no_std]
#![no_main]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]
#![feature(allocator_ext)] // Box::try_new_in: a task in the frame pool, refused when the pool is full (171-KRN-0032)

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;
use core::panic::PanicInfo;
use linked_list_allocator::LockedHeap;

static ALLOCATOR: LockedHeap = LockedHeap::empty();
// Never let a timer preempt a shell allocation while it holds the allocator
// lock: another CPU can hold the scheduler lock while waiting for that same
// allocator. All direct ALLOCATOR.lock() users must also have local IRQs off.
struct IrqAllocator;
#[global_allocator]
static GLOBAL_ALLOCATOR: IrqAllocator = IrqAllocator;
unsafe impl GlobalAlloc for IrqAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        interrupts::without(|| ALLOCATOR.alloc(layout))
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        interrupts::without(|| ALLOCATOR.dealloc(ptr, layout));
    }
}
#[path = "../../common/abi.rs"]
mod abi;
#[path = "../../common/fadt.rs"]
mod fadt;
#[path = "../../common/spinlock.rs"]
mod spinlock;
#[path = "../../common/text_ring.rs"]
mod text_ring;
#[path = "../../common/nodes.rs"]
mod nodes;
use abi::BootInfo;
// The processor and platform: only through these names (issue 200).
mod arch;
use arch::{acpi, clock, context, cpu, interrupts, mmu, pcicfg, platform, port, watchdog};
use arch::serial::{init_serial, serial_write_byte};
mod elf;
mod firmware;
mod frames;
#[path = "../../bootloader/src/elf_reloc.rs"]
mod elf_reloc;
mod input;
mod klog;
mod report;
mod memory;
mod paging;
mod pci;
mod platform_nodes;
mod scheduler;
mod screen;
mod task_state;
mod task_table;
mod tpm2;
mod trial;
mod user_heap;

#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let s_u8 = s as *mut u8;
    for i in 0..n {
        core::ptr::write_volatile(s_u8.add(i), c as u8);
    }
    s
}
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let d_u8 = dest as *mut u8;
    let s_u8 = src as *const u8;
    for i in 0..n {
        core::ptr::write_volatile(d_u8.add(i), core::ptr::read_volatile(s_u8.add(i)));
    }
    dest
}
#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
    let s1_u8 = s1 as *const u8;
    let s2_u8 = s2 as *const u8;
    for i in 0..n {
        let a = core::ptr::read_volatile(s1_u8.add(i));
        let b = core::ptr::read_volatile(s2_u8.add(i));
        if a != b {
            return (a as i32) - (b as i32);
        }
    }
    0
}

// Boot line and kernel diagnostics go to COM1, and to the screen until a task takes it (211-KRN-0013); the command
// shell is a ring 3 service started by init.
fn serial_only(text: &str) { for byte in text.bytes() { unsafe { if byte == b'\n' { serial_write_byte(b'\r'); } serial_write_byte(byte); } } }
fn serial_print(text: &str) { serial_only(text); screen::print(text); klog::keep(text); }

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(raw_info: *const BootInfo) -> ! {
    unsafe {
        cpu::disable_interrupts();
        cpu::kernel_level();
        init_serial();
        // Read the fixed ABI 10 prefix before any heap, firmware, memory-map or appended-image pointer is used.
        let Some(version) = BootInfo::read_abi_version(raw_info) else {
            serial_only("KERNEL STOPPED: INVALID BOOT INFORMATION POINTER.\n");
            cpu::halt_here();
        };
        // The loader's handoff version, apart from the system calls' ABI (351-KRN-0166); the test kernel takes only a later one.
        let wanted = if cfg!(feature = "loader-abi-test") { abi::BOOT_HANDOFF_VERSION + 1 } else { abi::BOOT_HANDOFF_VERSION };
        if version != wanted {
            // Every handoff from ABI 10 on keeps that record in front: the screen, the ACPI root and the trial (351-KRN-0167).
            let (trial, rsdp) = if version >= 10 { screen::init_early(raw_info); BootInfo::read_refused(raw_info) } else { (false, 0) };
            acpi::early(rsdp);
            let mut out = Fatal::begin();
            let _ = core::fmt::Write::write_fmt(&mut out, format_args!("\nKERNEL STOPPED: THE BOOTLOADER HANDS OVER VERSION {}, THIS KERNEL TAKES {}. WRITE BOTH FROM ONE BUILD.\n", version, wanted));
            // On a trial the loader has counted the try: a restart boots the fallback instead of leaving the machine stopped.
            if trial { let _ = core::fmt::Write::write_str(&mut out, "TRIAL: RESTARTING INTO THE FALLBACK\n"); acpi::reset(); }
            cpu::halt_here();
        }
        let info = &*raw_info;
        // From here a panic on a trial restarts the machine: the loader has counted the try (801-KRN-0258).
        trial::start(&info.boot_slot);
        // The board from ACPI, else from the device tree a board without ACPI gives (100000-PRT-0009).
        if info.acpi_rsdp != 0 || !platform::board_from_tree(info.device_tree) { acpi::init(info.acpi_rsdp); }
        // The chipset watchdogs that need no page tables, armed before anything else can stop the kernel; the FADT
        // names their PM base (801-KRN-0258, 801-KRN-0259).
        watchdog::early();
        // The devices outside PCI that init may lend, from the tree or else from the firmware's tables (100000-KRN-0171).
        platform::nodes();
        ALLOCATOR.lock().init(info.heap_ptr, info.heap_len);
        // The identity map first: the frame pool writes its lists into RAM above 4 GiB, which only it maps (issue 171).
        paging::init(core::slice::from_raw_parts(info.memory_map, info.memory_map_len)).expect("Kernel page tables");
        // Before anything else can stop the kernel: a PC without a serial port shows why on the screen.
        screen::init(info);
        trial::announce();
        // The other watchdogs, now that the chipset's MMIO and the FADT are reachable, and the boot line (801-KRN-0258).
        // Their feed, the boot CPU's tick, starts after the scheduler; init's BootInfo says whether one restarted the last run.
        watchdog::start();
        // A test kernel on trial stops for good here, before init: only the chipset's watchdog restarts it (801-KRN-0258).
        if cfg!(feature = "watchdog-test") && info.boot_slot.trial != 0 {
            serial_print("MIND CORE KERNEL: WATCHDOG TEST: THE TRIAL STOPPED FOR GOOD BEFORE INIT\n");
            cpu::halt_here();
        }
        firmware::init(info.efi_runtime);
        // A run the kernel stopped on a fault of its own says so at the next boot, and init learns it (801-KRN-0261).
        firmware::last_fault(info.boot_slot.slot);
        firmware::device_tree(info.device_tree);
        frames::init(core::slice::from_raw_parts(info.memory_map, info.memory_map_len));
        cpu::prepare(info).expect("CPU state");
        scheduler::init(info).expect("Scheduler init failed");
        scheduler::spawn_init().expect("init spawn");
        // Printed before any task can run, so it never lands in the middle of a task's console output.
        serial_print("MIND CORE KERNEL: INIT STARTED\n");
        interrupts::init();
        clock::calibrate();
        cpu::start(info);
    }
    // The BSP idle loop reclaims exited tasks and otherwise sleeps until the next interrupt.
    loop {
        scheduler::reap();
        scheduler::idle();
    }
}

/// `HALT` (the shell's `stop`, 000-KRN-0110): the other CPUs stop first, so no task draws over the last lines, then the
/// screen and COM1 say the system stopped.
pub fn halt_asked() -> ! {
    serial_only("MIND CORE KERNEL: STOPPING THE SYSTEM\n");
    cpu::stop_others();
    clock::spin_us(2_000); // a frame being copied to the screen ends before the lines go over it
    screen::last("MIND CORE: STOPPING THE SYSTEM...\nMIND CORE: THE SYSTEM IS STOPPED. IT IS SAFE TO TURN THE MACHINE OFF.\n");
    serial_only("MIND CORE KERNEL: THE SYSTEM IS STOPPED\n");
    cpu::halt_all()
}

// Serial output without locks or allocation, also on the screen until a task takes it.
pub struct PanicSerial;
impl core::fmt::Write for PanicSerial {
    fn write_str(&mut self, text: &str) -> core::fmt::Result { serial_print(text); Ok(()) }
}
// A fatal report, on COM1 and from the top of the screen: the panic may come from the allocator or inside the
// scheduler lock, so neither locks nor allocates.
pub struct Fatal;
impl Fatal { pub fn begin() -> Self { screen::begin_fatal(); Fatal } }
impl core::fmt::Write for Fatal {
    fn write_str(&mut self, text: &str) -> core::fmt::Result { serial_only(text); screen::fatal(text); Ok(()) }
}

// A fatal report's line, kept for the next boot (801-KRN-0261): written beside the report, cut at its first line's end.
pub struct Line { text: [u8; firmware::FAULT_TEXT], length: usize, ended: bool }
impl Line { pub const fn new() -> Self { Line { text: [0; firmware::FAULT_TEXT], length: 0, ended: false } } }
impl core::fmt::Write for Line {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        for &byte in text.as_bytes() {
            if self.ended || self.length == self.text.len() { break; }
            if byte == b'\n' { self.ended = true; } else { self.text[self.length] = byte; self.length += 1; }
        }
        Ok(())
    }
}
// The report and its line together.
pub struct Both<'a>(pub &'a mut Fatal, pub &'a mut Line);
impl core::fmt::Write for Both<'_> {
    fn write_str(&mut self, text: &str) -> core::fmt::Result { let _ = self.0.write_str(text); self.1.write_str(text) }
}

/// Every stop the kernel chose that no task asked for, after its report (801-KRN-0261): the report's line kept in a
/// firmware variable for the next boot, then a trial not yet confirmed restarted into the fallback, its watchdogs left
/// running in case the restart does not come (801-KRN-0258); any other run stops, its watchdog stopped first.
#[allow(unreachable_code)] // x86's reset never returns; aarch64's does where the FADT names no PSCI
pub fn fatal_end(out: &mut Fatal, kind: firmware::Fault, line: &Line) -> ! {
    use core::fmt::Write;
    cpu::stop_others();
    let kept = firmware::record_fault(kind, &line.text[..line.length]);
    let _ = out.write_str(if kept { "THE FAULT IS RECORDED FOR THE NEXT BOOT\n" } else { "THE FAULT IS NOT RECORDED: THE FIRMWARE KEEPS NO VARIABLES, OR WAS IN USE\n" });
    if trial::pending() {
        let _ = out.write_str("TRIAL: RESTARTING INTO THE FALLBACK\n");
        unsafe { acpi::reset(); }
        cpu::freeze();
    }
    cpu::halt_all();
}

static PANICKING: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use core::fmt::Write;
    unsafe { cpu::disable_interrupts(); }
    // A trial not yet confirmed restarts instead: the loader has counted its try, so the next boot is the fallback's,
    // and the watchdog runs on in case the restart does not come (801-KRN-0258).
    let trial = trial::pending();
    // One report: a second panic (another CPU, or inside the formatting) only stops the machine, on a trial with the
    // watchdog left running.
    if PANICKING.swap(true, core::sync::atomic::Ordering::AcqRel) { if trial { cpu::freeze() } cpu::halt_all(); }
    // Other CPUs stop first, so their output does not interleave with the report.
    cpu::stop_others();
    let cpu = cpu::id();
    let (mut out, mut line) = (Fatal::begin(), Line::new());
    let _ = write!(out, "\nKERNEL PANIC: ");
    let mut both = Both(&mut out, &mut line);
    let _ = write!(both, "{}", info.message());
    if let Some(location) = info.location() { let _ = write!(both, " at {}:{}:{}", location.file(), location.line(), location.column()); }
    let _ = write!(both, " CPU={}", cpu);
    let running = &cpu::RUNNING[cpu];
    let pid = running[0].load(core::sync::atomic::Ordering::Relaxed);
    if pid != 0 {
        let mut name = [0u8; 16];
        name[..8].copy_from_slice(&running[1].load(core::sync::atomic::Ordering::Relaxed).to_le_bytes());
        name[8..].copy_from_slice(&running[2].load(core::sync::atomic::Ordering::Relaxed).to_le_bytes());
        let len = name.iter().position(|&b| b == 0).unwrap_or(16);
        let _ = write!(both, " PID={} NAME={}", pid, core::str::from_utf8(&name[..len]).unwrap_or("?"));
    }
    let _ = write!(both, "\n");
    fatal_end(&mut out, firmware::Fault::Panic, &line)
}
