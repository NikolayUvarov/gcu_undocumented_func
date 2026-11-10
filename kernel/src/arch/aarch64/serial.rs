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

// The console as a byte channel (100000-KRN-0222): its received bytes, room to send one, and its interrupts.
pub unsafe fn console_take(out: &mut [u8]) -> usize {
    let (uart, is16550) = (board::get(&board::UART), board::get(&board::UART_KIND) == board::UART_16550);
    // A DesignWare UART's busy-detect interrupt (IIR 7: LCR written while it was busy, by the firmware, as the kernel
    // never writes it) holds the line until USR, register 31, is read. A plain 16550's IIR never reads 7.
    if uart != 0 && is16550 && read_16550(uart, 2) & 0x0F == 0x07 { read_16550(uart, 31); }
    let mut n = 0;
    // 16550: LSR bit 0, data ready; PL011: FR bit 4, the receiver is empty.
    while uart != 0 && n < out.len() && if is16550 { read_16550(uart, 5) & 1 != 0 } else { read_volatile((uart + 0x18) as *const u32) & 0x10 == 0 } {
        out[n] = if is16550 { read_16550(uart, 0) } else { read_volatile(uart as *const u32) as u8 }; n += 1;
    }
    n
}
// 16550: LSR bit 5, the holding register is empty; PL011: FR bit 5, the transmit FIFO is full.
pub unsafe fn console_room() -> bool {
    let uart = board::get(&board::UART);
    uart != 0 && if board::get(&board::UART_KIND) == board::UART_16550 { read_16550(uart, 5) & 0x20 != 0 } else { read_volatile((uart + 0x18) as *const u32) & 0x20 == 0 }
}
// Receive (and its timeout) and transmit interrupts on or off: 16550 IER bits 0 and 1, PL011 IMSC bits 4, 6 and 5. The
// PL011's transmit interrupt is cleared too, as QEMU's sends at once and keeps it up; a pending receive one never is: QEMU
// raises it again only when its FIFO goes from empty to one byte, and the bytes would stay there.
pub unsafe fn console_interrupts(rx: bool, tx: bool) {
    let uart = board::get(&board::UART);
    if uart == 0 { return; }
    if board::get(&board::UART_KIND) == board::UART_16550 { write_16550(uart, 1, rx as u8 | (tx as u8) << 1); return; }
    write_volatile((uart + 0x44) as *mut u32, if rx { 0x20 } else { 0x7FF });
    write_volatile((uart + 0x38) as *mut u32, if rx { 0x50 } else { 0 } | if tx { 0x20 } else { 0 });
}
