// One CPU, the page's thread (215-PRT-0017). Waiting and rescheduling are flags `run` reads after the idle loop's turn.
use crate::abi::BootInfo;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

pub const MAX: usize = 8;
pub static COUNT: AtomicUsize = AtomicUsize::new(1);
pub static ONLINE: [AtomicBool; MAX] = [const { AtomicBool::new(false) }; MAX];
pub static TICKS: [AtomicU64; MAX] = [const { AtomicU64::new(0) }; MAX];
/// What CPU i runs: the task's PID and its name's 16 bytes, for a panic's report.
pub static RUNNING: [[AtomicU64; 3]; MAX] = [const { [const { AtomicU64::new(0) }; 3] }; MAX];
/// The idle loop asked to reschedule, or to wait for an interrupt.
pub static RESCHEDULE: AtomicBool = AtomicBool::new(false);
pub static WAITING: AtomicBool = AtomicBool::new(false);

pub fn id() -> usize { 0 }
pub fn apic_id(_index: usize) -> u32 { 0 }
pub unsafe fn prepare(_info: &BootInfo) -> Result<(), &'static str> {
    crate::serial_print("MIND CORE KERNEL: PROTECTION: THE EMULATOR'S PAGE TABLE CHECKS (NO HARDWARE)\n");
    ONLINE[0].store(true, Ordering::Release);
    Ok(())
}
pub unsafe fn kernel_level() {}
pub unsafe fn write_back(_start: usize, _len: usize) {}
pub unsafe fn start(_info: &BootInfo) {}
pub unsafe fn wake(_index: usize) {}
pub fn stop_others() {}
pub fn halt_all() -> ! { super::host::stop(0) }
pub fn freeze() -> ! { super::host::stop(2) }
pub fn cycles() -> u64 { super::clock::cycles() }
pub unsafe fn reschedule() { RESCHEDULE.store(true, Ordering::Relaxed); }
pub unsafe fn wait_for_interrupt() { WAITING.store(true, Ordering::Relaxed); }
pub unsafe fn disable_interrupts() {}
pub unsafe fn halt_here() -> ! { super::host::stop(2) }
/// The emulator decodes each instruction from memory: nothing is cached to clean.
pub fn code_written() {}
/// RNDR comes from the page's random source.
pub fn features() -> u64 { crate::abi::FEATURE_ENTROPY }
