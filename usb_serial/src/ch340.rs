// The WCH CH340's requests (800-LNK-0002), as WCH's own serial driver uses them: vendor requests to the device, a
// version, a serial reset, registers written in pairs, and the modem lines. Its bulk data carry no status bytes.

pub const READ_VERSION: u8 = 0x5F;
pub const SERIAL_INIT: u8 = 0xA1;
pub const WRITE_REGISTERS: u8 = 0x9A;
pub const MODEM_CONTROL: u8 = 0xA4;
/// The register pairs: the rate's prescaler (0x12) and divisor (0x13); the line control (0x18) and its second (0x25).
pub const RATE_REGISTERS: u16 = 0x1312;
pub const LINE_REGISTERS: u16 = 0x2518;
/// Receive and transmit on, 8 data bits; no parity, one stop bit in the second register.
pub const LINE_8N1: u16 = 0x00C3;
/// DTR and RTS on: the modem request carries the lines inverted (DTR bit 5, RTS bit 6).
pub const DTR_RTS_ON: u16 = !0x60u16 & 0xFF;
/// In the rate's low byte: send a part-filled packet at once rather than wait for 32 bytes (chips after version 0x27).
pub const NO_WAIT: u16 = 0x80;

const CLOCK: u64 = 48_000_000;

/// The rate registers' value for `baud` (prescaler `ps` 0-3 and `fact` in the low byte, 256 minus the divisor in the
/// high one; the rate is 48 MHz / (2^(12 - 3 ps - fact) * divisor)), and the rate the chip makes; None beyond 3 %.
pub fn divisor(baud: u32) -> Option<(u16, u32)> {
    if baud == 0 { return None; }
    let mut best: Option<(u64, u16, u32)> = None;
    for ps in (0..=3u32).rev() {
        for fact in [1u32, 0] {
            let base = CLOCK >> (12 - 3 * ps - fact);
            let div = (base + baud as u64 / 2) / baud as u64;
            // A divisor from 2 to 255; with the higher base clock, from 9, and odd (an even one is the lower clock's).
            if !(2..=255).contains(&div) || fact == 1 && (div < 9 || div % 2 == 0) { continue; }
            let actual = ((base + div / 2) / div) as u32;
            let off = (actual as i64 - baud as i64).unsigned_abs();
            if best.is_none_or(|(o, _, _)| off < o) { best = Some((off, ((256 - div as u32) << 8 | fact << 2 | ps) as u16, actual)); }
        }
    }
    let (off, value, actual) = best?;
    (off * 100 <= baud as u64 * 3).then_some((value, actual))
}
