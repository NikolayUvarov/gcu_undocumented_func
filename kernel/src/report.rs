// The hardware report (174-KRN-0038): what the kernel can learn about the machine, as text, for init to write to the
// log volume at every boot. The processor's part is the architecture's (arch/*/report.rs).
use crate::abi::{BootInfo, StatPhys};
use crate::pci;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

const UEFI_TYPES: [&str; 16] = ["reserved", "loader code", "loader data", "boot services code", "boot services data", "runtime services code",
    "runtime services data", "conventional", "unusable", "ACPI reclaim", "ACPI NVS", "memory-mapped I/O", "memory-mapped I/O ports", "PAL code", "persistent", "unaccepted"];

fn class_name(class: u32) -> &'static str {
    match class >> 8 {
        0x0100 => "SCSI storage", 0x0101 => "IDE controller", 0x0104 => "RAID controller", 0x0106 => "SATA controller", 0x0107 => "SAS controller", 0x0108 => "NVMe controller", 0x0180 => "storage",
        0x0200 => "Ethernet", 0x0280 => "network (Wi-Fi or other)", 0x0300 => "VGA-compatible GPU", 0x0301 => "XGA GPU", 0x0302 => "3D GPU (no VGA)", 0x0380 => "display / GPU",
        0x0400 => "video", 0x0401 => "audio", 0x0403 => "HD audio", 0x0480 => "multimedia", 0x0500 => "RAM controller", 0x0580 => "memory controller",
        0x0600 => "host bridge", 0x0601 => "ISA bridge", 0x0604 => "PCI bridge", 0x0607 => "CardBus bridge", 0x0680 => "bridge", 0x0700 => "serial controller", 0x0780 => "communication (e.g. MEI)",
        0x0800 => "PIC", 0x0805 => "SD host controller", 0x0806 => "IOMMU", 0x0880 => "system peripheral", 0x0C00 => "FireWire", 0x0C03 => "USB controller", 0x0C05 => "SMBus",
        0x0C80 => "serial bus", 0x0B40 => "co-processor", 0x1101 => "performance counters", 0x1180 => "signal processing", 0x1200 => "processing accelerator (NPU)",
        _ => match class >> 16 { 0x03 => "display / GPU", 0x12 => "processing accelerator", 0x0D => "wireless", 0x10 => "encryption", _ => "" },
    }
}

fn vendor_name(vendor: u32) -> &'static str {
    match vendor { 0x8086 => "Intel", 0x1022 => "AMD", 0x1002 => "AMD/ATI", 0x10DE => "NVIDIA", 0x14E4 => "Broadcom", 0x106B => "Apple", 0x1AF4 => "Red Hat (VirtIO)", 0x1B36 => "Red Hat (QEMU)",
        0x1234 => "QEMU", 0x10EC => "Realtek", 0x144D => "Samsung", 0x15B7 => "SanDisk/WD", 0x1987 => "Phison", 0x1C5C => "SK hynix", 0x168C => "Qualcomm Atheros",
        0x17CB => "Qualcomm", 0x1B21 => "ASMedia", 0x1B4B => "Marvell", 0x1912 => "Renesas", 0x8087 => "Intel", 0x15AD => "VMware", 0x13B5 => "ARM", _ => "" }
}

fn hex_dump(out: &mut String, bytes: &[u8], indent: &str) {
    for (row, chunk) in bytes.chunks(16).enumerate() {
        let _ = write!(out, "{}{:03X}:", indent, row * 16);
        for byte in chunk { let _ = write!(out, " {:02X}", byte); }
        out.push('\n');
    }
}

/// The report as text.
pub fn build(boot: &BootInfo, devices: &[pci::Device]) -> Vec<u8> {
    let mut out = String::with_capacity(128 * 1024);
    let _ = writeln!(out, "MIND CORE HARDWARE REPORT 1 (174-KRN-0038): what the kernel learned about this machine at boot.\nABI {}, kernel built from the commit the boot manifest names.\n", crate::abi::ABI_VERSION);
    crate::arch::report::cpu(&mut out);
    memory(&mut out, boot);
    firmware(&mut out, boot);
    pci_section(&mut out, devices);
    out.push_str("\nThe kernel's choices\n");
    crate::arch::report::kernel(&mut out);
    let (frames, free) = crate::frames::stats();
    let _ = writeln!(out, "  frame pool        {} MiB, {} MiB free\n  clock resolution  {} ns", frames >> 20, free >> 20, crate::clock::resolution_ns());
    let _ = writeln!(out, "  screen            {}x{} stride {} format {} at {:#X}", boot.width, boot.height, boot.stride, boot.pixel_format, boot.fb_ptr as usize);
    let _ = writeln!(out, "  boot volume       kind {} partition {} start {} sectors {}; slot {}{}", boot.boot_volume.kind, boot.boot_volume.partition, boot.boot_volume.start, boot.boot_volume.sectors,
        boot.boot_slot.slot, if boot.boot_slot.trial != 0 { " on trial" } else { "" });
    let _ = writeln!(out, "  UEFI runtime      {}\n  device tree       {:#X}", if boot.efi_runtime != 0 { "present" } else { "absent" }, boot.device_tree);
    let mut ring = Vec::new();
    let dropped = crate::klog::copy(&mut ring);
    let _ = writeln!(out, "\nKernel lines{}\n", if dropped { " (the last 32 KiB)" } else { "" });
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(&ring);
    bytes
}

