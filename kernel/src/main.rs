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
use abi::BootInfo;
// The processor and platform: only through these names (issue 200).
mod arch;
use arch::{acpi, clock, context, cpu, interrupts, mmu, pcicfg, platform, port};
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
        acpi::init(info.acpi_rsdp);
        ALLOCATOR.lock().init(info.heap_ptr, info.heap_len);
        // The identity map first: the frame pool writes its lists into RAM above 4 GiB, which only it maps (issue 171).
        paging::init(core::slice::from_raw_parts(info.memory_map, info.memory_map_len)).expect("Kernel page tables");
        // Before anything else can stop the kernel: a PC without a serial port shows why on the screen.
        screen::init(info);
        trial::start(&info.boot_slot);
        firmware::init(info.efi_runtime);
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

static PANICKING: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use core::fmt::Write;
    unsafe { cpu::disable_interrupts(); }
    // One report: a second panic (another CPU, or inside the formatting) only stops the machine.
    if PANICKING.swap(true, core::sync::atomic::Ordering::AcqRel) { cpu::halt_all(); }
    // Other CPUs stop first, so their output does not interleave with the report.
    cpu::stop_others();
    let cpu = cpu::id();
    let mut out = Fatal::begin();
    let _ = write!(out, "\nKERNEL PANIC: {}", info.message());
    if let Some(location) = info.location() { let _ = write!(out, " at {}:{}:{}", location.file(), location.line(), location.column()); }
    let _ = write!(out, " CPU={}", cpu);
    let running = &cpu::RUNNING[cpu];
    let pid = running[0].load(core::sync::atomic::Ordering::Relaxed);
    if pid != 0 {
        let mut name = [0u8; 16];
        name[..8].copy_from_slice(&running[1].load(core::sync::atomic::Ordering::Relaxed).to_le_bytes());
        name[8..].copy_from_slice(&running[2].load(core::sync::atomic::Ordering::Relaxed).to_le_bytes());
        let len = name.iter().position(|&b| b == 0).unwrap_or(16);
        let _ = write!(out, " PID={} NAME={}", pid, core::str::from_utf8(&name[..len]).unwrap_or("?"));
    }
    let _ = write!(out, "\n");
    cpu::halt_all();
}
