//! DNS A queries and answers (RFC 1035), and the Internet checksum. Answers are untrusted input: every length is checked.

/// A recursive A query for `name` with `id` into `out`; returns its length, None for an invalid name.
pub fn query(name: &str, id: u16, out: &mut [u8; 512]) -> Option<usize> {
    let name = name.trim_end_matches('.');
    if name.is_empty() || name.len() > 253 { return None; }
    out[..12].copy_from_slice(&[(id >> 8) as u8, id as u8, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]);
    let mut at = 12;
    for label in name.split('.') {
        if label.is_empty() || label.len() > 63 || !label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return None; }
        out[at] = label.len() as u8; out[at + 1..at + 1 + label.len()].copy_from_slice(label.as_bytes()); at += 1 + label.len();
    }
    out[at..at + 5].copy_from_slice(&[0, 0, 1, 0, 1]); // root, type A, class IN
    Some(at + 5)
}

// Skips a (possibly compressed) name; returns the offset after it.
fn skip_name(data: &[u8], mut at: usize) -> Option<usize> {
    for _ in 0..128 {
        let len = *data.get(at)? as usize;
        if len == 0 { return Some(at + 1); }
        if len & 0xC0 == 0xC0 { data.get(at + 1)?; return Some(at + 2); }
        if len > 63 { return None; }
        at += 1 + len;
    }
    None
}

/// The first A record of a response to `id`: None if `data` is not that response, Err(true) if the name does not
/// exist, Err(false) for a malformed response or one without an A record.
pub fn answer(data: &[u8], id: u16) -> Option<Result<u32, bool>> {
    if data.len() < 12 || u16::from_be_bytes([data[0], data[1]]) != id || data[2] & 0x80 == 0 { return None; }
    match data[3] & 0x0F { 0 => {} 3 => return Some(Err(true)), _ => return Some(Err(false)) }
    let (questions, answers) = (u16::from_be_bytes([data[4], data[5]]), u16::from_be_bytes([data[6], data[7]]));
    let mut at = 12;
    for _ in 0..questions { at = match skip_name(data, at) { Some(next) if next + 4 <= data.len() => next + 4, _ => return Some(Err(false)) }; }
    for _ in 0..answers {
        let Some(next) = skip_name(data, at) else { return Some(Err(false)) };
        if next + 10 > data.len() { return Some(Err(false)); }
        let (kind, class, len) = (u16::from_be_bytes([data[next], data[next + 1]]), u16::from_be_bytes([data[next + 2], data[next + 3]]), u16::from_be_bytes([data[next + 8], data[next + 9]]) as usize);
        let body = next + 10;
        if body + len > data.len() { return Some(Err(false)); }
        if kind == 1 && class == 1 && len == 4 { return Some(Ok(u32::from_be_bytes([data[body], data[body + 1], data[body + 2], data[body + 3]]))); }
        at = body + len;
    }
    Some(Err(false))
}

/// The Internet checksum (RFC 1071) of `data`.
pub fn checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = data.chunks(2).map(|pair| u16::from_be_bytes([pair[0], *pair.get(1).unwrap_or(&0)]) as u32).sum();
    while sum >> 16 != 0 { sum = (sum & 0xFFFF) + (sum >> 16); }
    !(sum as u16)
}
