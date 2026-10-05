// The boot and diagnostic serial line: the PL011 UART the SPCR names (QEMU `virt`'s by default; UEFI has set it up).
use super::board;

pub unsafe fn init_serial() {}
pub unsafe fn serial_write_byte(b: u8) {
    let uart = board::get(&board::UART);
    if uart == 0 { return; }
    while core::ptr::read_volatile((uart + 0x18) as *const u32) & 0x20 != 0 {} // TXFF
    core::ptr::write_volatile(uart as *mut u32, b as u32);
}
