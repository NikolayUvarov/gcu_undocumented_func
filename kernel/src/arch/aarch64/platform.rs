// Platform resources init may hand to drivers outside PCI (PLATFORM_MMIO, PLATFORM_IRQ): on `virt` the PL011 UART
// (SPI 1) and the PL031 RTC (SPI 2), 4 KiB of registers each. There are no I/O ports.
pub const PORTS: [(u16, u16); 0] = [];
pub const MMIO: [(usize, usize); 2] = [(CONSOLE, 0x1000), (0x0901_0000, 0x1000)];
pub const CONSOLE: usize = 0x0900_0000; // the PL011 the kernel prints on
pub fn irq(line: usize) -> bool { matches!(line, 1 | 2) }
