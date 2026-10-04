#![no_std]
#![no_main]
// svc [list | start <service> | stop <service or PID> | restart <service>]: boot services through init's lifecycle
// interface (idl/lifecycle.wit) — PID, how often init started each, whether it runs, what init gave it; start, stop
// and restart a service, stop an application by PID. A console program: it asks the shell for lifecycle control.
use mind::abi::*;
use mind::idl::{lifecycle, wire};
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
    if mind::dev::cap_info(SLOT_LIFECYCLE).0 != CAP_KIND_ENDPOINT { mind::println!("svc: no lifecycle control here (start svc from the shell)"); return; }
    let Ok(mut shared) = wire::Shared::new(8192) else { mind::println!("svc: no memory"); return };
    let mut words = mind::process::args_str().split_whitespace();
    let (command, name) = (words.next().unwrap_or("list"), words.next());
    let failed = |what: &str, name: &str, error: Result<lifecycle::Error, mind::Error>| match error {
        Ok(error) => mind::println!("svc: {} {}: {}", what, name, reason(error)),
        Err(error) => mind::println!("svc: init did not answer: {:?}", error),
    };
    match (command, name) {
        ("list", None) => match lifecycle::list(LIFECYCLE, shared.buffer()) {
            Ok(Ok(services)) => {
                mind::println!("SERVICE         PID  STARTS  STATE    HOLDS");
                for s in services.iter() {
                    let pid = if s.running { s.pid } else { 0 };
                    mind::println!("{:<12} {:>6} {:>7}  {:<8} {}", s.name, pid, s.starts, if s.running { "running" } else { "stopped" }, s.holds);
                }
            }
            Ok(Err(error)) => failed("list", "", Ok(error)),
            Err(error) => failed("list", "", Err(error)),
        },
        ("start" | "restart", Some(name)) => {
            let result = if command == "start" { lifecycle::start(LIFECYCLE, shared.buffer(), name) } else { lifecycle::restart(LIFECYCLE, shared.buffer(), name) };
            match result {
                Ok(Ok(pid)) => mind::println!("{} {}: PID {}", name, if command == "start" { "started" } else { "restarted" }, pid),
                Ok(Err(error)) => failed(command, name, Ok(error)),
                Err(error) => failed(command, name, Err(error)),
            }
        }
        ("stop", Some(target)) => {
            let result = match target.parse::<u64>() {
                Ok(pid) => lifecycle::stop_task(LIFECYCLE, pid),
                Err(_) => lifecycle::stop(LIFECYCLE, shared.buffer(), target),
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
