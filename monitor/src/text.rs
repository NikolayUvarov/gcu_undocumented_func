//! Numbers as the monitors print them.
use crate::abi::*;
use alloc::format;
use alloc::string::String;

/// Size with a unit: 512B, 12.3K, 4.0M, 1.2G.
pub fn size(bytes: u64) -> String {
    for (unit, letter) in [(1u64 << 30, 'G'), (1 << 20, 'M'), (1 << 10, 'K')] {
        if bytes >= unit {
            let tenths = bytes as u128 * 10 / unit as u128;
            return if tenths >= 1000 { format!("{}{}", tenths / 10, letter) } else { format!("{}.{}{}", tenths / 10, tenths % 10, letter) };
        }
    }
    format!("{}B", bytes)
}

/// A count with thousands grouped by spaces: 1 234 567.
pub fn count(value: u64) -> String {
    let digits = format!("{}", value);
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 { out.push(' '); }
        out.push(ch);
    }
    out
}

/// CPU time: M:SS.cc below an hour, then H:MM:SS.
pub fn cpu_time(ns: u64) -> String {
    let centis = ns / 10_000_000;
    let seconds = centis / 100;
    if seconds < 3600 { format!("{}:{:02}.{:02}", seconds / 60, seconds % 60, centis % 100) } else { format!("{}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60) }
}

/// Uptime: H:MM:SS, with days after the first.
pub fn uptime(ms: u64) -> String {
    let seconds = ms / 1000;
    let days = seconds / 86_400;
    if days == 0 { format!("{}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60) } else { format!("{}d {}:{:02}:{:02}", days, seconds / 3600 % 24, seconds / 60 % 60, seconds % 60) }
}

/// A value x 100 as a decimal: 12 → 0.12.
pub fn hundredths(value: u32) -> String { format!("{}.{:02}", value / 100, value % 100) }

/// Per mille as a percentage with one decimal: 425 → 42.5.
pub fn permille(value: u32) -> String { format!("{}.{}", value / 10, value % 10) }

/// Short name of a task state.
pub fn state(state: u8) -> &'static str {
    match state { WAIT_NONE => "READY", WAIT_RUNNING => "RUN", WAIT_SLEEP => "SLEEP", WAIT_SEND => "SEND", WAIT_RECEIVE => "RECV", WAIT_REPLY => "CALL", WAIT_IRQ => "IRQ", WAIT_FLUSH => "FLUSH", WAIT_EXITED => "EXIT", _ => "?" }
}

/// What a task in `state` (WAIT_*) waits for, from `StatTask.wait_on`.
pub fn waits_for(state: u8, wait: u64) -> String {
    match state {
        WAIT_SEND => format!("sending to endpoint {}", wait), WAIT_RECEIVE => format!("receiving on endpoint {}", wait),
        WAIT_REPLY => format!("waiting for a reply from PID {}", wait), WAIT_IRQ => format!("waiting for IRQ {}", wait),
        WAIT_SLEEP => String::from("sleeping"), WAIT_FLUSH => String::from("waiting for a TLB flush"),
        WAIT_RUNNING => String::from("running"), WAIT_NONE => String::from("ready to run"), WAIT_EXITED => String::from("exited"), _ => String::new(),
    }
}

/// Name of a physical range kind: UEFI memory types and the platform layout.
pub fn phys_kind(kind: u32) -> &'static str {
    match kind {
        0 => "reserved", 1 => "loader code", 2 => "loader data", 3 => "boot code", 4 => "boot data", 5 => "runtime code", 6 => "runtime data",
        7 => "free RAM", 8 => "unusable", 9 => "ACPI reclaim", 10 => "ACPI NVS", 11 => "MMIO", 12 => "MMIO ports", 13 => "PAL code", 14 => "persistent",
        PHYS_ARENA => "kernel arena", PHYS_BOOT_IMAGE => "boot image", PHYS_FRAMEBUFFER => "framebuffer",
        PHYS_AP_TRAMPOLINE => "AP trampoline", PHYS_PCI_BAR => "device BAR", PHYS_KERNEL => "kernel image",
        _ => "other",
    }
}

/// Name of an address-space region kind.
pub fn region_kind(kind: u32) -> &'static str {
    match kind { REGION_IMAGE => "image", REGION_STACK => "stack", REGION_SCREEN => "screen", REGION_INFO => "info", REGION_MAILBOX => "mailbox", REGION_EXIT => "exit", REGION_HEAP => "heap", REGION_SHARED => "shared", REGION_DEVICE => "device", REGION_GUARD => "guard", _ => "?" }
}

/// `rwx` rights of a region.
pub fn rights(flags: u32) -> String {
    [(REGION_READ, 'r'), (REGION_WRITE, 'w'), (REGION_EXECUTE, 'x')].iter().map(|&(bit, ch)| if flags & bit != 0 { ch } else { '-' }).collect()
}

/// Name of a capability kind.
pub fn cap_kind(kind: u32) -> &'static str {
    match kind as usize {
        CAP_KIND_ENDPOINT => "endpoint", CAP_KIND_MEMORY => "memory", CAP_KIND_DMA => "dma", CAP_KIND_PORTS => "ports", CAP_KIND_IRQ => "irq",
        CAP_KIND_INPUT => "input", CAP_KIND_DISPLAY => "display", CAP_KIND_MMIO => "mmio", CAP_KIND_SPAWN => "spawn", CAP_KIND_REPLY => "reply",
        CAP_KIND_PLATFORM => "platform", CAP_KIND_CONTROL => "control", CAP_KIND_RESTART => "restart", CAP_KIND_OBSERVE => "observe", _ => "?",
    }
}

/// A short name of a PCI class code (class, subclass, interface).
pub fn pci_class(class: u32) -> &'static str {
    match class >> 8 {
        0x0101 => "IDE controller", 0x0106 => "SATA controller", 0x0108 => "NVMe controller", 0x0200 => "Ethernet", 0x0300 => "VGA display", 0x0380 => "display",
        0x0401 => "audio (AC97)", 0x0403 => "audio (HDA)", 0x0600 => "host bridge", 0x0601 => "ISA bridge", 0x0680 => "bridge", 0x0C03 => match class & 0xFF { 0x30 => "USB xHCI", 0x20 => "USB EHCI", 0x10 => "USB OHCI", 0x00 => "USB UHCI", _ => "USB controller" },
        0x0C05 => "SMBus", _ => match class >> 16 { 0x01 => "storage", 0x02 => "network", 0x03 => "display", 0x04 => "multimedia", 0x06 => "bridge", 0x0C => "serial bus", _ => "device" },
    }
}

/// The largest of 1, 2, 5 x 10^k at or above `value` (graph scales).
pub fn nice_max(value: u64) -> u64 {
    let mut unit = 1u64;
    loop {
        for step in [1, 2, 5] { if value <= unit * step { return unit * step; } }
        if unit > u64::MAX / 100 { return u64::MAX; }
        unit *= 10;
    }
}
