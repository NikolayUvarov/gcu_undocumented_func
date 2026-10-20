//! Consistent Overhead Byte Stuffing: a frame holds no zero byte, so a zero separates frames on the line.

/// `data` COBS-encoded into `out`, without the separating zero; its length, or None when `out` is too small.
pub fn encode(data: &[u8], out: &mut [u8]) -> Option<usize> {
    let (mut code_at, mut o, mut code) = (0usize, 1usize, 1u8);
    if out.is_empty() { return None; }
    for &b in data {
        if b != 0 {
            *out.get_mut(o)? = b;
            o += 1;
            code += 1;
        }
        if b == 0 || code == 0xFF {
            *out.get_mut(code_at)? = code;
            code_at = o;
            o += 1;
            code = 1;
        }
    }
    *out.get_mut(code_at)? = code;
    Some(o)
}

/// COBS-encoded `data` (no zero in it) decoded into `out`; its length, or None when it is malformed or `out` is small.
pub fn decode(data: &[u8], out: &mut [u8]) -> Option<usize> {
    let (mut i, mut o) = (0usize, 0usize);
    while i < data.len() {
        let code = data[i] as usize;
        if code == 0 { return None; }
        let end = i + code;
        if end > data.len() { return None; }
        let n = code - 1;
        out.get_mut(o..o + n)?.copy_from_slice(&data[i + 1..end]);
        o += n;
        i = end;
        if code < 0xFF && i < data.len() {
            *out.get_mut(o)? = 0;
            o += 1;
        }
    }
    Some(o)
}

/// The most bytes COBS turns `n` bytes into.
pub const fn max_encoded(n: usize) -> usize { n + n / 254 + 1 }
