#![no_std]
#![no_main]
// load: load graphs over 30 s or 10 min from sysmon's samples (monitor::load).
mind::request!(REQUEST_SYSINFO);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("load — CPU load graphs from sysmon's samples.\nUsage: load\n1: the last 30 s, 2: the last 10 min, c: total or per processor, q or Esc quit.");
    monitor::app::run(info, "LOAD", &mut monitor::load::LoadView::new());
}
