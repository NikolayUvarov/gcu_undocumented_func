// No watchdog in a page (215-PRT-0017): a kernel that stops is seen by the page.
pub fn early() {}
pub fn start() { crate::serial_print("MIND CORE KERNEL: NO WATCHDOG: THE PAGE RUNS THE KERNEL\n"); }
pub fn restarted() -> bool { false }
