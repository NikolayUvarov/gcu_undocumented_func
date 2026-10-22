#![no_std]
#![no_main]
//! Test-only stand-in for the Effector gateway (QEMU `modes` suite, 750-KRN-0120, 0121): reports what init's lifecycle
//! client badged BADGE_NAMED lets it do under the owner's policy, and what its writable scope of the delivery root lets it
//! write, at each start. Never packaged into the normal OS; the real gateway is 750-EFF-0007.
use mind::abi::*;
use mind::idl::{init, vfs};
use mind::ipc::Endpoint;

// What init answered a lifecycle request: OK, DENIED, or another outcome.
fn said<T>(answer: mind::sys::Result<Result<T, init::Error>>) -> &'static str {
    match answer { Ok(Ok(_)) => "OK", Ok(Err(init::Error::Denied)) => "DENIED", Ok(Err(_)) => "REFUSED OTHERWISE", Err(_) => "FAILED" }
}

// The lifecycle client against the policy the suite writes: `service sysmon restart` and nothing else.
fn lifecycle() {
    let client = Endpoint(SLOT_LIFECYCLE);
    let run = match init::run(client, "sysmon") { Ok(_) => "OK", Err(mind::sys::Error::Rights) => "DENIED", Err(_) => "REFUSED OTHERWISE" };
    mind::println!("[GATEWAY-STUB] RESTART sysmon {}; STOP sysmon {}; RUN sysmon {}; RESTART logd {}; RESTART init {}; RESTART effector_gw {}; STOP-TASK {}; REBOOT {}",
        said(init::restart(client, "sysmon")), said(init::stop(client, "sysmon")), run, said(init::restart(client, "logd")),
        said(init::restart(client, "init")), said(init::restart(client, "effector_gw")), said(init::stop_task(client, 1)), said(init::reboot(client)));
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

// A file made or replaced at `path` with `data`, flushed.
fn write(files: Endpoint, root: u32, path: &str, data: &[u8]) -> bool {
    let Ok(Ok(file)) = vfs::open(files, root, path, mind::fs::MODE_CREATE | mind::fs::MODE_WRITE | mind::fs::MODE_TRUNCATE) else { return false };
    let written = matches!(vfs::write(files, file, 0, data), Ok(Ok(n)) if n as usize == data.len()) && matches!(vfs::flush(files, file), Ok(Ok(())));
    let _ = vfs::close(files, file);
    written
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    lifecycle();
    scope();
    loop { mind::time::sleep(1000); }
}