fn memory(out: &mut String, boot: &BootInfo) {
    let map = unsafe { core::slice::from_raw_parts(boot.memory_map, boot.memory_map_len) };
    let mut totals = [0u64; 16];
    out.push_str("\nMemory map (the firmware's)\n");
    for entry in map.iter().filter(|e: &&StatPhys| e.kind < crate::abi::PHYS_PLATFORM) {
        let name = UEFI_TYPES.get(entry.kind as usize).copied().unwrap_or("?");
        if let Some(total) = totals.get_mut(entry.kind as usize) { *total += entry.pages * 4096; }
        let _ = writeln!(out, "  {:016X}-{:016X} {:>10} KiB  {}", entry.start, entry.start + entry.pages * 4096 - 1, entry.pages * 4, name);
    }
    out.push_str("  totals:");
    for (kind, total) in totals.iter().enumerate().filter(|(_, t)| **t > 0) { let _ = write!(out, " {} {} MiB;", UEFI_TYPES[kind], total >> 20); }
    out.push('\n');
}

/// The n-th ACPI table, in the order of the report's list.
pub fn acpi_table(index: usize) -> Option<&'static [u8]> {
    let mut found = None; let mut n = 0;
    crate::acpi::tables(|table| { if n == index { found = Some(table); } n += 1; });
    found
}

fn u16_at(t: &[u8], at: usize) -> u16 { t.get(at..at + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]])) }
fn u32_at(t: &[u8], at: usize) -> u32 { t.get(at..at + 4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap())) }
fn u64_at(t: &[u8], at: usize) -> u64 { t.get(at..at + 8).map_or(0, |b| u64::from_le_bytes(b.try_into().unwrap())) }
fn ascii(bytes: &[u8]) -> String { bytes.iter().map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { '.' }).collect() }

fn firmware(out: &mut String, boot: &BootInfo) {
    let _ = writeln!(out, "\nACPI tables (RSDP at {:#X}; each also written to log:acpi/ by init)", boot.acpi_rsdp);
    let mut index = 0;
    crate::acpi::tables(|t| {
        if t.starts_with(b"RSD PTR ") {
            let _ = writeln!(out, "  {:>2} RSDP revision {} OEM \"{}\"", index, t[15], ascii(&t[9..15]));
        } else {
            let _ = writeln!(out, "  {:>2} {} {:>7} B revision {} OEM \"{}\" \"{}\" {:#X} creator \"{}\" {:#X}", index, ascii(&t[..4]), t.len(), t.get(8).copied().unwrap_or(0),
                ascii(t.get(10..16).unwrap_or(&[])), ascii(t.get(16..24).unwrap_or(&[])), u32_at(t, 24), ascii(t.get(28..32).unwrap_or(&[])), u32_at(t, 32));
            decode(out, t);
        }
        index += 1;
    });
}

