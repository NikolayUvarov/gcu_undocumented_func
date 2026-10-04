//! Kernel observation (`STAT`, MC-10.2): records about tasks, CPUs, memory, address spaces, capabilities, endpoints,
//! interrupt lines and devices (`common/abi.rs`: StatTask .. StatDevice), and names for what they contain. Needs the
//! observe or the process-control privilege.
use crate::abi::*;
use crate::sys::{Error, Result};

/// Flags of a task in idl/sysinfo.wit (`task.flags`): a boot service, has a screen, has the focus.
pub const TASK_SERVICE: u8 = 1;
pub const TASK_SCREEN: u8 = 2;
pub const TASK_FOCUS: u8 = 4;

/// Records of one class in a caller's buffer.
pub struct Records<'a> { pub header: StatHeader, bytes: &'a [u8] }

impl<'a> Records<'a> {
    /// Number of records in the buffer; `total()` says how many exist.
    pub fn len(&self) -> usize { self.header.count as usize }
    pub fn is_empty(&self) -> bool { self.header.count == 0 }
    pub fn total(&self) -> usize { self.header.total as usize }
    /// The records as `T` (the record type of the class); a record smaller than `T` (an older kernel) gives nothing.
    pub fn iter<T: Copy>(&self) -> impl Iterator<Item = T> + '_ {
        let size = self.header.record_size as usize;
        let usable = size >= core::mem::size_of::<T>();
        (0..if usable { self.len() } else { 0 }).map(move |i| unsafe { core::ptr::read_unaligned(self.bytes.as_ptr().add(i * size) as *const T) })
    }
}

/// Reads class `class` (STAT_*) with `argument` (a PID for STAT_VMAP/STAT_CAPS) into `buffer`.
pub fn read(class: usize, argument: u64, buffer: &mut [u8]) -> Result<Records<'_>> {
    let header_size = core::mem::size_of::<StatHeader>();
    if buffer.len() < header_size { return Err(Error::Invalid); }
    let header = crate::control::stat(class, argument as usize, buffer)?;
    if header.version != STAT_VERSION { return Err(Error::Invalid); }
    let end = header_size + header.count as usize * header.record_size as usize;
    Ok(Records { header, bytes: &buffer[header_size..end.min(buffer.len())] })
}

/// One record of a single-record class (STAT_MEMORY).
pub fn one<T: Copy + Default>(class: usize) -> Result<T> {
    let mut buffer = [0u8; 512];
    let first = read(class, 0, &mut buffer)?.iter::<T>().next();
    first.ok_or(Error::NotFound)
}

/// Text for what a task waits for (StatTask::wait, WAIT_*).
pub fn state_name(wait: u8) -> &'static str {
    match wait { WAIT_NONE => "READY", WAIT_RUNNING => "RUNNING", WAIT_SLEEP => "SLEEP", WAIT_SEND => "SEND", WAIT_RECEIVE => "RECV", WAIT_REPLY => "CALL", WAIT_IRQ => "IRQ", WAIT_FLUSH => "FLUSH", WAIT_EXITED => "EXIT", _ => "?" }
}

/// Whether a task with this wait state can run (ready or running).
pub fn runnable(wait: u8) -> bool { matches!(wait, WAIT_NONE | WAIT_RUNNING) }

/// Name of a physical range kind: UEFI memory types and the platform layout.
pub fn phys_name(kind: u32) -> &'static str {
    match kind {
        0 => "reserved", 1 => "loader code", 2 => "loader data", 3 => "boot code", 4 => "boot data", 5 => "runtime code", 6 => "runtime data",
        7 => "free RAM", 8 => "unusable", 9 => "ACPI reclaim", 10 => "ACPI NVS", 11 => "MMIO", 12 => "MMIO ports", 13 => "PAL code", 14 => "persistent",
        PHYS_ARENA => "kernel arena", PHYS_BOOT_IMAGE => "boot image", PHYS_FRAMEBUFFER => "framebuffer",
        PHYS_AP_TRAMPOLINE => "AP trampoline", PHYS_PCI_BAR => "device BAR", PHYS_KERNEL => "kernel image",
        _ => "other",
    }
}

/// Name of an address-space region kind (REGION_*).
pub fn vm_name(kind: u32) -> &'static str {
    match kind { REGION_IMAGE => "image", REGION_STACK => "stack", REGION_SCREEN => "screen", REGION_INFO => "info", REGION_MAILBOX => "mailbox", REGION_EXIT => "exit", REGION_HEAP => "heap", REGION_SHARED => "shared", REGION_DEVICE => "device", REGION_GUARD => "guard", _ => "?" }
}

/// Name of a capability kind.
pub fn cap_name(kind: u32) -> &'static str {
    match kind as usize {
        CAP_KIND_ENDPOINT => "endpoint", CAP_KIND_MEMORY => "memory", CAP_KIND_DMA => "dma", CAP_KIND_PORTS => "ports", CAP_KIND_IRQ => "irq",
        CAP_KIND_INPUT => "input", CAP_KIND_DISPLAY => "display", CAP_KIND_MMIO => "mmio", CAP_KIND_SPAWN => "spawn", CAP_KIND_REPLY => "reply",
        CAP_KIND_PLATFORM => "platform", CAP_KIND_CONTROL => "control", CAP_KIND_RESTART => "restart", CAP_KIND_OBSERVE => "observe", _ => "?",
    }
}

/// A short name of a PCI class code.
pub fn class_name(class: u32) -> &'static str {
    match class >> 8 {
        0x0101 => "IDE controller", 0x0106 => "SATA controller", 0x0108 => "NVMe controller", 0x0200 => "Ethernet", 0x0300 => "VGA display", 0x0380 => "display",
        0x0401 => "audio (AC97)", 0x0403 => "audio (HDA)", 0x0600 => "host bridge", 0x0601 => "ISA bridge", 0x0680 => "bridge", 0x0C03 => "USB controller",
        0x0C05 => "SMBus", _ => match class >> 16 { 0x01 => "storage", 0x02 => "network", 0x03 => "display", 0x04 => "multimedia", 0x06 => "bridge", 0x0C => "serial bus", _ => "device" },
    }
}
