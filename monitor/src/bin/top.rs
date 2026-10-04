#![no_std]
#![no_main]
// top: tasks, CPU and memory use from sysmon (monitor::top). The shell lends it a sysmon client and its client of
// init, through which k stops a task and r restarts a service.
mind::request!(REQUEST_SYSINFO | REQUEST_LIFECYCLE);

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("top — tasks with their CPU and memory use, from sysmon.\nUsage: top\nP/M/N/T sort, S services, t tree, Enter details, k stop, r restart a service, +/- interval, q or Esc quit.");
    monitor::app::run(info, "TOP", &mut monitor::top::Top::new());
}
