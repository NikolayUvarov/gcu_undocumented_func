//! `reboot [-f] [--off]` (issue 084) and `poweroff [-f]`, `shutdown [-f]` (000-APP-0065): writes what `vfs_server`
//! caches to the disks, stops the boot services in reverse start order through `init`'s lifecycle requests (so drivers
//! quiesce their devices), then resets the machine (`REBOOT`, kernel issue 152), or turns it off: ACPI S5 on x86
//! (000-KRN-0080), PSCI on aarch64 (issue 203). `-f` skips stopping the services.
use crate::console::Console;
use crate::files;
use core::fmt::Write;
use mind::control;
use mind::idl::init as idl_init;
use mind::ipc::Endpoint;

/// Services that stay: init, which cannot be stopped, and the shell, which is doing this.
const KEEP: [&str; 2] = ["init", "shell"];

pub fn reboot(out: &mut Console, args: &[u8]) {
    let (mut force, mut off) = (false, false);
    for word in args.split(|b| *b == b' ').filter(|w| !w.is_empty()) {
        match word { b"-f" | b"-F" => force = true, b"--off" | b"--OFF" => off = true, _ => { let _ = writeln!(out, "USAGE: REBOOT [-F] [--OFF]"); return; } }
    }
    down(out, force, off);
}

/// `poweroff [-f]` and `shutdown [-f]`: `reboot --off` by the names people look for.
pub fn poweroff(out: &mut Console, args: &[u8]) {
    let mut force = false;
    for word in args.split(|b| *b == b' ').filter(|w| !w.is_empty()) {
        match word { b"-f" | b"-F" => force = true, _ => { let _ = writeln!(out, "USAGE: POWEROFF [-F] (OR SHUTDOWN [-F])"); return; } }
    }
    down(out, force, true);
}

fn down(out: &mut Console, force: bool, off: bool) {
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
    if off {
        let _ = writeln!(out, "POWERING OFF...");
        match control::power_off() {
            Ok(()) => {}
            Err(mind::sys::Error::Invalid) => { let _ = writeln!(out, "ERROR: POWER OFF REFUSED: THE FIRMWARE NAMES NO WAY TO TURN THIS MACHINE OFF (NO ACPI S5); REBOOT RESTARTS IT"); }
            Err(_) => { let _ = writeln!(out, "ERROR: POWER OFF REFUSED"); }
        }
        return;
    }
    let _ = writeln!(out, "REBOOTING...");
    if control::reboot().is_err() { let _ = writeln!(out, "ERROR: REBOOT REFUSED"); }
}
