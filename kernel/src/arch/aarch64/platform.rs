// Platform resources init may hand to drivers outside PCI (PLATFORM_MMIO by index, PLATFORM_IRQ): the console UART and
// the RTC of the board (board.rs: from ACPI, else QEMU `virt`'s PL011 at SPI 1 and PL031 at SPI 2), 4 KiB of
// registers each. There are no I/O ports.
use super::board;

pub const PORTS: [(u16, u16); 0] = [];
/// Registers of platform device `index` (PLATFORM_UART, PLATFORM_RTC): (base, bytes), if the board has it.
pub fn mmio(index: usize) -> Option<(usize, usize)> {
    let base = match index { crate::abi::PLATFORM_UART => board::get(&board::UART), crate::abi::PLATFORM_RTC => board::get(&board::RTC), _ => 0 };
    (base != 0).then_some((base, 0x1000))
}
/// The kernel's own console (logs stop going there once it is handed out).
pub fn console() -> usize { board::get(&board::UART) }
pub fn irq(line: usize) -> bool { line == board::get(&board::UART_LINE) || line == board::get(&board::RTC_LINE) }
