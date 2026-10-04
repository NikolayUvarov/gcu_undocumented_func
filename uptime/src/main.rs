#![no_std]
#![no_main]
// uptime: one line with the uptime, load averages over 1/5/15 minutes, the current CPU load and the task count, from
// sysmon (idl/sysinfo.wit). A console program: it asks the shell for a sysmon client and prints into the shell.
use mind::abi::BootInfo;
use mind::idl::sysinfo;
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_SYSINFO);

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("uptime — uptime, load averages over 1, 5 and 15 minutes, the current CPU load and the task count.\nUsage: uptime");
    let load = match sysinfo::load(Endpoint::SYSINFO) {
        Ok(Ok(load)) => load,
        Ok(Err(error)) => { mind::println!("uptime: sysmon: {:?}", error); return; }
        Err(_) => { mind::println!("uptime: no access to sysmon (start it from the shell)"); return; }
    };
    let seconds = load.uptime_ms / 1000;
    mind::print!("up {}:{:02}:{:02}, load {}.{:02} {}.{:02} {}.{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60,
                 load.one / 100, load.one % 100, load.five / 100, load.five % 100, load.fifteen / 100, load.fifteen % 100);
    if let (Ok(Ok(cpus)), Ok(Ok(samples))) = (sysinfo::cpus(Endpoint::SYSINFO), sysinfo::history(Endpoint::SYSINFO, false, 10, 0)) {
        let online = cpus.as_slice().iter().filter(|c| c.online).count().max(1) as u64;
        let (mut busy, mut count, mut tasks) = (0u64, 0u64, 0u8);
        for s in samples.as_slice() {
            busy += (0..4).map(|i| (s.busy_low >> (16 * i)) & 0xFFFF).chain((0..4).map(|i| (s.busy_high >> (16 * i)) & 0xFFFF)).sum::<u64>();
            count += 1; tasks = s.tasks;
        }
        if count > 0 { mind::print!(", cpu {}%, {} tasks", busy / count / online / 10, tasks); }
    }
    mind::println!();
}
