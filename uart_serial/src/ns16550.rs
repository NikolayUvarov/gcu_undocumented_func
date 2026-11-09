// The 16550's registers at its ports (a PC's COM1) and its rate: the divisor of a 1.8432 MHz clock over 16.

/// Receive and transmit holding registers; with DLAB, the divisor's low byte.
pub const DATA: u16 = 0;
/// Interrupt enable: bit 0, a byte received; with DLAB, the divisor's high byte.
pub const INTERRUPTS: u16 = 1;
pub const RECEIVED_IRQ: u8 = 0x01;
/// FIFO control: on, both cleared, an interrupt at the first byte.
pub const FIFO: u16 = 2;
pub const FIFO_ON: u8 = 0x07;
/// Line control: 8 data bits, no parity, one stop bit; DLAB opens the divisor.
pub const LINE: u16 = 3;
pub const LINE_8N1: u8 = 0x03;
pub const DLAB: u8 = 0x80;
/// Modem control: DTR, RTS and OUT2, which a PC needs to pass the line's interrupt on.
pub const MODEM: u16 = 4;
pub const MODEM_ON: u8 = 0x0B;
/// Line status: a byte ready, an overrun, the transmit register empty.
pub const STATUS: u16 = 5;
pub const READY: u8 = 0x01;
pub const OVERRUN: u8 = 0x02;
pub const SENDABLE: u8 = 0x20;

/// The rate the divisor makes for each 115200 over it.
pub const BASE: u32 = 115_200;

/// The divisor for `baud` and the rate it makes, within 3 % of the one asked; None beyond.
pub fn divisor(baud: u32) -> Option<(u16, u32)> {
    if baud == 0 || baud > BASE { return None; }
    let divisor = (BASE + baud / 2) / baud;
    let divisor = u16::try_from(divisor).ok().filter(|&d| d >= 1)?;
    let actual = BASE / divisor as u32;
    (actual.abs_diff(baud) as u64 * 100 <= baud as u64 * 3).then_some((divisor, actual))
}
