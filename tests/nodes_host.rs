//! Host tests of the platform's nodes (common/nodes.rs and fdt.rs's `devices`, 100000-KRN-0171): which nodes of QEMU
//! `virt`'s and the PinePhone's trees are devices to lend, their ranges and lines; that a page another node shares is
//! lent to neither (the A64's four UARTs, the H5's UART0 and UART1); that the H618's console page is UART0's alone in its
//! tree (100004-PRT-0013); and the table's bounds, as PLATFORM_NODES hands it out.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../common/fdt.rs"]
mod fdt;
#[path = "../common/nodes.rs"]
mod nodes;

use abi::{PlatformNode, NODE_CONSOLE, NODE_MORE, NODE_REGS, PLATFORM_NODES_MAX};
use nodes::Nodes;

const VIRT_V2: &[u8] = include_bytes!("data/fdt/virt-gicv2-uboot.dtb");
const VIRT_V3: &[u8] = include_bytes!("data/fdt/virt-gicv3.dtb");
const PINEPHONE: &[u8] = include_bytes!("data/fdt/pinephone-1.2.dtb");
const ZERO_PLUS2: &[u8] = include_bytes!("data/fdt/orangepi-zero-plus2-h5.dtb");
const ZERO3: &[u8] = include_bytes!("data/fdt/orangepi-zero3-h618.dtb");

// The table the kernel builds from `blob` (devicetree.rs does the same).
fn table(blob: &[u8]) -> Nodes {
    let tree = fdt::Tree::open(blob).expect("a tree");
    let mut nodes = Nodes::new();
    fdt::devices(&tree, |d| { nodes.add(d.compatible, if d.console { NODE_CONSOLE } else { 0 } | if d.more { NODE_MORE } else { 0 }, &d.regs[..d.reg_count], &d.lines[..d.line_count]); });
    nodes
}
fn named<'a>(nodes: &'a Nodes, compatible: &str) -> Vec<(usize, &'a PlatformNode)> {
    nodes.iter().enumerate().filter(|(_, n)| n.compatible() == compatible.as_bytes()).collect()
}

#[test]
fn virt_lists_its_devices_and_not_what_the_kernel_drives() {
    for blob in [VIRT_V2, VIRT_V3] {
        let nodes = table(blob);
        let uart = named(&nodes, "arm,pl011");
        assert_eq!(uart.len(), 1);
        let (index, pl011) = uart[0];
        assert_eq!((pl011.regs[0], pl011.reg_count, &pl011.lines[..pl011.line_count as usize]), ([0x0900_0000, 0x1000], 1, &[1u32][..]));
        assert_eq!(pl011.flags & NODE_CONSOLE, NODE_CONSOLE);
        assert_eq!(nodes.pages(index, 0), Some((0x0900_0000, 0x1000)));
        assert_eq!(nodes.sharer(index, 0), None);
        let rtc = named(&nodes, "arm,pl031");
        assert_eq!((rtc.len(), rtc[0].1.regs[0][0], rtc[0].1.lines[0]), (1, 0x0901_0000, 2));
        assert_eq!(named(&nodes, "arm,pl061").len(), 1);
        assert_eq!(named(&nodes, "virtio,mmio").len(), 32);
        // The GIC, its MSI frame or ITS, the PCI host, the memory and the CPUs are the kernel's: not listed.
        for kernels in ["arm,cortex-a15-gic", "arm,gic-v3", "arm,gic-v2m-frame", "arm,gic-v3-its", "pci-host-ecam-generic", "arm,cortex-a53", "arm,cortex-a57", "arm,armv8-timer", "arm,psci-1.0"] {
            assert!(named(&nodes, kernels).is_empty(), "{kernels} listed");
        }
        assert!(nodes.iter().all(|n| n.reg_count > 0 && !n.compatible().is_empty()));
        assert!(nodes.iter().filter(|n| n.flags & NODE_CONSOLE != 0).count() == 1);
        assert!(nodes.count() < PLATFORM_NODES_MAX && nodes.left_out() == 0);
    }
}

#[test]
fn the_pinephones_uarts_share_a_page_and_none_is_lent_by_it() {
    let nodes = table(PINEPHONE);
    let uarts = named(&nodes, "snps,dw-apb-uart");
    // UART0 (the console at 0x1c28000), UART1 (Bluetooth) and UART3 are enabled; UART2 and UART4 are not.
    let bases: Vec<u64> = uarts.iter().map(|(_, n)| n.regs[0][0]).collect();
    assert_eq!(bases, [0x01c2_8000, 0x01c2_8400, 0x01c2_8c00]);
    let (console, uart0) = uarts[0];
    assert_eq!(uart0.flags & NODE_CONSOLE, NODE_CONSOLE);
    assert_eq!(uart0.regs[0][1], 0x400);
    assert_eq!(nodes.pages(console, 0), Some((0x01c2_8000, 0x1000)));
    assert_eq!(nodes.sharer(console, 0), Some(uarts[1].0));
    assert_eq!(nodes.sharer(uarts[1].0, 0), Some(console));
    // Its line is SPI 0, to the GIC.
    assert_eq!(&uart0.lines[..uart0.line_count as usize], &[0]);
    // The GIC-400 and the memory are not listed; a disabled node is not.
    assert!(named(&nodes, "arm,gic-400").is_empty());
    assert!(nodes.iter().all(|n| n.regs[0][0] != 0x0300_0000 || n.compatible() != b"snps,dw-apb-uart"));
    assert!(nodes.count() > 20 && nodes.left_out() == 0, "{} nodes, {} left out", nodes.count(), nodes.left_out());
}

