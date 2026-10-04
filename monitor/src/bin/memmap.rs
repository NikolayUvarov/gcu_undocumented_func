#![no_std]
#![no_main]
// memmap: physical memory map, kernel arena, address spaces and quotas from sysmon (monitor::memmap).
mind::request!(REQUEST_SYSINFO);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("memmap — memory: the physical map, the kernel arena, address spaces and quotas.\nUsage: memmap\n1-4 or Tab view, ↑↓ move, m merge regions, z zoom, q or Esc quit.");
    monitor::app::run(info, "MEMMAP", &mut monitor::memmap::Memmap::new());
}
