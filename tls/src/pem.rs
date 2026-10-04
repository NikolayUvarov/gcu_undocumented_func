// Certificates from a PEM file (RFC 7468): the base64 between "-----BEGIN CERTIFICATE-----" and the matching END line.
use alloc::vec::Vec;

fn sextet(c: u8) -> Option<u32> {
    Some(match c { b'A'..=b'Z' => c - b'A', b'a'..=b'z' => c - b'a' + 26, b'0'..=b'9' => c - b'0' + 52, b'+' => 62, b'/' => 63, _ => return None } as u32)
}

/// Standard base64 with padding; whitespace is skipped. None on any other character or a bad length.
pub fn base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut bits, mut count, mut padding) = (0u32, 0, 0);
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' { padding += 1; bits <<= 6; } else if padding > 0 { return None; } else { bits = bits << 6 | sextet(c)?; }
        count += 1;
        if count == 4 {
            let bytes = [(bits >> 16) as u8, (bits >> 8) as u8, bits as u8];
            if padding > 2 { return None; }
            out.extend_from_slice(&bytes[..3 - padding]);
            bits = 0; count = 0;
        }
    }
    (count == 0).then_some(out)
}

/// Every certificate in `text` (DER), skipping blocks that do not decode.
pub fn certificates(text: &str) -> Vec<Vec<u8>> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(BEGIN) {
        let body = &rest[start + BEGIN.len()..];
        let Some(end) = body.find(END) else { break };
        if let Some(der) = base64(&body[..end]) { out.push(der); }
        rest = &body[end + END.len()..];
    }
    out
}
