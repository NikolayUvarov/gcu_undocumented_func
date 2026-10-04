#![no_std]
#![no_main]
// top: tasks, CPU and memory use from sysmon (monitor::top). The shell lends it a sysmon client.
mind::request!(REQUEST_SYSINFO);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) { monitor::app::run(info, "TOP", &mut monitor::top::Top::new()); }
