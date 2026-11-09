// Platform resources init may hand to drivers outside PCI: the platform's nodes (100000-KRN-0171), from the device tree
// (devicetree.rs), or on a board with ACPI made here from board.rs's record: the console UART (the SPCR's) and the RTC
// (QEMU `virt`'s PL011 at SPI 1 and PL031 at SPI 2), the pin controllers the DSDT and SSDTs name (issue 206) and the TPM.
// There are no I/O ports, and PLATFORM_IRQ takes no line: a node's lines are PLATFORM_NODE_IRQ's.
use super::board;
use crate::abi::NODE_CONSOLE;

pub const PORTS: [(u16, u16); 0] = [];
pub fn ports(_start: usize, _end: usize) -> bool { false }
/// The table from board.rs's record, when the device tree did not fill it.
pub fn nodes() {
    let line = |at: &core::sync::atomic::AtomicUsize| [board::get(at) as u32];
    unsafe { crate::platform_nodes::build(|table| {
        let uart = board::get(&board::UART);
        if uart != 0 {
            let name: &[u8] = if board::get(&board::UART_KIND) == board::UART_PL011 { b"arm,pl011" } else { b"ns16550a" };
            table.add(name, NODE_CONSOLE, &[(uart as u64, 0x1000)], &line(&board::UART_LINE));
        }
        let rtc = board::get(&board::RTC);
        if rtc != 0 { table.add(b"arm,pl031", 0, &[(rtc as u64, 0x1000)], &line(&board::RTC_LINE)); }
        for (slot, [base, size]) in board::PINS.iter().enumerate() {
            let (base, size) = (board::get(base), board::get(size));
            if base == 0 || size == 0 { continue; }
            let name: &[u8] = if slot < board::PINS_BCM2711 { b"arm,pl061" } else { b"brcm,bcm2711-gpio" };
            table.add(name, 0, &[(base as u64, size as u64)], &[]);
        }
        let tpm = board::get(&board::TPM);
        if tpm != 0 { table.add(b"mind,tpm2", 0, &[(tpm as u64, 0x1000)], &[]); }
    }); }
}
/// Whether physical `start..end` holds registers the kernel drives itself: the GIC's (a GICv3 redistributor of 128 KiB
/// for each CPU), its MSI frame or ITS, the ECAM.
pub fn kernel_owns(start: u64, end: u64) -> bool {
    let at = |base: usize, bytes: u64| base != 0 && (base as u64) < end && base as u64 + bytes > start;
    let cpus = super::acpi::CPU_COUNT.load(core::sync::atomic::Ordering::Relaxed).max(1) as u64;
    let gic = if board::get(&board::GIC_VERSION) == 2 { at(board::get(&board::GICC), 0x1_0000) || at(board::get(&board::V2M), 0x1_0000) }
              else { at(board::get(&board::GICR), 0x2_0000 * cpus) || at(board::get(&board::GITS), 0x2_0000) };
    gic || at(board::get(&board::GICD), 0x1_0000) || super::pcicfg::ecam().is_some_and(|(from, to)| from < end && to > start)
}
/// Whether a node's line `n` is one the kernel can bind: a device line, SPI 0..15 (line n is interrupt 32 + n).
pub fn node_line(line: u32) -> bool { (line as usize) < super::context::MSI_FIRST }
/// The kernel's own console (logs stop going there once it is handed out).
pub fn console() -> usize { board::get(&board::UART) }
/// No line by number: a node's lines are lent through its node (PLATFORM_NODE_IRQ).
pub fn irq(_line: usize) -> bool { false }
/// A board without ACPI: its record from the device tree at `address` (100000-PRT-0009); false when there is none to read.
pub unsafe fn board_from_tree(address: u64) -> bool { super::devicetree::init(address) }
/// RAM the board reserves though the firmware's map may call it free: start and end of each range (the device tree's).
pub fn reserved(mut each: impl FnMut(u64, u64)) {
    for [start, end] in &board::RESERVED { let (s, e) = (start.load(core::sync::atomic::Ordering::Relaxed), end.load(core::sync::atomic::Ordering::Relaxed)); if e > s { each(s, e); } }
}
