#![no_std]
#![no_main]
// caps [pid]: the capabilities of a task, the derivation tree across tasks and what a revoke would remove
// (monitor::caps). Asks for the authority client: sysmon tells who holds what only to it.
use monitor::caps::Caps;

mind::request!(REQUEST_SYSINFO | REQUEST_AUTHORITY);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    let pid = mind::process::args_str().trim().parse::<u64>().unwrap_or(0);
    monitor::app::run(info, "CAPS", &mut Caps::new(pid));
}
