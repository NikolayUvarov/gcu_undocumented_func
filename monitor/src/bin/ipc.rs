#![no_std]
#![no_main]
// ipc: endpoints with their servers and holders, and who waits for whom (monitor::ipc).
use monitor::ipc::Ipc;

mind::request!(REQUEST_SYSINFO);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    monitor::app::run(info, "IPC", &mut Ipc::new());
}
