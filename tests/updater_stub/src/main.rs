#![no_std]
#![no_main]
//! Test-only stand-in for the updater service (QEMU `updater` suite, 351-KRN-0022): reports the slots init filled and
//! what each authority does, tries the update zone when the system booted from a slot, then waits for the directory
//! data/reboot on the boot volume and asks init to restart the machine. It answers `update.wit` meanwhile (`serve`).
//! In effector_gw's place it stands in for the Effector gateway instead (`gateway`, 751-UPD-0032). Never packaged into
//! the normal OS; the real updater is 351-UPD-0007.
extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use mind::abi::*;
use mind::idl::update::{self as api, self as update, Error, Metadata, State};
use mind::idl::{init, vfs, wire};
use mind::ipc::{Endpoint, Received};
use mind::sha256::Sha256;

// What `install-commit` answers as staged: the stand-in stages nothing.
const STAGED: u64 = 1;
const ASK_MS: u32 = 1000;
const RECEIVED: usize = 9; // the buffer of an install call lands here

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

// In effector_gw's place, which alone holds the updater's client badged for the gateway in SLOT_UPDATE: each line of
// system/effector_gw/install.txt, `FILE SHA256 [short|apply]`, is a bundle of that directory handed to the updater with
// the SHA-256 named: in 32 KiB pieces, the second written twice as a resumed transfer would; `short` stops after the
// first piece; `apply` asks the updater to apply once the directory data/apply exists. Booted from slot B, nothing.
fn gateway(info: &BootInfo) -> ! {
    const DIR: &str = "system/effector_gw";
    const PIECE: usize = 32 * 1024;
    let updater = Endpoint(SLOT_UPDATE);
    let list = mind::fs::File::open(&format!("{}/install.txt", DIR)).ok().and_then(|f| {
        let mut text = alloc::vec![0u8; f.size().min(4096)];
        (f.read_at(0, &mut text) == Ok(text.len())).then(|| String::from_utf8_lossy(&text).into_owned())
    });
    match (info.boot_slot.slot, list) {
        (BOOT_SLOT_B, _) => mind::println!("[GATEWAY-STUB] BOOTED FROM SLOT B: NOTHING TO HAND OVER"),
        (_, None) => mind::println!("[GATEWAY-STUB] NO {}/install.txt", DIR),
        (_, Some(list)) => for (index, line) in list.lines().filter(|l| !l.trim().is_empty()).enumerate() {
            let mut words = line.split_whitespace();
            let (Some(name), Some(sha256)) = (words.next(), words.next()) else { continue };
            let what = words.next().unwrap_or("");
            let command = format!("test-{}", index + 1);
            let Ok(file) = mind::fs::File::open(&format!("{}/{}", DIR, name)) else { mind::println!("[GATEWAY-STUB] {}: NOT FOUND", name); continue };
            let size = file.size();
            let begun = update::install_begin(updater, size as u64, sha256, &command);
            let handle = match begun { Ok(Ok(handle)) => handle, _ => 1 };
            let mut written: Result<usize, Error> = Ok(0);
            if matches!(begun, Ok(Ok(_))) {
                let mut piece = alloc::vec![0u8; PIECE];
                let mut offsets = (0..size).step_by(PIECE).collect::<alloc::vec::Vec<_>>();
                if what == "short" { offsets.truncate(1); } else if offsets.len() > 2 { offsets.insert(2, offsets[1]); }
                for offset in offsets {
                    let got = file.read_at(offset, &mut piece[..PIECE.min(size - offset)]).unwrap_or(0);
                    match update::install_write(updater, handle, offset as u64, &piece[..got]) {
                        Ok(Ok(())) => written = written.map(|n| n + got),
                        Ok(Err(error)) => { written = Err(error); break; }
                        Err(_) => { written = Err(Error::Denied); break; }
                    }
                }
            }
            let committed = update::install_commit(updater, handle);
            mind::println!("[GATEWAY-STUB] {}: {} BYTES, BEGIN {:?}, WRITTEN {:?}, COMMIT {:?}", name, size, begun, written, committed);
            if what == "apply" && matches!(committed, Ok(Ok(_))) {
                mind::println!("[GATEWAY-STUB] {}: APPLY WAITS FOR data/apply", name);
                while mind::fs::Dir::open("data/apply").is_err() { mind::time::sleep(200); }
                mind::println!("[GATEWAY-STUB] {}: APPLY {:?}", name, update::apply(updater, 3));
            }
        },
    }
    mind::println!("[GATEWAY-STUB] DONE");
    loop { mind::time::sleep(1000); }
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    if kind(SLOT_UPDATE) == CAP_KIND_ENDPOINT { gateway(info); }
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
    if let Some(root) = root { zone(files, root, info); }
    let mut stub = Stub { install: None, handles: 0, staged: 0 };
    let mut scratch: Box<[u8; api::REQUEST_MAX]> = alloc::vec![0u8; api::REQUEST_MAX].into_boxed_slice().try_into().unwrap();
    let mut rebooting = false;
    // It answers update.wit only in the updater's place, which alone holds the firmware privilege: in another service's
    // place (the policy broker's, in the updater suite) it receives nothing, as a service that never answers.
    let updater = kind(SLOT_FIRMWARE) == CAP_KIND_FIRMWARE;
    loop {
        if let Some(dir) = root.filter(|_| !rebooting).and_then(|root| vfs::open_dir(files, root, "data/reboot", false).ok()?.ok()) {
            let _ = vfs::close(files, dir);
            rebooting = true;
            restart();
        }
        if !updater { mind::time::sleep(200); continue; }
        if let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, 200) { stub.serve(&request, &mut scratch); }
    }
}

