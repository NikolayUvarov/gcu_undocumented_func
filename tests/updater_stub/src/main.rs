#![no_std]
#![no_main]
//! Test-only stand-in for the updater service (QEMU `updater` suite, 351-KRN-0022): reports the slots init filled and
//! what each authority does, tries the update zone when the system booted from a slot, then waits for the directory
//! data/reboot on the boot volume and asks init to restart the machine. Never packaged into the normal OS; the real
//! updater is 351-UPD-0007.
use mind::abi::*;
use mind::idl::{init, vfs};
use mind::ipc::Endpoint;

fn kind(slot: usize) -> usize { let kind = mind::dev::cap_info(slot).0; if kind > 64 { CAP_KIND_NONE } else { kind } }

// What vfs_server answered an open and a write: WRITTEN, or the error.
fn outcome(answer: mind::sys::Result<Result<u32, vfs::Error>>) -> &'static str {
    match answer { Ok(Ok(_)) => "WRITTEN", Ok(Err(vfs::Error::Denied)) => "DENIED", Ok(Err(vfs::Error::Invalid)) => "INVALID", Ok(Err(vfs::Error::ReadOnly)) => "READ-ONLY", Ok(Err(_)) => "REFUSED OTHERWISE", Err(_) => "FAILED" }
}

// A file made at `path` and `data` written to it.
fn create(files: Endpoint, root: u32, path: &str, data: &[u8]) -> &'static str {
    match vfs::open(files, root, path, mind::fs::MODE_CREATE | mind::fs::MODE_WRITE | mind::fs::MODE_TRUNCATE) {
        Ok(Ok(file)) => { let wrote = vfs::write(files, file, 0, data); let _ = vfs::close(files, file); outcome(wrote) }
        other => outcome(other),
    }
}

// On a slot volume (351-UPD-0008): the slot that did not boot is the badge's to fill, a boot record is written only
// whole and in place, and the running slot, EFI and MIND itself stay read-only. The record is written with its own bytes.
fn zone(files: Endpoint, root: u32, info: &BootInfo) {
    let ((running, live), (inactive, staged)) = match info.boot_slot.slot {
        BOOT_SLOT_A => (("A", "MIND/A/STUB.TXT"), ("B", "MIND/B/STUB.TXT")),
        BOOT_SLOT_B => (("B", "MIND/B/STUB.TXT"), ("A", "MIND/A/STUB.TXT")),
        _ => return,
    };
    let staged = create(files, root, staged, b"STAGED BY THE UPDATER'S STAND-IN\n");
    let mut record = [0u8; 512];
    let (whole, part) = match vfs::open(files, root, "MIND/BOOT0", mind::fs::MODE_WRITE) {
        Ok(Ok(file)) => {
            let read = vfs::read(files, file, 0, 512, &mut record);
            let whole = if matches!(read, Ok(Ok(512))) { outcome(vfs::write(files, file, 0, &record)) } else { "UNREAD" };
            let part = outcome(vfs::write(files, file, 0, &record[..100]));
            let _ = vfs::close(files, file);
            (whole, part)
        }
        other => (outcome(other), "-"),
    };
    let made = outcome(vfs::open(files, root, "MIND/BOOT0", mind::fs::MODE_CREATE | mind::fs::MODE_WRITE | mind::fs::MODE_TRUNCATE));
    let live = create(files, root, live, b"-");
    let efi = create(files, root, "EFI/STUB.TXT", b"-");
    let mind = create(files, root, "MIND/STUB.TXT", b"-");
    mind::println!("[UPDATER-STUB] ZONE MIND/{}: STUB.TXT {}; MIND/BOOT0 WHOLE {}, A PART {}, MADE AGAIN {}; MIND/{} {}; EFI {}; MIND {}",
        inactive, staged, whole, part, made, running, live, efi, mind);
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
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
    zone(files, root, info);
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
