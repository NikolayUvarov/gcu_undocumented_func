#![no_std]
#![no_main]
// hw: processor (CPUID), clocks, display, PCI devices and interrupt lines with their holders (monitor::hw).
use monitor::hw::{Hw, Local};

mind::request!(REQUEST_SYSINFO);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    let (_, resolution_ns, tsc_hz) = mind::time::clock_info();
    let local = Local { tsc_hz, resolution_ns, width: info.width, height: info.height, stride: info.stride, ..Local::cpuid() };
    monitor::app::run(info, "HW", &mut Hw::new(local));
}
