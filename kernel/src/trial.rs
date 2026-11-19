// A trial boot (351-KRN-0014): unless init confirms it in time (BOOT_CONFIRM), the kernel restarts the machine on its
// own tick; the bootloader has already counted the try. A panic on a trial restarts at once, and a stop of the tick
// itself is the chipset watchdog's, where the machine has one (801-KRN-0258).
use crate::abi::{BootSlot, BOOT_SLOT_A, TRIAL_DEADLINE_S};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering::Relaxed};

static DEADLINE_MS: AtomicU64 = AtomicU64::new(0); // 0: no trial, or confirmed
static TRIAL: AtomicBool = AtomicBool::new(false);
static SLOT: AtomicU8 = AtomicU8::new(0); // b'A' or b'B' on a trial

/// As soon as the handoff is read: a panic from here on knows the boot is on trial.
pub fn start(slot: &BootSlot) {
    if slot.trial == 0 { return; }
    let seconds = if slot.deadline_s == 0 { TRIAL_DEADLINE_S } else { slot.deadline_s };
    let seconds = if cfg!(feature = "trial-test") { seconds.min(15) } else { seconds };
    TRIAL.store(true, Relaxed);
    SLOT.store(if slot.slot == BOOT_SLOT_A { b'A' } else { b'B' }, Relaxed);
    DEADLINE_MS.store(seconds as u64 * 1000, Relaxed);
}

/// The boot line, once the screen shows it.
pub fn announce() {
    if !TRIAL.load(Relaxed) { return; }
    let _ = core::fmt::Write::write_fmt(&mut crate::PanicSerial, format_args!("MIND CORE KERNEL: SLOT {} ON TRIAL: A RESTART IN {} S UNLESS INIT CONFIRMS IT\n",
                                                                               SLOT.load(Relaxed) as char, DEADLINE_MS.load(Relaxed) / 1000));
}

/// A trial boot not confirmed yet, its deadline not passed.
pub fn pending() -> bool { TRIAL.load(Relaxed) && DEADLINE_MS.load(Relaxed) != 0 }

/// On the boot CPU's tick: the deadline passed without a confirmation.
pub fn check(now_ms: u64) {
    let deadline = DEADLINE_MS.load(Relaxed);
    if deadline == 0 || now_ms < deadline { return; }
    DEADLINE_MS.store(0, Relaxed);
    let _ = core::fmt::Write::write_fmt(&mut crate::PanicSerial, format_args!("MIND CORE KERNEL: THE TRIAL BOOT WAS NOT CONFIRMED IN {} S: RESTARTING\n", deadline / 1000));
    unsafe { crate::acpi::reboot() }
}

/// The boot is good: no restart at the deadline. Whether it was on trial.
pub fn confirm() -> bool {
    let trial = TRIAL.load(Relaxed);
    if trial && DEADLINE_MS.swap(0, Relaxed) != 0 { let _ = core::fmt::Write::write_fmt(&mut crate::PanicSerial, format_args!("MIND CORE KERNEL: THE TRIAL BOOT IS CONFIRMED\n")); }
    trial
}
