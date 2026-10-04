#![no_std]
#![no_main]
// caps [pid]: the capabilities of a task, the derivation tree across tasks and what a revoke would remove
// (monitor::caps). Asks for the authority client: sysmon tells who holds what only to it.
use monitor::caps::Caps;

mind::request!(REQUEST_SYSINFO | REQUEST_AUTHORITY);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("caps — the capabilities of every task, their derivation tree and what a revoke would remove.\nUsage: caps   (run caps <pid>: start at that task; the shell's caps <pid> prints one task's list)\nTab or 1-2 view, ↑↓ move, ←→ task, Enter what a revoke removes, r refresh, q or Esc quit.");
    let pid = mind::process::args_str().trim().parse::<u64>().unwrap_or(0);
    monitor::app::run(info, "CAPS", &mut Caps::new(pid));
}