fn restart() {
    mind::println!("[UPDATER-STUB] ASKING INIT TO RESTART THE MACHINE");
    let answer = init::reboot(Endpoint(SLOT_LIFECYCLE));
    mind::println!("[UPDATER-STUB] REBOOT ANSWERED {:?}", answer);
}

fn hex(text: &str) -> Option<[u8; 32]> {
    let digit = |c: u8| (c as char).to_digit(16);
    let mut out = [0u8; 32];
    if text.len() != 64 { return None; }
    for (i, pair) in text.as_bytes().chunks_exact(2).enumerate() { out[i] = (digit(pair[0])? * 16 + digit(pair[1])?) as u8; }
    Some(out)
}

// A bundle begun: hashed as it comes, kept nowhere.
struct Install { handle: u32, size: u64, sha256: [u8; 32], command: mind::idl::codec::Text<64>, written: u64, hash: Sha256 }

struct Stub { install: Option<Install>, handles: u32, staged: u64 }

impl Stub {
    // As the updater (751-UPD-0032): `install-…` from BADGE_UPDATE_GATEWAY only, at begin and commit while init's
    // remote-update counts; `apply` asks init to restart the machine. For the gateway's own tests (751-EFF-0023).
    fn allowed(request: &Received, what: &str) -> Result<(), Error> {
        let why = if request.badge != mind::process::BADGE_UPDATE_GATEWAY {
            "ANOTHER BADGE"
        } else {
            match wire::with_timeout(ASK_MS, || init::mode(Endpoint(SLOT_LIFECYCLE))) {
                Ok(Ok(modes)) if modes.remote_update => return Ok(()),
                Ok(Ok(_)) => "REMOTE-UPDATE IS OFF",
                _ => "INIT DID NOT SAY",
            }
        };
        mind::println!("[UPDATER-STUB] {} REFUSED (BADGE {}, PID {}): {}", what, request.badge, request.sender, why);
        Err(Error::Denied)
    }

