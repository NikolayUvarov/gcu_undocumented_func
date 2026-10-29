#![no_std]
#![no_main]
// svc [list | start <service> | stop <service or PID> | restart <service>]: boot services through init's lifecycle
// interface (idl/init.wit 1.1) — PID, how often init started each, whether it runs or is quarantined, what init gave
// it; start, stop and restart a service, stop an application by PID. A console program: it asks the shell for
// lifecycle control. svc boot | enable | disable | after | reset (173-APP-0092): this boot's plan (`init.wit` 1.10), and
// data/services.txt edited through the user's files, which the shell lends.
use mind::abi::*;
use mind::idl::init as lifecycle;
use mind::ipc::Endpoint;

// init's own reader of data/services.txt (173-KRN-0035): svc judges its edits as init will.
#[path = "../../init/src/services.rs"]
#[allow(dead_code)]
mod services;
mod plan;

mind::request!(REQUEST_CONSOLE | REQUEST_LIFECYCLE | REQUEST_FILES);

const USAGE: &str = "usage: svc [list | start <service> | stop <service or PID> | restart <service> | boot | enable <service> | disable <service> | after <service> <other> | reset]";
const UNITS: usize = BOOT_IMAGES + SERVICE_INSTANCES.len();

const LIFECYCLE: Endpoint = Endpoint(SLOT_LIFECYCLE);

// The loader that started svc answers its launcher (the shell) before it takes another call. One call to it first, so
// that stopping or restarting the loader cannot cut that answer off: the shell would report svc's start as failed
// while svc goes on (000-APP-0003). The call names no session of svc's, so it changes nothing.
fn settle(name: &str) {
    if name == "loader" { let _ = mind::idl::loader::abort(Endpoint::LOADER, 0); }
}

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
    mind::about!("svc — the boot services through init: PID, starts, running or quarantined, what init gave each; which start at boot.\nUsage: svc [list | start <service> | stop <service or PID> | restart <service>]\nsvc boot: this boot's plan: each service's place, whether data/services.txt leaves it on, what it waited for, why it did not start; then what the file changes at the next boot.\nsvc enable|disable <service>, svc after <service> <other>: edit data/services.txt for the next boot, refused where init would refuse (an essential or unknown service, an order against init's); svc reset removes the file.");
    if mind::dev::cap_info(SLOT_LIFECYCLE).0 != CAP_KIND_ENDPOINT { mind::println!("svc: no lifecycle control here (start svc from the shell)"); return; }
    // The user's files (data/ writable) the shell lends for REQUEST_FILES.
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { mind::fs::use_endpoint(Endpoint(SLOT_FILE)); }
    let mut words = mind::process::args_str().split_whitespace();
    let (command, name) = (words.next().unwrap_or("list"), words.next());
    let other = words.next();
    if words.next().is_some() { mind::println!("{}", USAGE); return; }
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
        ("restart", Some(name)) => match { settle(name); lifecycle::restart(LIFECYCLE, name) } {
            Ok(Ok(pid)) => mind::println!("{} restarted: PID {}", name, pid),
            Ok(Err(error)) => failed("restart", name, Ok(error)),
            Err(error) => failed("restart", name, Err(error)),
        },
        ("stop", Some(target)) => {
            let result = match target.parse::<u64>() {
                Ok(pid) => lifecycle::stop_task(LIFECYCLE, pid),
                Err(_) => { settle(target); lifecycle::stop(LIFECYCLE, target) }
            };
            match result {
                Ok(Ok(())) => mind::println!("{} stopped", target),
                Ok(Err(error)) => failed("stop", target, Ok(error)),
                Err(error) => failed("stop", target, Err(error)),
            }
        }
        ("boot", None) => boot(),
        ("enable", Some(name)) if other.is_none() => configure(plan::Change::Enable(name)),
        ("disable", Some(name)) if other.is_none() => configure(plan::Change::Disable(name)),
        ("after", Some(name)) if other.is_some() => configure(plan::Change::After(name, other.unwrap_or(""))),
        ("reset", None) => reset(),
        _ => mind::println!("{}", USAGE),
    }
}

fn units() -> [&'static str; UNITS] { plan::units(&BOOT_SERVICES, &SERVICE_INSTANCES) }

// data/services.txt as init will read it: its first 4 KiB and one byte more, to see a longer one; None when absent.
fn file(text: &mut [u8; services::MAX + 1]) -> Result<Option<usize>, mind::fs::Error> {
    match mind::fs::File::open(services::FILE) {
        Ok(file) => file.read_at(0, text).map(Some),
        Err(mind::fs::Error::NotFound) => Ok(None),
        Err(error) => Err(error),
    }
}

fn slot(slot: u8) -> &'static str {
    match slot as u32 { BOOT_SLOT_A => "A", BOOT_SLOT_B => "B", _ => "the volume's root" }
}

