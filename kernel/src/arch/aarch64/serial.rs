// The boot and diagnostic serial line: the PL011 UART the SPCR names (QEMU `virt`'s by default), or the console a device
// tree's `stdout-path` names, a PL011 or a 16550 in memory (100000-PRT-0010: the A64's DesignWare UART). The firmware
// has set its speed and format up; the kernel only sends.
use super::board;
use core::ptr::{read_volatile, write_volatile};

pub unsafe fn init_serial() {}

// A 16550's register `n`, read or written `width` bytes wide at `n << shift`.
unsafe fn read_16550(base: usize, n: usize) -> u8 {
    let at = base + (n << board::get(&board::UART_SHIFT));
    match board::get(&board::UART_WIDTH) { 4 => read_volatile(at as *const u32) as u8, 2 => read_volatile(at as *const u16) as u8, _ => read_volatile(at as *const u8) }
}
unsafe fn write_16550(base: usize, n: usize, value: u8) {
    let at = base + (n << board::get(&board::UART_SHIFT));
    match board::get(&board::UART_WIDTH) { 4 => write_volatile(at as *mut u32, value as u32), 2 => write_volatile(at as *mut u16, value as u16), _ => write_volatile(at as *mut u8, value) }
}

pub unsafe fn serial_write_byte(b: u8) {
    let uart = board::get(&board::UART);
    if uart == 0 { return; }
    if board::get(&board::UART_KIND) == board::UART_16550 {
        // LSR (5) bit 5: the holding register is empty. Bounded, so a UART that never empties cannot hang the kernel.
        for _ in 0..1_000_000 { if read_16550(uart, 5) & 0x20 != 0 { break; } core::hint::spin_loop(); }
        write_16550(uart, 0, b);
        return;
    }
    while read_volatile((uart + 0x18) as *const u32) & 0x20 != 0 {} // TXFF
    write_volatile(uart as *mut u32, b as u32);
}