    fn begin(&mut self, request: &Received, size: u64, sha256: &str, command: mind::idl::codec::Text<64>) -> Result<u32, Error> {
        Self::allowed(request, "INSTALL-BEGIN")?;
        let sha256 = hex(sha256).ok_or(Error::Malformed)?;
        self.handles = self.handles.wrapping_add(1).max(1);
        mind::println!("[UPDATER-STUB] INSTALL-BEGIN: {} BYTES, COMMAND {}: HANDLE {}{}", size, command.as_str(), self.handles,
                       if self.install.is_some() { " (THE BUNDLE BEGUN BEFORE DROPPED)" } else { "" });
        self.install = Some(Install { handle: self.handles, size, sha256, command, written: 0, hash: Sha256::new() });
        Ok(self.handles)
    }

    // The stand-in keeps no bytes, so a write resumes only where the last ended, or from the start.
    fn write(&mut self, handle: u32, offset: u64, bytes: &[u8]) -> Result<(), Error> {
        let install = self.install.as_mut().filter(|i| i.handle == handle).ok_or(Error::Malformed)?;
        if offset == 0 && install.written > 0 { install.hash = Sha256::new(); install.written = 0; }
        if offset != install.written || offset + bytes.len() as u64 > install.size {
            mind::println!("[UPDATER-STUB] INSTALL-WRITE AT {} OF {}: THE STAND-IN TAKES BYTES ONLY AT {} OR 0", offset, install.size, install.written);
            return Err(Error::Malformed);
        }
        install.hash.update(bytes);
        install.written += bytes.len() as u64;
        Ok(())
    }

    fn commit(&mut self, request: &Received, handle: u32) -> Result<u64, Error> {
        if !self.install.as_ref().is_some_and(|i| i.handle == handle) { return Err(Error::Malformed); }
        Self::allowed(request, "INSTALL-COMMIT")?;
        let install = self.install.take().ok_or(Error::Malformed)?;
        let whole = install.written == install.size && install.hash.finish() == install.sha256;
        mind::println!("[UPDATER-STUB] INSTALL-COMMIT: {} OF {} BYTES, COMMAND {}: {}", install.written, install.size, install.command.as_str(),
                       if whole { "SHA-256 AS BEGUN; VERSION 1 ANSWERED, NOTHING STAGED (THE STAND-IN)" } else { "REFUSED: Digest" });
        if !whole { return Err(Error::Digest); }
        self.staged = STAGED;
        Ok(STAGED)
    }

    fn apply(&mut self, request: &Received, tries: u8) -> Result<(), Error> {
        if request.badge == mind::process::BADGE_UPDATE_GATEWAY { Self::allowed(request, "APPLY")?; }
        if self.staged == 0 { return Err(Error::NotStaged); }
        mind::println!("[UPDATER-STUB] APPLY WITH {} TRIES", tries);
        restart();
        Ok(())
    }

    fn serve(&mut self, request: &Received, scratch: &mut [u8; api::REQUEST_MAX]) {
        let _ = match api::decode(request, RECEIVED, scratch) {
            Ok((api::Request::Check, call)) => api::reply_check(call, Err(Error::NoSource)),
            Ok((api::Request::Fetch, call)) => api::reply_fetch(call, Err(Error::NoSource)),
            Ok((api::Request::Apply { tries }, call)) => { let applied = self.apply(request, tries); api::reply_apply(call, applied) }
            Ok((api::Request::Rollback, call)) => api::reply_rollback(call, Err(Error::NoFallback)),
            Ok((api::Request::Status, call)) => api::reply_status(call, &State { staged: self.staged, ..State::default() }),
            Ok((api::Request::Metadata, call)) => api::reply_metadata(call, &Metadata::default()),
            Ok((api::Request::InstallBegin { size, sha256, command }, call)) => { let begun = self.begin(request, size, sha256.as_str(), command); api::reply_install_begin(call, begun) }
            Ok((api::Request::InstallWrite { handle, offset, bytes }, call)) => { let written = self.write(handle, offset, bytes); api::reply_install_write(call, written) }
            Ok((api::Request::InstallCommit { handle }, call)) => { let committed = self.commit(request, handle); api::reply_install_commit(call, committed) }
            Err(reason) if request.is_call => wire::reject(reason),
            Err(_) => Ok(()),
        };
    }
}
