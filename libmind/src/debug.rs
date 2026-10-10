//! Debug mode (211-KRN-0053): a file `debug.txt` on the log volume (`log:debug.txt`), made from the shell
//! (`write log:debug.txt on`) or on any computer that mounts the MIND LOG partition. While it is there, the loader gives
//! every program a log client, so what a program prints reaches the system log and this boot's log file, and programs
//! may say more of what they do. Removing the file ends it for programs started afterwards.
use core::sync::atomic::{AtomicU8, Ordering};

/// The file whose presence turns debug mode on.
pub const FILE: &str = "log:debug.txt";

const UNKNOWN: u8 = 0; const ON: u8 = 1; const OFF: u8 = 2;
static STATE: AtomicU8 = AtomicU8::new(UNKNOWN);

/// Whether debug mode is on, looked at once in this process.
pub fn on() -> bool {
    match STATE.load(Ordering::Relaxed) {
        ON => true,
        OFF => false,
        _ => { let on = check(); STATE.store(if on { ON } else { OFF }, Ordering::Relaxed); on }
    }
}

/// Whether debug mode is on now (the loader asks at every launch).
pub fn check() -> bool { crate::fs::File::open(FILE).is_ok() }
