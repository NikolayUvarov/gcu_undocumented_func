// The FTDI FT232R's requests, facts from FTDI's application notes AN232B-04 (latency, status bytes) and AN232B-05 (baud
// rates): vendor requests to the device, and two status bytes at the start of every packet it sends.

pub const RESET: u8 = 0;
pub const MODEM_CTRL: u8 = 1;
pub const SET_FLOW_CTRL: u8 = 2;
pub const SET_BAUD_RATE: u8 = 3;
pub const SET_DATA: u8 = 4;
pub const SET_LATENCY_TIMER: u8 = 9;
/// RESET's values: the chip, its receive buffer, its transmit buffer.
pub const RESET_SIO: u16 = 0;
pub const PURGE_RX: u16 = 1;
pub const PURGE_TX: u16 = 2;
/// DTR and RTS on, each with its enable bit.
pub const DTR_RTS_ON: u16 = 0x0303;
/// 8 data bits, no parity, one stop bit.
pub const DATA_8N1: u16 = 8;
/// The line status byte's error bits: overrun, parity, framing, break.
pub const LINE_ERRORS: u8 = 0x1E;

/// The base clock divided: 3 MHz over a divisor of 1/8 steps, with 0 and 1 standing for 3 and 2 Mbit/s.
const BASE8: u64 = 24_000_000;
/// The sub-integer part (eighths) as the chip codes it in bits 14-16 of the divisor.
const EIGHTHS: [u32; 8] = [0, 3, 2, 4, 1, 5, 6, 7];

/// The SET_BAUD_RATE request's value and index for `baud`, and the rate the chip makes; None beyond 3 % or out of range.
pub fn divisor(baud: u32) -> Option<(u16, u16, u32)> {
    if baud == 0 { return None; }
    let rate = |eighths: u64| ((BASE8 + eighths / 2) / eighths) as u32;
    let off = |actual: u32| (actual as i64 - baud as i64).unsigned_abs();
    let (code, actual) = if baud > 2_500_000 { (0, 3_000_000) } else if baud > 1_750_000 { (1, 2_000_000) } else {
        // The nearer rate of the two divisors around the exact one; 2.0 is the least below 2 Mbit/s.
        let low = (BASE8 / baud as u64).max(16);
        let eighths = if off(rate(low + 1)) < off(rate(low)) { low + 1 } else { low };
        if eighths >> 3 > 0x3FFF { return None; }
        ((eighths >> 3) as u32 | EIGHTHS[(eighths & 7) as usize] << 14, rate(eighths))
    };
    if off(actual) * 100 > baud as u64 * 3 { return None; }
    Some((code as u16, (code >> 16) as u16, actual))
}

/// The data of packets received in one piece, each led by its two status bytes: written to `out`, the bytes written,
/// and whether the line status reported an overrun or another error.
pub fn strip(packet: &[u8], out: &mut [u8]) -> (usize, bool) {
    let Some(data) = packet.get(2..) else { return (0, false) };
    let n = data.len().min(out.len());
    out[..n].copy_from_slice(&data[..n]);
    (n, packet[1] & LINE_ERRORS != 0)
}