// The tables whose content says the most about the machine, decoded in a line or a few.
fn decode(out: &mut String, t: &[u8]) {
    match &t[..4] {
        b"APIC" => {
            let (mut cpus, mut enabled, mut ioapics, mut other) = (0, 0, 0, 0);
            let mut at = 44;
            while at + 2 <= t.len() && t[at + 1] >= 2 {
                match t[at] { 0 => { cpus += 1; if u32_at(t, at + 4) & 1 != 0 { enabled += 1; } } 9 => { cpus += 1; if u32_at(t, at + 8) & 1 != 0 { enabled += 1; } }
                    0xB => { cpus += 1; if u32_at(t, at + 12) & 1 != 0 { enabled += 1; } } 1 | 0xC => ioapics += 1, _ => other += 1 }
                at += t[at + 1] as usize;
            }
            let _ = writeln!(out, "       processors {} ({} enabled), interrupt controllers {}, other entries {}; local APIC at {:#X}", cpus, enabled, ioapics, other, u32_at(t, 36));
        }
        b"FACP" => {
            let profile = ["unspecified", "desktop", "mobile", "workstation", "enterprise server", "SOHO server", "appliance PC", "performance server", "tablet"];
            let _ = writeln!(out, "       profile {}, flags {:#X}, PM timer port {:#X}, SCI {}, reset register {:#X} value {:#X}, ARM boot flags {:#X}",
                profile.get(t.get(45).copied().unwrap_or(0) as usize).unwrap_or(&"?"), u32_at(t, 112), u32_at(t, 76), u16_at(t, 46), u64_at(t, 120), t.get(128).copied().unwrap_or(0), u16_at(t, 129));
        }
        b"MCFG" => for at in (44..t.len().saturating_sub(15)).step_by(16) {
            let _ = writeln!(out, "       ECAM {:#X} segment {} buses {}-{}", u64_at(t, at), u16_at(t, at + 8), t[at + 10], t[at + 11]);
        },
        b"HPET" => { let _ = writeln!(out, "       HPET at {:#X}, {} comparators", u64_at(t, 44), ((u32_at(t, 36) >> 8) & 0x1F) + 1); }
        b"DMAR" => {
            let (mut units, mut at) = (0, 48);
            while at + 4 <= t.len() && u16_at(t, at + 2) >= 4 { if u16_at(t, at) == 0 { units += 1; } at += u16_at(t, at + 2) as usize; }
            let _ = writeln!(out, "       Intel VT-d: host address width {}, flags {:#X}, {} remapping units (an IOMMU is present)", t.get(36).copied().unwrap_or(0) as u32 + 1, t.get(37).copied().unwrap_or(0), units);
        }
        b"IVRS" => { let _ = writeln!(out, "       AMD-Vi: IVinfo {:#X} (an IOMMU is present)", u32_at(t, 36)); }
        b"SRAT" => {
            let (mut cpus, mut ranges, mut domains, mut at) = (0, 0, 0u64, 48);
            while at + 2 <= t.len() && t[at + 1] >= 2 {
                match t[at] { 0 | 2 | 3 => cpus += 1, 1 => { ranges += 1; domains |= 1 << (u32_at(t, at + 2) & 63); let _ = writeln!(out, "       memory {:#X}+{:#X} in domain {}", u64_at(t, at + 8), u64_at(t, at + 16), u32_at(t, at + 2)); } _ => {} }
                at += t[at + 1] as usize;
            }
            let _ = writeln!(out, "       NUMA: {} processor entries, {} memory ranges, {} domains", cpus, ranges, domains.count_ones());
        }
        b"SLIT" => {
            let n = u64_at(t, 36) as usize;
            for row in 0..n.min(16) { let _ = write!(out, "       distances from {}:", row); for col in 0..n.min(16) { let _ = write!(out, " {}", t.get(44 + row * n + col).copied().unwrap_or(0)); } out.push('\n'); }
        }
        b"TPM2" => { let _ = writeln!(out, "       TPM 2.0: start method {}, control area {:#X}", u32_at(t, 48), u64_at(t, 40)); }
        b"BGRT" => { let _ = writeln!(out, "       boot logo at {:#X}", u64_at(t, 40)); }
        _ => {}
    }
}

