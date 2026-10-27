#![no_std]
#![no_main]
//! Test-only stand-in for the Effector gateway (QEMU `modes` suite, 750-KRN-0120, 0121, 0154): reports this boot's
//! plan (173-KRN-0035), what init's lifecycle client badged BADGE_NAMED lets it do under the owner's policy, what its
//! writable scope of the delivery root lets it write, at each start, what its scope of data/apps/ lets it write
//! (750-KRN-0169), and whether a reboot it asks is served. Never
//! packaged into the normal OS; the real gateway is 750-EFF-0007.
use mind::abi::*;
use mind::idl::{display, init, vfs};
use mind::ipc::Endpoint;

// What init answered a lifecycle request: OK, DENIED, or another outcome.
fn said<T>(answer: mind::sys::Result<Result<T, init::Error>>) -> &'static str {
    match answer { Ok(Ok(_)) => "OK", Ok(Err(init::Error::Denied)) => "DENIED", Ok(Err(_)) => "REFUSED OTHERWISE", Err(_) => "FAILED" }
}

// The lifecycle client against the policy the suite writes: `service sysmon restart` and nothing else.
fn lifecycle() {
    let client = Endpoint(SLOT_LIFECYCLE);
    let run = match init::run(client, "sysmon") { Ok(_) => "OK", Err(mind::sys::Error::Rights) => "DENIED", Err(_) => "REFUSED OTHERWISE" };
    mind::println!("[GATEWAY-STUB] RESTART sysmon {}; STOP sysmon {}; RUN sysmon {}; RESTART logd {}; RESTART init {}; RESTART effector_gw {}; STOP-TASK {}",
        said(init::restart(client, "sysmon")), said(init::stop(client, "sysmon")), run, said(init::restart(client, "logd")),
        said(init::restart(client, "init")), said(init::restart(client, "effector_gw")), said(init::stop_task(client, 1)));
}

// A reboot asked once: when the suite left `reboot-now` in the delivery root, removed first so the next start does not ask.
fn reboot() {
    let files = Endpoint(SLOT_FILE);
    let asked = match vfs::root(files, "") {
        Ok(Ok(root)) => { let removed = matches!(vfs::remove(files, root, "reboot-now"), Ok(Ok(()))); let _ = vfs::close(files, root); removed }
        _ => false,
    };
    if !asked { mind::println!("[GATEWAY-STUB] REBOOT NOT ASKED"); return; }
    mind::println!("[GATEWAY-STUB] REBOOT ASKED");
    mind::println!("[GATEWAY-STUB] REBOOT {}", said(init::reboot(Endpoint(SLOT_LIFECYCLE))));
}

// The delivery scope: this start's number kept in `starts`, a file written, flushed and renamed in the scope, and a name
// or a rename that climbs out refused.
fn scope() {
    let files = Endpoint(SLOT_FILE);
    let Ok(Ok(root)) = vfs::root(files, "") else { mind::println!("[GATEWAY-STUB] NO SCOPE"); return };
    let mut count = [0u8; 8];
    let previous = match vfs::open(files, root, "starts", 0) {
        Ok(Ok(file)) => { let n = vfs::read(files, file, 0, 8, &mut count).ok().and_then(|r| r.ok()).unwrap_or(0); let _ = vfs::close(files, file); core::str::from_utf8(&count[..n]).ok().and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0) }
        _ => 0,
    };
    let start = previous + 1;
    let mut text = mind::util::FixedBuf::<8>::new();
    let _ = core::fmt::Write::write_fmt(&mut text, format_args!("{}", start));
    let kept = write(files, root, "starts", text.as_bytes());
    let written = write(files, root, "parcel.part", b"delivered");
    // A file is renamed onto a name that is free: the one the start before left goes first, as the gateway does.
    let _ = vfs::remove(files, root, "parcel.txt");
    let renamed = matches!(vfs::rename(files, root, "parcel.part", root, "parcel.txt"), Ok(Ok(())));
    let mut back = [0u8; 16];
    let read = match vfs::open(files, root, "parcel.txt", 0) {
        Ok(Ok(file)) => { let n = vfs::read(files, file, 0, 16, &mut back).ok().and_then(|r| r.ok()).unwrap_or(0); let _ = vfs::close(files, file); &back[..n] == b"delivered" }
        _ => false,
    };
    let outside = !matches!(vfs::open(files, root, "../escape.txt", mind::fs::MODE_CREATE | mind::fs::MODE_WRITE), Ok(Ok(_)));
    let out = !matches!(vfs::rename(files, root, "parcel.txt", root, "../parcel.txt"), Ok(Ok(())));
    let ok = |b: bool| if b { "OK" } else { "FAILED" };
    mind::println!("[GATEWAY-STUB] SCOPE START {}: KEPT {}, WRITE {}, RENAME {}, READ BACK {}, OUTSIDE {}, RENAME OUT {}", start,
        ok(kept), ok(written), ok(renamed), ok(read), if outside { "REFUSED" } else { "ALLOWED" }, if out { "REFUSED" } else { "ALLOWED" });
}

