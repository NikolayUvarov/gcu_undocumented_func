// Platform resources init may hand to drivers outside PCI (PLATFORM_MMIO by index, PLATFORM_IRQ): the console UART and
// the RTC of the board (board.rs: from ACPI, or QEMU `virt`'s PL011 at SPI 1 and PL031 at SPI 2), 4 KiB of
// registers each, and the pin controllers the DSDT and SSDTs name (issue 206). There are no I/O ports.
use super::board;

pub const PORTS: [(u16, u16); 0] = [];
/// Registers of platform device `index` (PLATFORM_UART, PLATFORM_RTC, PLATFORM_PINS_*): (base, bytes), if the board has it.
pub fn mmio(index: usize) -> Option<(usize, usize)> {
    use crate::abi::{PLATFORM_PINS_BCM2711, PLATFORM_PINS_MAX, PLATFORM_PINS_PL061};
    let pins = |slot: usize| { let [base, size] = &board::PINS[slot]; (board::get(base), board::get(size).div_ceil(0x1000) * 0x1000) };
    let (base, size) = match index {
        crate::abi::PLATFORM_UART => (board::get(&board::UART), 0x1000), crate::abi::PLATFORM_RTC => (board::get(&board::RTC), 0x1000),
        crate::abi::PLATFORM_TPM => (board::get(&board::TPM), 0x1000),
        n if (PLATFORM_PINS_PL061..PLATFORM_PINS_PL061 + PLATFORM_PINS_MAX).contains(&n) => pins(n - PLATFORM_PINS_PL061),
        n if (PLATFORM_PINS_BCM2711..PLATFORM_PINS_BCM2711 + PLATFORM_PINS_MAX).contains(&n) => pins(board::PINS_BCM2711 + n - PLATFORM_PINS_BCM2711),
        _ => (0, 0),
    };
    (base != 0 && size != 0).then_some((base, size))
}
/// The kernel's own console (logs stop going there once it is handed out).
pub fn console() -> usize { board::get(&board::UART) }
pub fn irq(line: usize) -> bool { line == board::get(&board::UART_LINE) || line == board::get(&board::RTC_LINE) }
