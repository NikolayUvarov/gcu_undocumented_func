#![no_std]
#![no_main]
//! Test-only stand-in for the updater service (QEMU `updater` suite, 351-KRN-0022): reports the slots init filled and
//! what each authority does, then waits for the directory data/reboot on the boot volume and asks init to restart the
//! machine. Never packaged into the normal OS; the real updater is 351-UPD-0007.
use mind::abi::*;
use mind::idl::{init, vfs};
use mind::ipc::Endpoint;

fn kind(slot: usize) -> usize { let kind = mind::dev::cap_info(slot).0; if kind > 64 { CAP_KIND_NONE } else { kind } }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut held = mind::util::FixedBuf::<256>::new();
    for slot in 1..SLOT_DYNAMIC {
        if kind(slot) != CAP_KIND_NONE { let _ = core::fmt::Write::write_fmt(&mut held, format_args!(" {}:{}", slot, kind(slot))); }
    }
    mind::println!("[UPDATER-STUB] HOLDS{}", core::str::from_utf8(held.as_bytes()).unwrap_or("?"));
    // The firmware privilege reads a variable every firmware has.
    let mut order = [0u8; 64];
    let firmware = match mind::firmware::get("BootOrder", &mind::firmware::GLOBAL, &mut order) { Ok(_) => "READ", Err(_) => "REFUSED" };
    // The file system client reads the boot volume and may not write to it.
    let files = Endpoint(SLOT_VFS);
    let root = vfs::root(files, "").ok().and_then(|r| r.ok());
    let write = root.map(|root| match vfs::open(files, root, "updater.txt", mind::fs::MODE_CREATE | mind::fs::MODE_WRITE) {
        Ok(Ok(_)) => "WRITTEN", Ok(Err(vfs::Error::Denied)) => "DENIED", Ok(Err(_)) => "REFUSED OTHERWISE", Err(_) => "FAILED",
    });
    mind::println!("[UPDATER-STUB] FIRMWARE {} VFS {} WRITE {}", firmware, if root.is_some() { "READ" } else { "REFUSED" }, write.unwrap_or("-"));
    let Some(root) = root else { loop { mind::time::sleep(1000); } };
    loop {
        if let Ok(Ok(dir)) = vfs::open_dir(files, root, "data/reboot", false) {
            let _ = vfs::close(files, dir);
            mind::println!("[UPDATER-STUB] ASKING INIT TO RESTART THE MACHINE");
            let answer = init::reboot(Endpoint(SLOT_LIFECYCLE));
            mind::println!("[UPDATER-STUB] REBOOT ANSWERED {:?}", answer);
            loop { mind::time::sleep(1000); }
        }
        mind::time::sleep(200);
    }
}