// The scope of data/apps/ (750-KRN-0169): a file written there and read back, then removed; one outside refused, the
// owner's policy beside it too.
fn apps() {
    let files = Endpoint(SLOT_APPS);
    let Ok(Ok(root)) = vfs::root(files, "") else { mind::println!("[GATEWAY-STUB] NO APPS SCOPE"); return };
    let written = write(files, root, "stub-probe", b"installed");
    let mut back = [0u8; 16];
    let read = match vfs::open(files, root, "stub-probe", 0) {
        Ok(Ok(file)) => { let n = vfs::read(files, file, 0, 16, &mut back).ok().and_then(|r| r.ok()).unwrap_or(0); let _ = vfs::close(files, file); &back[..n] == b"installed" }
        _ => false,
    };
    let removed = matches!(vfs::remove(files, root, "stub-probe"), Ok(Ok(())));
    let outside = !write(files, root, "../escape.txt", b"x") && !write(files, root, "../effector/policy", b"x");
    let _ = vfs::close(files, root);
    let ok = |b: bool| if b { "OK" } else { "FAILED" };
    mind::println!("[GATEWAY-STUB] APPS SCOPE: WRITE {}, READ BACK {}, REMOVE {}, OUTSIDE {}", ok(written), ok(read), ok(removed),
        if outside { "REFUSED" } else { "ALLOWED" });
}

// A file made or replaced at `path` with `data`, flushed.
fn write(files: Endpoint, root: u32, path: &str, data: &[u8]) -> bool {
    let Ok(Ok(file)) = vfs::open(files, root, path, mind::fs::MODE_CREATE | mind::fs::MODE_WRITE | mind::fs::MODE_TRUNCATE) else { return false };
    let written = matches!(vfs::write(files, file, 0, data), Ok(Ok(n)) if n as usize == data.len()) && matches!(vfs::flush(files, file), Ok(Ok(())));
    let _ = vfs::close(files, file);
    written
}

// The Effector agent's mark (800-KRN-0156) through the screen's client, badged for it: each state for 6 s, a call a
// second, then no more calls, so the mark goes 3 s later.
fn remote_mark() {
    let screen = Endpoint(SLOT_DISPLAY);
    for (state, name) in [(display::RemoteState::Changing, "CHANGING"), (display::RemoteState::Reading, "READING"), (display::RemoteState::Unconnected, "UNCONNECTED")] {
        let answer = display::remote(screen, state);
        mind::println!("[GATEWAY-STUB] REMOTE MARK {} {}", name, if matches!(answer, Ok(Ok(()))) { "OK" } else { "FAILED" });
        for _ in 0..6 { mind::time::sleep(1000); let _ = display::remote(screen, state); }
    }
    mind::println!("[GATEWAY-STUB] REMOTE MARK STOPPED");
}

// This boot's plan (173-KRN-0035): a line a service, in the order init tried them, then where it came from; any
// client of init may ask.
fn boot_plan() {
    match init::boot_plan(Endpoint(SLOT_LIFECYCLE)) {
        Ok(Ok(plan)) => for s in plan.as_slice() {
            mind::println!("[GATEWAY-STUB] PLAN {} {} {} {} AFTER '{}' OFF '{}'", s.place, s.name.as_str(), if s.enabled { "ENABLED" } else { "DISABLED" },
                           if s.essential { "ESSENTIAL" } else { "CONFIGURABLE" }, s.after.as_str(), s.off.as_str());
        },
        _ => mind::println!("[GATEWAY-STUB] PLAN FAILED"),
    }
    // Where the plan came from (173-KRN-0036).
    let source = match init::configuration(Endpoint(SLOT_LIFECYCLE)) {
        Ok(Ok(init::Configuration::Absent)) => "ABSENT", Ok(Ok(init::Configuration::Read)) => "READ",
        Ok(Ok(init::Configuration::Ignored)) => "IGNORED", Ok(Ok(init::Configuration::Unreadable)) => "UNREADABLE", _ => "FAILED",
    };
    mind::println!("[GATEWAY-STUB] PLAN END, CONFIGURATION {}", source);
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    boot_plan();
    // Whether the link's console is on (800-KRN-0155): any client of init may ask.
    let console = match init::link_console(Endpoint(SLOT_LIFECYCLE)) { Ok(Ok(true)) => "ON", Ok(Ok(false)) => "OFF", _ => "FAILED" };
    mind::println!("[GATEWAY-STUB] LINK CONSOLE {}", console);
    lifecycle();
    scope();
    apps();
    reboot();
    remote_mark();
    loop { mind::time::sleep(1000); }
}
