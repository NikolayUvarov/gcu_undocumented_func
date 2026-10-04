#![no_std]
#![no_main]
// ipc: endpoints with their servers and holders, and who waits for whom (monitor::ipc). The holders of an endpoint
// are part of the authority graph: sysmon tells them to the authority client only.
use monitor::ipc::Ipc;

mind::request!(REQUEST_SYSINFO | REQUEST_AUTHORITY);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("ipc — endpoints with their servers and holders, and who waits for whom.\nUsage: ipc\nTab or 1-2 view, ↑↓ move, Enter holders, i/m/w sort by index/messages/waiting, q or Esc quit.");
    monitor::app::run(info, "IPC", &mut Ipc::new());
}
