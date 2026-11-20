// What the firmware hands over besides ACPI: its runtime variable services (351-KRN-0027) and a device tree (210-KRN-0029).
// GetVariable and SetVariable are called in the identity map without SetVirtualAddressMap (the firmware's runtime
// regions keep their physical addresses), one CPU at a time. Callers hold the scheduler lock, so interrupts are off;
// the caller's FP state was saved on kernel entry.
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static RUNTIME: AtomicU64 = AtomicU64::new(0);
static BUSY: AtomicBool = AtomicBool::new(false);
const SIGNATURE: u64 = 0x5652_4553_544e_5552; // "RUNTSERV"
const GET_VARIABLE: usize = 72; // offsets in EFI_RUNTIME_SERVICES, after its 24-byte header
const SET_VARIABLE: usize = 88;
const ERROR: usize = 1 << 63;
pub const NOT_FOUND: usize = ERROR | 14;
pub const BUFFER_TOO_SMALL: usize = ERROR | 5;

type GetVariable = unsafe extern "efiapi" fn(*const u16, *const [u8; 16], *mut u32, *mut usize, *mut u8) -> usize;
type SetVariable = unsafe extern "efiapi" fn(*const u16, *const [u8; 16], u32, usize, *const u8) -> usize;

/// The runtime services table the bootloader passed, if it is one.
pub fn init(table: u64) {
    if table == 0 || table >= crate::mmu::IDENTITY_END { return; }
    if unsafe { core::ptr::read_volatile(table as *const u64) } == SIGNATURE { RUNTIME.store(table, Ordering::Release); }
}

fn function(offset: usize) -> Option<usize> {
    let table = RUNTIME.load(Ordering::Acquire);
    if table == 0 { return None; }
    let pointer = unsafe { core::ptr::read_volatile((table as usize + offset) as *const usize) };
    (pointer != 0 && (pointer as u64) < crate::mmu::IDENTITY_END).then_some(pointer) // runtime code the identity map covers
}

// The firmware runs with the control words it expects, and alone.
fn call<T>(body: impl FnOnce() -> T) -> T {
    while BUSY.swap(true, Ordering::Acquire) { core::hint::spin_loop(); }
    entered(body)
}

fn entered<T>(body: impl FnOnce() -> T) -> T {
    #[cfg(target_arch = "x86_64")]
    unsafe { let mxcsr: u32 = 0x1F80; core::arch::asm!("fninit", "ldmxcsr [{}]", in(reg) &mxcsr); }
    // aarch64: FP/SIMD stays enabled (250-KRN-0056); the registers the firmware changes are the task's, which the exit
    // loads again from its frame.
    let result = body();
    BUSY.store(false, Ordering::Release);
    result
}

/// Reads variable `name` (UTF-16, NUL-terminated) of `guid` into `data`: (attributes, length), or the firmware's status;
/// with BUFFER_TOO_SMALL the needed length is in the second field.
pub fn get(name: &[u16], guid: &[u8; 16], data: &mut [u8]) -> Result<(u32, usize), (usize, usize)> {
    let Some(f) = function(GET_VARIABLE) else { return Err((NOT_FOUND, 0)) };
    let (mut attributes, mut length) = (0u32, data.len());
    let status = call(|| unsafe { core::mem::transmute::<usize, GetVariable>(f)(name.as_ptr(), guid, &mut attributes, &mut length, data.as_mut_ptr()) });
    if status == 0 { Ok((attributes, length)) } else { Err((status, length)) }
}

/// Writes variable `name` of `guid`; empty `data` deletes it. The firmware's status on an error.
pub fn set(name: &[u16], guid: &[u8; 16], attributes: u32, data: &[u8]) -> Result<(), usize> {
    let Some(f) = function(SET_VARIABLE) else { return Err(NOT_FOUND) };
    let status = call(|| unsafe { core::mem::transmute::<usize, SetVariable>(f)(name.as_ptr(), guid, attributes, data.len(), data.as_ptr()) });
    if status == 0 { Ok(()) } else { Err(status) }
}

fn utf16<'a>(name: &str, units: &'a mut [u16; 32]) -> &'a [u16] {
    for (unit, byte) in units.iter_mut().zip(name.bytes()) { *unit = byte as u16; }
    &units[..name.len() + 1]
}

/// Deletes init's mark of an unfinished run (000-KRN-0076): this run ends as a task asked.
pub fn clear_run_mark() {
    let _ = set(utf16(crate::abi::RUN_MARK_NAME, &mut [0; 32]), &crate::abi::RUN_MARK_GUID, 0, &[]);
}

// The kernel's record of a run it stopped on a fault of its own (801-KRN-0261, its layout in abi.rs): kept across a
// restart and a cut power, named by the next boot and deleted by its init (801-KRN-0265).
use crate::abi::{FAULT_RECORD_NAME as FAULT_NAME, FAULT_RECORD_VERSION as FAULT_VERSION};
pub use crate::abi::FAULT_RECORD_TEXT as FAULT_TEXT;
const KEPT: u32 = 7; // non-volatile, boot and runtime access

/// What stopped a run.
#[derive(Clone, Copy, PartialEq, Eq)] #[repr(u8)]
pub enum Fault { Panic = crate::abi::FAULT_PANIC, Exception = crate::abi::FAULT_EXCEPTION, InitExited = crate::abi::FAULT_INIT_EXITED, Unexpected = crate::abi::FAULT_UNEXPECTED, Deadline = crate::abi::FAULT_DEADLINE }

static SLOT: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);
static FAULTED: AtomicBool = AtomicBool::new(false);

