// The boot and diagnostic serial line: COM1, a 16550 UART.
use super::port::{inb, outb};

const COM1: u16 = 0x3F8;
pub unsafe fn init_serial() {
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x80);
    outb(COM1 + 0, 0x03);
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);
    outb(COM1 + 2, 0xC7);
    outb(COM1 + 4, 0x0B);
}
unsafe fn serial_is_transmit_empty() -> bool {
    (inb(COM1 + 5) & 0x20) != 0
}
pub unsafe fn serial_write_byte(b: u8) {
    while !serial_is_transmit_empty() {}
    outb(COM1, b);
}
// No console channel here (100000-KRN-0222): COM1 is lent by its ports.
pub unsafe fn console_take(_out: &mut [u8]) -> usize { 0 }
pub unsafe fn console_room() -> bool { false }
pub unsafe fn console_interrupts(_rx: bool, _tx: bool) {}
