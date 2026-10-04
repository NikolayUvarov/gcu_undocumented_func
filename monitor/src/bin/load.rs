#![no_std]
#![no_main]
// load: load graphs over 30 s or 10 min from sysmon's samples (monitor::load).
mind::request!(REQUEST_SYSINFO);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) { monitor::app::run(info, "LOAD", &mut monitor::load::LoadView::new()); }