// `svc boot`: how this boot went, its plan, and what data/services.txt as it is now changes at the next boot.
fn boot() {
    let plan = match lifecycle::boot_plan(LIFECYCLE) {
        Ok(Ok(plan)) => plan,
        Ok(Err(error)) => { mind::println!("svc: boot: {}", reason(error)); return; }
        Err(error) => { mind::println!("svc: init did not answer: {:?}", error); return; }
    };
    let configuration = match lifecycle::configuration(LIFECYCLE) {
        Ok(Ok(lifecycle::Configuration::Absent)) => "absent: every service in its order",
        Ok(Ok(lifecycle::Configuration::Read)) => "read",
        Ok(Ok(lifecycle::Configuration::Ignored)) => "ignored: a safe start asked at the bootloader",
        Ok(Ok(lifecycle::Configuration::Unreadable)) => "unreadable: no boot volume when init looked",
        _ => "not answered",
    };
    match lifecycle::boot(LIFECYCLE) {
        Ok(Ok(b)) => mind::println!("boot: slot {}, {}; data/services.txt {}", slot(b.slot),
                                    if !b.trial { "not on trial" } else if b.confirmed { "on trial, confirmed" } else { "on trial, NOT confirmed" }, configuration),
        _ => mind::println!("boot: data/services.txt {}", configuration),
    }
    mind::println!("PLACE SERVICE       FILE       AFTER         OFF AT BOOT");
    for p in plan.as_slice() {
        let file = if p.essential { "essential" } else if p.enabled { "enabled" } else { "disabled" };
        mind::println!("{:>5} {:<13} {:<10} {:<13} {}", p.place, p.name, file, p.after, p.off);
    }
    // The file now against this boot's plan.
    let mut text = [0u8; services::MAX + 1];
    let length = match file(&mut text) { Ok(length) => length.unwrap_or(0), Err(error) => { mind::println!("svc: {}: {:?}", services::FILE, error); return; } };
    let units = units();
    let next = plan::read(&text[..length.min(services::MAX)], &units, |_, _, _| {});
    let mut changes = 0;
    for p in plan.as_slice() {
        let Some(u) = units.iter().position(|u| *u == p.name.as_str()) else { continue };
        let after = next.after[u].map_or("", |a| units[a]);
        if p.essential { continue; }
        if next.off[u] == p.enabled {
            if changes == 0 { mind::println!("at the next boot, by {} as it is now:", services::FILE); }
            mind::println!("  {} {}", p.name, if next.off[u] { "disabled" } else { "enabled" });
            changes += 1;
        }
        if after != p.after.as_str() {
            if changes == 0 { mind::println!("at the next boot, by {} as it is now:", services::FILE); }
            if after.is_empty() { mind::println!("  {} after nothing of the file's", p.name); } else { mind::println!("  {} after {}", p.name, after); }
            changes += 1;
        }
    }
    if changes == 0 { mind::println!("the next boot: as this one ({} has no change for it)", services::FILE); }
}

// `svc enable|disable|after`: the file edited for the next boot, refused where init would refuse.
fn configure(change: plan::Change) {
    let what = match change { plan::Change::Enable(_) => "enable", plan::Change::Disable(_) => "disable", plan::Change::After(..) => "after" };
    let (name, other) = match change { plan::Change::Enable(n) | plan::Change::Disable(n) => (n, ""), plan::Change::After(n, o) => (n, o) };
    let mut text = [0u8; services::MAX + 1];
    let length = match file(&mut text) { Ok(length) => length.unwrap_or(0), Err(error) => { mind::println!("svc: {}: {:?}", services::FILE, error); return; } };
    let units = units();
    let mut out = [0u8; services::MAX];
    let edited = plan::edit(&text[..length], &units, change, &mut out, |line, why, text| {
        mind::println!("svc: {} line {} is one init leaves out: {}: {}", services::FILE, line, why, text);
    });
    let edited = match edited {
        Ok(edited) => edited,
        Err(why) => { mind::println!("svc: {} {}{}{}: refused, as init would refuse it: {}", what, name, if other.is_empty() { "" } else { " " }, other, why); return; }
    };
    if !edited.changed { mind::println!("svc: {} has it already: nothing written", services::FILE); }
    else if let Err(error) = write(&out[..edited.len]) { mind::println!("svc: {}: not written: {:?}", services::FILE, error); return; }
    let positions = || (0..UNITS).filter(move |&p| units[p] == name || services::base(units[p]) == name);
    match change {
        plan::Change::Disable(_) => {
            mind::println!("{} off from the next boot ({}); svc start {} still starts it", name, services::FILE, name);
            // What needs it, through any chain, is off too.
            let mut off = [false; UNITS];
            for p in positions() { off[p] = true; }
            for _ in 0..UNITS { for p in 0..UNITS { if edited.plan.needs[p].is_some_and(|q| off[q]) { off[p] = true; } } }
            for p in (0..UNITS).filter(|&p| off[p] && !positions().any(|q| q == p)) { mind::println!("  {} off too: it needs {}", units[p], name); }
        }
        plan::Change::Enable(_) => match positions().find_map(|p| edited.plan.needs[p]) {
            Some(q) => mind::println!("{} enabled in {}, but off at the next boot: it needs {}, which the file turns off", name, services::FILE, units[q]),
            None => mind::println!("{} on from the next boot ({})", name, services::FILE),
        },
        plan::Change::After(..) => mind::println!("{} starts after {} from the next boot ({})", name, other, services::FILE),
    }
}

// The file written, or removed when nothing is left in it.
fn write(text: &[u8]) -> Result<(), mind::fs::Error> {
    if text.is_empty() {
        return match mind::fs::remove(services::FILE) { Err(mind::fs::Error::NotFound) => Ok(()), result => result };
    }
    let mut file = mind::fs::File::create(services::FILE)?;
    file.write_at(0, text)?;
    file.flush()
}

// `svc reset`: no file, so every service starts in its order at the next boot.
fn reset() {
    match mind::fs::remove(services::FILE) {
        Ok(()) => mind::println!("{} removed: every service starts in its order from the next boot", services::FILE),
        Err(mind::fs::Error::NotFound) => mind::println!("svc: no {}: every service starts in its order already", services::FILE),
        Err(error) => mind::println!("svc: {}: not removed: {:?}", services::FILE, error),
    }
}
