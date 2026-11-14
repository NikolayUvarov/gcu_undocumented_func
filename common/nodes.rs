#![allow(dead_code)]
// The platform's nodes (100000-KRN-0171): the devices outside PCI the kernel lends to drivers, read once at boot from the
// device tree or made from the ACPI tables, before the other CPUs start. PLATFORM_NODES copies the table out; PLATFORM_MMIO
// and PLATFORM_NODE_IRQ lend from it. A page that another node's registers share is lent to neither: the A64's four UARTs
// share one, the Bluetooth chip's among them. Shared by the kernel and the host tests.
use crate::abi::{PlatformNode, PlatformNodes, NODE_COMPATIBLE, NODE_LINES, NODE_MORE, NODE_REGS, PLATFORM_NODES_MAX};

const PAGE: u64 = 4096;

/// Whether a UEFI memory type is memory the firmware or the system uses, which no node's registers may be lent over
/// (801-KRN-0244): loader, boot and runtime code and data, conventional, unusable, ACPI reclaim and NVS, persistent and
/// unaccepted memory. Reserved (0) and memory-mapped I/O (11, 12) are not: firmware lists a device's registers as
/// either, the DEXP's Intel CRB TPM among them.
pub fn holds_ram(kind: u32) -> bool { matches!(kind, 1..=10 | 14 | 15) }

// The header right before the records, as PLATFORM_NODES hands them out.
#[repr(C)]
pub struct Nodes { header: PlatformNodes, list: [PlatformNode; PLATFORM_NODES_MAX] }

impl Nodes {
    pub const fn new() -> Nodes { Nodes { header: PlatformNodes { count: 0, left_out: 0 }, list: [PlatformNode::EMPTY; PLATFORM_NODES_MAX] } }

    pub fn count(&self) -> usize { self.header.count as usize }
    pub fn left_out(&self) -> usize { self.header.left_out as usize }
    pub fn get(&self, index: usize) -> Option<&PlatformNode> { self.list[..self.count()].get(index) }
    pub fn iter(&self) -> impl Iterator<Item = &PlatformNode> { self.list[..self.count()].iter() }

    /// Adds a node, its first NODE_REGS ranges and NODE_LINES lines (NODE_MORE when it has more, or `flags` says so);
    /// false when the table is full, the node counted as left out.
    pub fn add(&mut self, compatible: &[u8], flags: u32, regs: &[(u64, u64)], lines: &[u32]) -> bool {
        let count = self.count();
        if count == PLATFORM_NODES_MAX { self.header.left_out = self.header.left_out.saturating_add(1); return false; }
        let mut node = PlatformNode::EMPTY;
        let cut = compatible.len().min(NODE_COMPATIBLE);
        node.compatible[..cut].copy_from_slice(&compatible[..cut]);
        node.flags = flags | if regs.len() > NODE_REGS || lines.len() > NODE_LINES { NODE_MORE } else { 0 };
        for (slot, &(base, size)) in node.regs.iter_mut().zip(regs) { *slot = [base, size]; }
        for (slot, &line) in node.lines.iter_mut().zip(lines) { *slot = line; }
        node.reg_count = regs.len().min(NODE_REGS) as u8;
        node.line_count = lines.len().min(NODE_LINES) as u8;
        self.list[count] = node;
        self.header.count += 1;
        true
    }

    /// The pages of node `index`'s register range `reg`: the first page's address and the bytes to the end of the last.
    pub fn pages(&self, index: usize, reg: usize) -> Option<(u64, u64)> {
        let node = self.get(index)?;
        if reg >= node.reg_count as usize { return None; }
        let [base, size] = node.regs[reg];
        if size == 0 { return None; }
        let end = base.checked_add(size)?.checked_next_multiple_of(PAGE)?;
        let start = base & !(PAGE - 1);
        Some((start, end - start))
    }

    /// Another node with registers in the pages of node `index`'s range `reg`, if one has (its other ranges are its own).
    pub fn sharer(&self, index: usize, reg: usize) -> Option<usize> {
        let (start, bytes) = self.pages(index, reg)?;
        let end = start + bytes;
        self.iter().enumerate().filter(|&(i, _)| i != index).find(|(_, node)| {
            node.regs[..node.reg_count as usize].iter().any(|&[base, size]| size != 0 && base < end && base.saturating_add(size) > start)
        }).map(|(i, _)| i)
    }

    /// Line `n` of node `index`.
    pub fn line(&self, index: usize, n: usize) -> Option<u32> {
        let node = self.get(index)?;
        (n < node.line_count as usize).then(|| node.lines[n])
    }

    /// The table as PLATFORM_NODES hands it out: the header, then `count` records (no padding in either).
    pub fn as_bytes(&self) -> &[u8] {
        let bytes = core::mem::size_of::<PlatformNodes>() + self.count() * core::mem::size_of::<PlatformNode>();
        unsafe { core::slice::from_raw_parts(self as *const Nodes as *const u8, bytes) }
    }
}

/// The header and records of a table PLATFORM_NODES handed out (`bytes`, as mapped): None when it is shorter than its
/// header says.
pub fn read(bytes: &[u8]) -> Option<(PlatformNodes, impl Iterator<Item = PlatformNode> + '_)> {
    let head = core::mem::size_of::<PlatformNodes>();
    let size = core::mem::size_of::<PlatformNode>();
    if bytes.len() < head { return None; }
    let header = unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const PlatformNodes) };
    let count = (header.count as usize).min(PLATFORM_NODES_MAX);
    if bytes.len() < head + count * size { return None; }
    Some((header, (0..count).map(move |i| unsafe { core::ptr::read_unaligned(bytes[head + i * size..].as_ptr() as *const PlatformNode) })))
}
