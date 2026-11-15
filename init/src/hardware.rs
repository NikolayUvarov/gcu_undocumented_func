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

/// This boot's log on the log volume (`log:bootNNNN.log`, 801-KRN-0242): its number, 0 without a log volume.
pub fn boot_log(keeper: usize) -> u32 {
    let Ok(client) = mind::ipc::mint_badged(keeper, crate::CLIENT, mind::fs::BADGE_USER) else { return 0 };
    let files = Endpoint(client);
    let mut this = 0;
    if let Ok(Ok(root)) = vfs::root(files, "log") { this = this_boot(files, root); let _ = vfs::close(files, root); }
    let _ = mind::ipc::drop_cap(client);
    this
}

/// The platform's transmit power policy for a WLAN device (213-KRN-0122): every DSDT and SSDT read whole and checked,
/// and the SAR names searched for; a table that cannot be mapped makes it unknown. Read once at boot, while the platform
/// privilege allows it.
pub fn power_policy() -> mind::platform_power::Policy {
    let mut search = mind::platform_power::Search::new();
    for index in 0..256 {
        let Ok(slot) = platform::cap(PLATFORM_ACPI_TABLE, index, 0) else { break };
        match Mapping::new(slot) {
            Ok(map) => { search.feed(map.as_slice()); drop(map); }
            Err(_) => search.unreadable(),
        }
        let _ = mind::ipc::drop_cap(slot);
    }
    search.finish()
}

/// The embedded controller's command/status and data ports (the ECDT), its SMBus host's base (ACPI0001's `_EC` in the
/// DSDT or an SSDT) and, where the tables name it, the AC adapter's bit in its space (`_PSR`), for the battery service
/// (211-PRT-0008); None where the tables do not name the ports and the base.
pub fn battery_place() -> Option<(u16, u16, u8, Option<(u8, u8)>)> {
    let (mut ports, mut base, mut ac) = (None, None, None);
    for index in 0..256 {
        let Some(table) = object(PLATFORM_ACPI_TABLE, index) else { break };
        let bytes = table.1.as_slice();
        let length = bytes.get(4..8).map_or(0, |l| u32::from_le_bytes(l.try_into().unwrap()) as usize).min(bytes.len());
        if bytes.starts_with(b"ECDT") { ports = ports.or_else(|| mind::battery::ecdt_ports(&bytes[..length])); }
        if (bytes.starts_with(b"DSDT") || bytes.starts_with(b"SSDT")) && length > 36 {
            base = base.or_else(|| mind::battery::smbus_base(&bytes[36..length]));
            ac = ac.or_else(|| mind::battery::ac_bit(&bytes[36..length]));
        }
        done(table);
    }
    let (command, data) = ports?;
    Some((command, data, base?, ac))
}

/// The FADT's fixed hardware (000-KRN-0216), for the power button; None where the tables have no FADT.
pub fn fadt() -> Option<mind::fadt::Fixed> {
    let mut found = None;
    for index in 0..256 {
        let Some(table) = object(PLATFORM_ACPI_TABLE, index) else { break };
        let bytes = table.1.as_slice();
        if bytes.starts_with(b"FACP") {
            let length = bytes.get(4..8).map_or(0, |l| u32::from_le_bytes(l.try_into().unwrap()) as usize).min(bytes.len());
            found = mind::fadt::parse(&bytes[..length]);
        }
        done(table);
        if found.is_some() { break; }
    }
    found
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

/// The platform's nodes init gives drivers (100000-KRN-0171), found once by their first compatible string.
#[derive(Clone, Copy, Default)]
pub struct Nodes {
    /// The console, when it is a PL011 the shell's serial console drives.
    pub console: Option<usize>,
    pub rtc: Option<usize>,
    pub pl061: Option<usize>,
    pub bcm2711: Option<usize>,
    pub tpm: Option<usize>,
}

/// Reads the kernel's table and names what was found on the boot log.
pub fn platform_nodes() -> Nodes {
    let mut found = Nodes::default();
    let Some(table) = object(PLATFORM_NODES, 0) else { mind::println!("[INIT] PLATFORM NODES NOT READ"); return found };
    let Some((header, records)) = mind::nodes::read(table.1.as_slice()) else { mind::println!("[INIT] PLATFORM NODES: A SHORT TABLE, NOT READ"); done(table); return found };
    let mut other_console = FixedBuf::<{ NODE_COMPATIBLE }>::new();
    for (index, node) in records.enumerate() {
        let first = |at: &mut Option<usize>| { if at.is_none() { *at = Some(index); } };
        match node.compatible() {
            b"arm,pl011" | b"arm,sbsa-uart" if node.flags & NODE_CONSOLE != 0 => first(&mut found.console),
            b"arm,pl031" => first(&mut found.rtc),
            b"arm,pl061" => first(&mut found.pl061),
            b"brcm,bcm2711-gpio" => first(&mut found.bcm2711),
            b"mind,tpm2" => first(&mut found.tpm),
            name if node.flags & NODE_CONSOLE != 0 => { let _ = core::fmt::Write::write_str(&mut other_console, core::str::from_utf8(name).unwrap_or("?")); }
            _ => {}
        }
    }
    done(table);
    let at = |n: Option<usize>| n.map_or(-1, |i| i as i64);
    mind::println!("[INIT] PLATFORM NODES: {}{}; CONSOLE {}, RTC {}, PL061 {}, BCM2711 GPIO {}, TPM {} (-1: NONE)", header.count, if header.left_out > 0 { " AND MORE LEFT OUT" } else { "" },
        at(found.console), at(found.rtc), at(found.pl061), at(found.bcm2711), at(found.tpm));
    if !other_console.as_str().is_empty() { mind::println!("[INIT] THE CONSOLE ({}) STAYS THE KERNEL'S: THE SHELL'S SERIAL CONSOLE DRIVES A PL011 ONLY", other_console.as_str()); }
    found
}
