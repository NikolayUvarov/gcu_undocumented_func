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
    #[cfg(target_arch = "x86_64")]
    unsafe { let mxcsr: u32 = 0x1F80; core::arch::asm!("fninit", "ldmxcsr [{}]", in(reg) &mxcsr); }
    // aarch64: FP/SIMD allowed at EL1 for the call (CPACR_EL1.FPEN = 01), trapped again after; tasks hold no FP state.
    #[cfg(target_arch = "aarch64")]
    unsafe { core::arch::asm!("msr cpacr_el1, {}", "isb", in(reg) 1u64 << 20); }
    let result = body();
    #[cfg(target_arch = "aarch64")]
    unsafe { core::arch::asm!("msr cpacr_el1, {}", "isb", in(reg) 0u64); }
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