fn pci_section(out: &mut String, devices: &[pci::Device]) {
    let _ = writeln!(out, "\nPCI functions ({}; index as DEVICE_FIND and `devices` give it)", devices.len());
    for (index, device) in devices.iter().enumerate() {
        let config: Vec<u8> = (0..64u8).flat_map(|i| unsafe { pci::config(device, i * 4) }.to_le_bytes()).collect();
        let location = device.location();
        let (vendor, id) = (device.id & 0xFFFF, device.id >> 16);
        let kind = match device.class >> 16 { 0x03 => " [GPU]", 0x12 => " [NPU / accelerator]", 0x0B if device.class >> 8 == 0x0B40 => " [co-processor]", _ => "" };
        let _ = writeln!(out, "\n  {:02} {:02X}:{:02X}.{} {:04X}:{:04X} {} {}{}\n     class {:06X} revision {:02X} header {:02X} subsystem {:04X}:{:04X} IRQ line {} pin {} command {:04X} status {:04X}{}",
            index, location >> 8, (location >> 3) & 0x1F, location & 7, vendor, id, vendor_name(vendor), class_name(device.class), kind, device.class, config[8], config[14] & 0x7F,
            u16_at(&config, 0x2C), u16_at(&config, 0x2E), config[0x3C], config[0x3D], u16_at(&config, 4), u16_at(&config, 6), match (device.granted, device.mastered_at_boot) {
                (true, _) => " (granted to a driver)", (false, true) => " (bus mastering, on at boot, turned off)", (false, false) => "" });
        for (number, bar) in device.bars.iter().enumerate().filter(|(_, b)| b.size != 0) {
            let raw = u32_at(&config, 0x10 + number * 4);
            let _ = writeln!(out, "     BAR{} {} {:#X} size {:#X}{}", number, if bar.io { "I/O" } else if (raw >> 1) & 3 == 2 { "mem64" } else { "mem32" }, bar.base, bar.size,
                if !bar.io && raw & 8 != 0 { " prefetchable" } else { "" });
        }
        for (base, end) in device.windows.iter().filter(|w| w.1 > w.0) { let _ = writeln!(out, "     window {:#X}-{:#X}", base, end - 1); }
        capabilities(out, &config);
        if vendor == 0x10DE && device.class >> 16 == 0x03 { nvidia(out, device); }
        hex_dump(out, &config, "     ");
    }
}

fn capabilities(out: &mut String, config: &[u8]) {
    if u16_at(config, 6) & 0x10 == 0 { return; }
    let (mut at, mut seen) = (config[0x34] as usize & 0xFC, 0);
    while at >= 0x40 && at + 2 <= config.len() && seen < 48 {
        let (id, next) = (config[at], config[at + 1] as usize & 0xFC);
        let _ = match id {
            0x01 => writeln!(out, "     cap {:02X} power management, version {}", at, u16_at(config, at + 2) & 7),
            0x05 => writeln!(out, "     cap {:02X} MSI, {} vectors, {}-bit", at, 1 << ((u16_at(config, at + 2) >> 1) & 7), if u16_at(config, at + 2) & 0x80 != 0 { 64 } else { 32 }),
            0x10 => {
                let kind = ["endpoint", "legacy endpoint", "", "", "root port", "upstream switch port", "downstream switch port", "PCIe-to-PCI bridge", "PCI-to-PCIe bridge", "root complex endpoint", "event collector"];
                let (caps, link_caps, link_status) = (u32_at(config, at + 4), u32_at(config, at + 0x0C), u16_at(config, at + 0x12));
                writeln!(out, "     cap {:02X} PCI Express {}, max payload {} B; link: up to gen {} x{}, now gen {} x{}", at, kind.get(((u16_at(config, at + 2) >> 4) & 0xF) as usize).unwrap_or(&"?"),
                    128 << (caps & 7), link_caps & 0xF, (link_caps >> 4) & 0x3F, link_status & 0xF, (link_status >> 4) & 0x3F)
            }
            0x11 => writeln!(out, "     cap {:02X} MSI-X, {} entries, table BAR{} +{:#X}", at, (u16_at(config, at + 2) & 0x7FF) + 1, u32_at(config, at + 4) & 7, u32_at(config, at + 4) & !7),
            0x09 => writeln!(out, "     cap {:02X} vendor-specific, {} B", at, config.get(at + 2).copied().unwrap_or(0)),
            0x12 => writeln!(out, "     cap {:02X} SATA", at),
            0x13 => writeln!(out, "     cap {:02X} advanced features", at),
            0x0D => writeln!(out, "     cap {:02X} bridge subsystem IDs", at),
            other => writeln!(out, "     cap {:02X} id {:#04X}", at, other),
        };
        at = next; seen += 1;
    }
}

// NVIDIA's PMC_BOOT_0 (BAR0 + 0) names the chip; read only where decoding is on and BAR0 is in the kernel's map.
fn nvidia(out: &mut String, device: &pci::Device) {
    let bar = device.bars[0];
    let decoding = unsafe { pci::config(device, 4) } & 2 != 0;
    if bar.io || bar.size == 0 || !decoding || bar.base + 4 > crate::mmu::IDENTITY_END as u64 { let _ = writeln!(out, "     NVIDIA chip: not read (BAR0 not decoded or not mapped)"); return; }
    let boot0 = unsafe { core::ptr::read_volatile(bar.base as *const u32) };
    let _ = writeln!(out, "     NVIDIA PMC_BOOT_0 {:08X}: chipset {:#X}, revision {:#X}", boot0, (boot0 >> 20) & 0x1FF, boot0 & 0xFF);
}
