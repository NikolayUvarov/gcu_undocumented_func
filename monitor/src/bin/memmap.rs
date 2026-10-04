#![no_std]
#![no_main]
// memmap: physical memory map, kernel arena, address spaces and quotas from sysmon (monitor::memmap).
mind::request!(REQUEST_SYSINFO);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) { monitor::app::run(info, "MEMMAP", &mut monitor::memmap::Memmap::new()); }