/// Whether the last run of this slot ended in a fault of the kernel's (ABI 35's BOOT_LAST_RUN_FAULTED): a freeze's
/// evidence for init's safe start. init's own exit and a trial's deadline are not: no service's start is suspected.
pub fn faulted() -> bool { FAULTED.load(Ordering::Relaxed) }

/// At a stop no task asked for: the record for the next boot, and whether it was written. Skipped while the firmware is
/// in use, as the fault may be inside it or another CPU may have stopped there.
pub fn record_fault(kind: Fault, text: &[u8]) -> bool {
    let Some(f) = function(SET_VARIABLE) else { return false };
    if BUSY.swap(true, Ordering::Acquire) { return false; }
    let mut data = [0u8; 3 + FAULT_TEXT];
    let length = text.len().min(FAULT_TEXT);
    data[..3].copy_from_slice(&[FAULT_VERSION, SLOT.load(Ordering::Relaxed), kind as u8]);
    data[3..3 + length].copy_from_slice(&text[..length]);
    let mut units = [0u16; 32];
    let name = utf16(FAULT_NAME, &mut units);
    let status = entered(|| unsafe { core::mem::transmute::<usize, SetVariable>(f)(name.as_ptr(), &crate::abi::RUN_MARK_GUID, KEPT, 3 + length, data.as_ptr()) });
    status == 0
}

/// As the boot starts, once the runtime services are known: the record the last run left, said and left for init.
pub fn last_fault(slot: u32) {
    SLOT.store(slot as u8, Ordering::Relaxed);
    let mut units = [0u16; 32];
    let name = utf16(FAULT_NAME, &mut units);
    let mut data = [0u8; 3 + FAULT_TEXT];
    let found = get(name, &crate::abi::RUN_MARK_GUID, &mut data);
    if matches!(found, Err((NOT_FOUND, _))) { return; }
    let mut out = crate::PanicSerial;
    let _ = match found {
        Ok((_, length)) if length >= 3 && data[0] == FAULT_VERSION => {
            let (kind, text) = (data[2], &data[3..length]);
            let text = match core::str::from_utf8(text) { Ok(text) => text, Err(e) => core::str::from_utf8(&text[..e.valid_up_to()]).unwrap_or("") };
            let how = ["IN THE KERNEL'S PANIC: ", "IN AN EXCEPTION IN THE KERNEL: ", "AS INIT EXITED: ", "IN AN INTERRUPT THE KERNEL HAS NO HANDLER FOR: ", "AT ITS TRIAL'S DEADLINE: "].get((kind as usize).wrapping_sub(1)).unwrap_or(&"IN A FAULT OF A KIND THIS KERNEL DOES NOT NAME: ");
            #[cfg(feature = "panic-test")] PANIC_STAGE.store(if text.starts_with("panic test at ") { 1 } else { 2 }, Ordering::Relaxed);
            let ours = data[1] == slot as u8;
            FAULTED.store(ours && kind != Fault::InitExited as u8 && kind != Fault::Deadline as u8, Ordering::Relaxed);
            let other = match data[1] as u32 { crate::abi::BOOT_SLOT_A => ", OF SLOT A,", crate::abi::BOOT_SLOT_B => ", OF SLOT B,", _ => ", OF THE VOLUME'S ROOT," };
            core::fmt::Write::write_fmt(&mut out, format_args!("MIND CORE KERNEL: THE LAST RUN{} ENDED {}{}\n", if ours { " OF THIS SLOT" } else { other }, how, text))
        }
        _ => core::fmt::Write::write_fmt(&mut out, format_args!("MIND CORE KERNEL: THE LAST RUN LEFT A FAULT RECORD THIS KERNEL CANNOT READ (VERSION {}, NOT {})\n", data[0], FAULT_VERSION)),
    };
}

/// The loader's lines (213-KRN-0262), kept by it in a volatile variable: into the kernel's log alone, as COM1 and the
/// screen showed them already.
pub fn loader_lines() {
    let mut data = [0u8; crate::abi::LOADER_LINES_MAX];
    let Ok((_, length)) = get(utf16(crate::abi::LOADER_LINES_NAME, &mut [0; 32]), &crate::abi::RUN_MARK_GUID, &mut data) else { return };
    let text = &data[..length.min(data.len())];
    crate::klog::keep(core::str::from_utf8(text).unwrap_or_else(|e| core::str::from_utf8(&text[..e.valid_up_to()]).unwrap_or("")));
}

/// The test kernel's panic (801-KRN-0261): 0 at the first log of a boot after no record, 1 on the tick 20 s past the boot
/// after its own first panic, 2 none after that.
#[cfg(feature = "panic-test")]
pub static PANIC_STAGE: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

const FDT_MAGIC: u32 = 0xd00d_feed;

/// The device tree the bootloader found (210-KRN-0029): its header checked and named; the board code reads it (210-APL-0002).
pub fn device_tree(address: u64) -> Option<(u64, usize)> {
    if address == 0 { return None; }
    let header = |offset: u64| unsafe { u32::from_be(core::ptr::read_unaligned((address + offset) as *const u32)) };
    let valid = address < crate::mmu::IDENTITY_END && header(0) == FDT_MAGIC;
    let size = if valid { header(4) as usize } else { 0 };
    let mut out = crate::PanicSerial;
    let _ = if valid { core::fmt::Write::write_fmt(&mut out, format_args!("MIND CORE KERNEL: DEVICE TREE AT {:#x}, {} BYTES, VERSION {}\n", address, size, header(20))) }
            else { core::fmt::Write::write_fmt(&mut out, format_args!("MIND CORE KERNEL: DEVICE TREE AT {:#x}: NO FDT HEADER, IGNORED\n", address)) };
    valid.then_some((address, size))
}
