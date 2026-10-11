// The hardware report (174-KRN-0038): the kernel's text goes to log:hwNNNN.txt beside this boot's bootNNNN.log, the
// ACPI tables to log:acpi/<SIG>.bin. Without a log volume nothing is written: ram: and data/ are the user's. Run once at
// boot, while init holds the platform privilege that the report and the tables need.
use mind::abi::*;
use mind::fs::{MODE_CREATE, MODE_TRUNCATE, MODE_WRITE};
use mind::idl::vfs;
use mind::ipc::Endpoint;
use mind::mem::Mapping;
use mind::platform;
use mind::util::FixedBuf;

const KEEP: u32 = 50; // as many reports as vfs_server keeps boot logs

// The number in `prefix`NNNN`suffix`, any case.
fn number(name: &[u8], prefix: &[u8], suffix: &[u8]) -> Option<u32> {
    if name.len() != prefix.len() + 4 + suffix.len() || !name[..prefix.len()].eq_ignore_ascii_case(prefix) || !name[prefix.len() + 4..].eq_ignore_ascii_case(suffix) { return None; }
    core::str::from_utf8(&name[prefix.len()..prefix.len() + 4]).ok()?.parse().ok()
}

// This boot's log number (the highest bootNNNN.log, which vfs_server made before it served anyone); reports older than
// KEEP boots are removed.
fn this_boot(files: Endpoint, root: u32) -> u32 {
    let (mut this, mut start) = (0, 0);
    let mut old = [0u32; 16]; let mut old_count = 0;
    loop {
        let Ok(Ok(list)) = vfs::list(files, root, start) else { break };
        for entry in list.as_slice() {
            let name = entry.name.as_str().as_bytes();
            if let Some(n) = number(name, b"boot", b".log") { this = this.max(n); }
            if let Some(n) = number(name, b"hw", b".txt") { if old_count < old.len() { old[old_count] = n; old_count += 1; } }
        }
        if list.len() < 16 { break; }
        start += list.len() as u32;
    }
    for &n in &old[..old_count] {
        if n + KEEP <= this { let mut name = FixedBuf::<16>::new(); let _ = core::fmt::Write::write_fmt(&mut name, format_args!("hw{:04}.txt", n)); let _ = vfs::remove(files, root, name.as_str()); }
    }
    this
}

fn write(files: Endpoint, dir: u32, name: &str, data: &[u8]) -> bool {
    let Ok(Ok(file)) = vfs::open(files, dir, name, MODE_CREATE | MODE_WRITE | MODE_TRUNCATE) else { return false };
    let mut written = true;
    for (k, chunk) in data.chunks(16384).enumerate() {
        if !matches!(vfs::write(files, file, (k * 16384) as u32, chunk), Ok(Ok(_))) { written = false; break; }
    }
    let _ = vfs::close(files, file);
    written
}

// The kernel's memory object of `kind`, mapped, with its slot (dropped by `done` once the mapping is gone).
fn object(kind: usize, a: usize) -> Option<(usize, Mapping)> {
    let slot = platform::cap(kind, a, 0).ok()?;
    match Mapping::new(slot) { Ok(map) => Some((slot, map)), Err(_) => { let _ = mind::ipc::drop_cap(slot); None } }
}
fn done((slot, map): (usize, Mapping)) { drop(map); let _ = mind::ipc::drop_cap(slot); }

/// Writes the report and the ACPI tables through `vfs_server`'s keeper (a client with the user's badge, for log:).
pub fn report(keeper: usize) {
    let Ok(client) = mind::ipc::mint_badged(keeper, crate::CLIENT, mind::fs::BADGE_USER) else { return };
    let files = Endpoint(client);
    if let Some(text) = object(PLATFORM_REPORT, 0) {
        let bytes = text.1.as_slice();
        let bytes = &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())];
        let mut name = FixedBuf::<16>::new();
        match vfs::root(files, "log") {
            Ok(Ok(root)) => {
                let _ = core::fmt::Write::write_fmt(&mut name, format_args!("hw{:04}.txt", this_boot(files, root)));
                if write(files, root, name.as_str(), bytes) { mind::println!("[INIT] HARDWARE REPORT: log:{}, {} BYTES", name.as_str(), bytes.len()); }
                else { mind::println!("[INIT] HARDWARE REPORT NOT WRITTEN ({} BYTES)", bytes.len()); }
                tables(files, root);
                let _ = vfs::close(files, root);
            }
            _ => mind::println!("[INIT] HARDWARE REPORT: NO LOG VOLUME, NOT WRITTEN ({} BYTES)", bytes.len()),
        }
        done(text);
    }
    let _ = mind::ipc::drop_cap(client);
}

// Every ACPI table to log:acpi/<SIG>.bin (repeated signatures numbered: SSDT1.bin, SSDT2.bin, …).
fn tables(files: Endpoint, root: u32) {
    let Ok(Ok(dir)) = vfs::open_dir(files, root, "acpi", true) else { return };
    let mut seen: [([u8; 4], u32); 64] = [([0; 4], 0); 64];
    let (mut kinds, mut count) = (0, 0);
    for index in 0..256 {
        let Some(table) = object(PLATFORM_ACPI_TABLE, index) else { break };
        let bytes = table.1.as_slice();
        let rsdp = bytes.starts_with(b"RSD PTR ");
        let length = if rsdp { if bytes[15] >= 2 { u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize } else { 20 } } else { u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize };
        let signature: [u8; 4] = if rsdp { *b"RSDP" } else { bytes[..4].try_into().unwrap() };
        let repeat = match seen[..kinds].iter_mut().find(|(s, _)| *s == signature) {
            Some((_, n)) => { *n += 1; *n }
            None => { if kinds < seen.len() { seen[kinds] = (signature, 1); kinds += 1; } 1 }
        };
        let signature = core::str::from_utf8(&signature).unwrap_or("ACPI");
        let mut name = FixedBuf::<16>::new();
        let _ = if repeat > 1 || signature == "SSDT" { core::fmt::Write::write_fmt(&mut name, format_args!("{}{}.bin", signature, repeat)) } else { core::fmt::Write::write_fmt(&mut name, format_args!("{}.bin", signature)) };
        if write(files, dir, name.as_str(), &bytes[..length.min(bytes.len())]) { count += 1; }
        done(table);
    }
    let _ = vfs::close(files, dir);
    mind::println!("[INIT] ACPI TABLES: {} IN log:acpi/", count);
}