#[test]
fn the_allwinner_boards_consoles_and_the_uarts_beside_them() {
    // H5 (Orange Pi Zero Plus 2): UART1, the AP6212's Bluetooth, is enabled in UART0's page, so neither is lent by it.
    let nodes = table(ZERO_PLUS2);
    let uarts = named(&nodes, "snps,dw-apb-uart");
    let bases: Vec<u64> = uarts.iter().map(|(_, n)| n.regs[0][0]).collect();
    assert_eq!(bases, [0x01c2_8000, 0x01c2_8400]);
    let (console, uart0) = uarts[0];
    assert_eq!((uart0.flags & NODE_CONSOLE, &uart0.lines[..uart0.line_count as usize]), (NODE_CONSOLE, &[0u32][..]));
    assert_eq!(nodes.pages(console, 0), Some((0x01c2_8000, 0x1000)));
    assert_eq!(nodes.sharer(console, 0), Some(uarts[1].0));
    assert!(named(&nodes, "arm,gic-400").is_empty() && nodes.left_out() == 0);
    // H618 (Orange Pi Zero 3): UART1 to UART5 are disabled in its tree, so the console's page is UART0's alone. init
    // still asks for the kernel's channel there: it lends a console's node only when it is a PL011 (hardware.rs).
    let nodes = table(ZERO3);
    let uarts = named(&nodes, "snps,dw-apb-uart");
    assert_eq!(uarts.iter().map(|(_, n)| n.regs[0][0]).collect::<Vec<_>>(), [0x0500_0000]);
    let (console, uart0) = uarts[0];
    assert_eq!((uart0.flags & NODE_CONSOLE, &uart0.lines[..uart0.line_count as usize]), (NODE_CONSOLE, &[0u32][..]));
    assert_eq!((nodes.pages(console, 0), nodes.sharer(console, 0)), (Some((0x0500_0000, 0x1000)), None));
    assert!(named(&nodes, "arm,gic-400").is_empty() && nodes.left_out() == 0);
}

#[test]
fn the_table_keeps_its_bounds() {
    let mut nodes = Nodes::new();
    // A compatible string longer than a record keeps is cut, and the record says nothing more of it.
    let long = [b'x'; 60];
    assert!(nodes.add(&long, 0, &[(0x1000, 0x10)], &[]));
    assert_eq!(nodes.get(0).unwrap().compatible().len(), abi::NODE_COMPATIBLE);
    // More ranges and lines than a record keeps: the first ones, and NODE_MORE.
    let regs: Vec<(u64, u64)> = (0..6).map(|i| (0x10_0000 + i * 0x1000, 0x100)).collect();
    assert!(nodes.add(b"many", 0, &regs, &[1, 2, 3, 4, 5]));
    let many = nodes.get(1).unwrap();
    assert_eq!((many.reg_count as usize, many.line_count, many.flags & NODE_MORE), (NODE_REGS, 4, NODE_MORE));
    // An empty range has no pages; a range that wraps the address space has none either; past a node's ranges, none.
    assert!(nodes.add(b"empty", 0, &[(0x20_0000, 0)], &[]));
    assert_eq!(nodes.pages(2, 0), None);
    assert!(nodes.add(b"wraps", 0, &[(u64::MAX - 0x10, 0x100)], &[]));
    assert_eq!(nodes.pages(3, 0), None);
    assert_eq!(nodes.pages(1, NODE_REGS), None);
    assert_eq!(nodes.pages(99, 0), None);
    assert_eq!(nodes.line(1, 3), Some(4));
    assert_eq!(nodes.line(1, 4), None);
    // A range across a page boundary covers both pages.
    assert!(nodes.add(b"across", 0, &[(0x30_0ff0, 0x20)], &[]));
    assert_eq!(nodes.pages(4, 0), Some((0x30_0000, 0x2000)));
    // The table is full at PLATFORM_NODES_MAX; what does not fit is counted.
    while nodes.count() < PLATFORM_NODES_MAX { assert!(nodes.add(b"filler", 0, &[(0x4000_0000 + nodes.count() as u64 * 0x1000, 0x1000)], &[])); }
    assert!(!nodes.add(b"one more", 0, &[(0x9000_0000, 0x1000)], &[]));
    assert!(!nodes.add(b"and another", 0, &[(0x9000_1000, 0x1000)], &[]));
    assert_eq!((nodes.count(), nodes.left_out()), (PLATFORM_NODES_MAX, 2));
}

#[test]
fn the_table_reads_back_as_handed_out() {
    let nodes = table(VIRT_V2);
    let bytes = nodes.as_bytes();
    assert_eq!(bytes.len(), 8 + nodes.count() * core::mem::size_of::<PlatformNode>());
    assert_eq!(core::mem::size_of::<PlatformNode>(), 128);
    let (header, records) = nodes::read(bytes).expect("the table");
    let records: Vec<PlatformNode> = records.collect();
    assert_eq!((header.count as usize, records.len(), header.left_out), (nodes.count(), nodes.count(), 0));
    assert!(records.iter().zip(nodes.iter()).all(|(a, b)| a.compatible() == b.compatible() && a.regs == b.regs && a.lines == b.lines && a.flags == b.flags));
    // A table shorter than its header says is refused; so is one shorter than a header.
    assert!(nodes::read(&bytes[..bytes.len() - 1]).is_none());
    assert!(nodes::read(&bytes[..4]).is_none());
}
