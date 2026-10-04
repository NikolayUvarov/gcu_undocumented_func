//! Process control (holder of the control capability, i.e. the shell): tasks, focus, logs, diagnostics.
use crate::abi::*;
use crate::sys::{call, check, syscall, Result};

/// Fills `out` with the task table; returns the number of entries.
pub fn tasks(out: &mut [TaskInfo]) -> Result<usize> { check(call(SYSCALL_TASK_LIST, out.as_mut_ptr() as usize, out.len())) }
pub fn kill(pid: u64) -> Result<()> { check(call(SYSCALL_TASK_KILL, pid as usize, 0)).map(drop) }
/// Focuses `pid` (0 = the caller); `keep_output` keeps its buffered console output. Returns the focused PID.
pub fn focus(pid: u64, keep_output: bool) -> Result<u64> { check(call(SYSCALL_FOCUS, pid as usize, keep_output as usize)).map(|p| p as u64) }
/// Drains the task's log (as LOGS shows it).
pub fn logs(pid: u64, out: &mut [u8]) -> Result<usize> { check(syscall(SYSCALL_TASK_LOGS, pid as usize, 0, [out.as_mut_ptr() as usize, out.len(), 0, 0]).result) }
/// Drains the task's console output (mirrored while it is focused).
pub fn console(pid: u64, out: &mut [u8]) -> Result<usize> { check(syscall(SYSCALL_CONSOLE_READ, pid as usize, 0, [out.as_mut_ptr() as usize, out.len(), 0, 0]).result) }

/// What happened to the focused task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice { Exited(u64), Background(u64) }
pub fn notice() -> Option<Notice> {
    match call(SYSCALL_NOTICE, 0, 0) { 0 => None, v if v & NOTICE_EXITED != 0 => Some(Notice::Exited((v & !NOTICE_EXITED) as u64)), v => Some(Notice::Background(v as u64)) }
}
pub fn faults(out: &mut [FaultInfo]) -> Result<usize> { check(call(SYSCALL_FAULTS, out.as_mut_ptr() as usize, out.len())) }

/// (APIC id, online, timer ticks) of CPU `index`, or None past the last CPU.
pub fn cpu(index: usize) -> Option<(u32, bool, usize)> {
    let raw = syscall(SYSCALL_CPU_INFO, index, 0, [0; 4]);
    check(raw.result).ok().map(|apic| (apic as u32, raw.arg2 != 0, raw.msg[2]))
}
/// Kernel heap (used, free, test allocation fully released).
pub fn kernel_heap() -> (usize, usize, bool) { let raw = syscall(SYSCALL_KERNEL_HEAP, 0, 0, [0; 4]); (raw.result, raw.arg2, raw.msg[2] != 0) }
pub fn halt() -> ! { call(SYSCALL_HALT, 0, 0); loop { core::hint::spin_loop(); } }
/// Resets the machine (process control); returns only without the privilege.
pub fn reboot() -> Result<()> { check(call(SYSCALL_REBOOT, 0, 0)).map(drop) }

/// STAT (observe or control privilege): fills `buffer` with a header and records of `class`; `argument` is a PID for
/// VMAP and CAPS. Returns the header; read the records with `records`.
pub fn stat(class: usize, argument: usize, buffer: &mut [u8]) -> Result<StatHeader> {
    check(syscall(SYSCALL_STAT, class, buffer.as_mut_ptr() as usize, [buffer.len(), argument, 0, 0]).result)?;
    Ok(unsafe { core::ptr::read_unaligned(buffer.as_ptr().cast::<StatHeader>()) })
}
/// The records a STAT call wrote into `buffer` (checked against the header's record size and the buffer length).
pub fn records<'a, T: Copy + 'a>(buffer: &'a [u8], header: StatHeader) -> impl Iterator<Item = T> + 'a {
    let (start, size) = (core::mem::size_of::<StatHeader>(), core::mem::size_of::<T>());
    let count = if header.record_size as usize == size { (header.count as usize).min((buffer.len() - start) / size) } else { 0 };
    (0..count).map(move |i| unsafe { core::ptr::read_unaligned(buffer.as_ptr().add(start + i * size).cast::<T>()) })
}

/// Scheduling context of `pid` (its lifecycle owner, or process control): `budget_us` per `period_us` (0: no limit)
/// and the band (BAND_SYSTEM, BAND_APPLICATION or BAND_KEEP).
pub fn sched_set(pid: u64, budget_us: u64, period_us: u64, band: usize) -> Result<()> {
    check(syscall(SYSCALL_SCHED_SET, pid as usize, budget_us as usize, [period_us as usize, band, 0, 0]).result).map(drop)
}
