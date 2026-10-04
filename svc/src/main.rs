#![no_std]
#![no_main]
// svc [list | start <service> | stop <service or PID> | restart <service>]: boot services through init's lifecycle
// interface (idl/init.wit 1.1) — PID, how often init started each, whether it runs or is quarantined, what init gave
// it; start, stop and restart a service, stop an application by PID. A console program: it asks the shell for
// lifecycle control.
use mind::abi::*;
use mind::idl::init as lifecycle;
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_LIFECYCLE);

const LIFECYCLE: Endpoint = Endpoint(SLOT_LIFECYCLE);

fn reason(error: lifecycle::Error) -> &'static str {
    match error {
        lifecycle::Error::NotFound => "no such service or task",
        lifecycle::Error::Running => "it runs already",
        lifecycle::Error::Stopped => "it does not run",
        lifecycle::Error::Denied => "init and the shell cannot be stopped",
        lifecycle::Error::NoDevice => "its device is missing",
        lifecycle::Error::Failed => "init could not start it",
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("svc — the boot services through init: PID, starts, running or quarantined, what init gave each.\nUsage: svc [list | start <service> | stop <service or PID> | restart <service>]");
    if mind::dev::cap_info(SLOT_LIFECYCLE).0 != CAP_KIND_ENDPOINT { mind::println!("svc: no lifecycle control here (start svc from the shell)"); return; }
    let mut words = mind::process::args_str().split_whitespace();
    let (command, name) = (words.next().unwrap_or("list"), words.next());
    let failed = |what: &str, name: &str, error: Result<lifecycle::Error, mind::Error>| match error {
        Ok(error) => mind::println!("svc: {} {}: {}", what, name, reason(error)),
        Err(error) => mind::println!("svc: init did not answer: {:?}", error),
    };
    match (command, name) {
        ("list", None) => match lifecycle::list(LIFECYCLE) {
            Ok(Ok(services)) => {
                mind::println!("SERVICE         PID  STARTS  STATE    HOLDS");
                for s in services.as_slice() {
                    let state = if s.running { "running" } else if s.quarantined { "quarantined" } else { "stopped" };
                    mind::println!("{:<12} {:>6} {:>7}  {:<8} {}", s.name, s.pid, s.starts, state, s.holds);
                }
            }
            Ok(Err(error)) => failed("list", "", Ok(error)),
            Err(error) => failed("list", "", Err(error)),
        },
        // `run` lifts a quarantine and resets the restart budget, as the shell's RUN <service> does.
        ("start", Some(name)) => match lifecycle::run(LIFECYCLE, name) {
            Ok(pid) => mind::println!("{} started: PID {}", name, pid),
            Err(mind::Error::Other(ERR_BUSY)) => failed("start", name, Ok(lifecycle::Error::Running)),
            Err(mind::Error::NotFound) => mind::println!("svc: start {}: no such service, or its device is missing", name),
            Err(mind::Error::Rights) => mind::println!("svc: start {}: init cannot start it after boot", name),
            Err(error) => failed("start", name, Err(error)),
        },
        ("restart", Some(name)) => match lifecycle::restart(LIFECYCLE, name) {
            Ok(Ok(pid)) => mind::println!("{} restarted: PID {}", name, pid),
            Ok(Err(error)) => failed("restart", name, Ok(error)),
            Err(error) => failed("restart", name, Err(error)),
        },
        ("stop", Some(target)) => {
            let result = match target.parse::<u64>() {
                Ok(pid) => lifecycle::stop_task(LIFECYCLE, pid),
                Err(_) => lifecycle::stop(LIFECYCLE, target),
            };
            match result {
                Ok(Ok(())) => mind::println!("{} stopped", target),
                Ok(Err(error)) => failed("stop", target, Ok(error)),
                Err(error) => failed("stop", target, Err(error)),
            }
        }
        _ => mind::println!("usage: svc [list | start <service> | stop <service or PID> | restart <service>]"),
    }
}
