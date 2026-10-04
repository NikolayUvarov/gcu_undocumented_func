//! `reboot [-f]` (issue 084): writes what `vfs_server` caches to the disks, stops the boot services in reverse start
//! order through `init`'s lifecycle requests (so drivers quiesce their devices), then resets the machine (`REBOOT`,
//! kernel issue 152). `-f` skips stopping the services.
use crate::console::Console;
use crate::files;
use core::fmt::Write;
use mind::control;
use mind::idl::init as idl_init;
use mind::ipc::Endpoint;

/// Services that stay: init, which cannot be stopped, and the shell, which is doing this.
const KEEP: [&str; 2] = ["init", "shell"];

pub fn reboot(out: &mut Console, args: &[u8]) {
    let force = match args.trim_ascii() { b"" => false, b"-f" | b"-F" => true, _ => { let _ = writeln!(out, "USAGE: REBOOT [-F]"); return; } };
    files::flush_all();
    if !force {
        match idl_init::list(Endpoint::INIT) {
            Ok(Ok(services)) => {
                for service in services.as_slice().iter().rev().filter(|s| s.running && !KEEP.contains(&s.name.as_str())) {
                    match idl_init::stop(Endpoint::INIT, service.name.as_str()) {
                        Ok(Ok(())) => { let _ = writeln!(out, "STOPPED {}", service.name.as_str()); }
                        Ok(Err(error)) => { let _ = writeln!(out, "STOP {}: {:?}", service.name.as_str(), error); }
                        Err(error) => { let _ = writeln!(out, "STOP {}: INIT DID NOT ANSWER ({:?})", service.name.as_str(), error); }
                    }
                }
            }
            _ => { let _ = writeln!(out, "INIT DID NOT ANSWER: THE SERVICES ARE NOT STOPPED"); }
        }
    }
    let _ = writeln!(out, "REBOOTING...");
    if control::reboot().is_err() { let _ = writeln!(out, "ERROR: REBOOT REFUSED"); }
}
