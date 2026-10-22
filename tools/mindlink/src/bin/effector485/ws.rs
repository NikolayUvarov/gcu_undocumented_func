//! WebSocket enough for effector485's page (RFC 6455): the handshake's answer, the server's frames, and the page's
//! masked frames taken from a buffer, so that a read's timeout never cuts one.
use std::io::Write;
use std::net::TcpStream;

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
/// The largest frame taken from the page: its messages are a few words.
const PAGE_FRAME_MAX: usize = 4096;

pub const TEXT: u8 = 1;
pub const CLOSE: u8 = 8;
pub const PONG: u8 = 10;

/// SHA-1, which the handshake alone needs (FIPS 180-4).
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let mut m = data.to_vec();
    m.push(0x80);
    while m.len() % 64 != 56 { m.push(0); }
    m.extend_from_slice(&(data.len() as u64 * 8).to_be_bytes());
    for block in m.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 { w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]); }
        for i in 16..80 { w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1); }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, t);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) { *x = x.wrapping_add(y); }
    }
    let mut out = [0u8; 20];
    for (i, x) in h.iter().enumerate() { out[4 * i..4 * i + 4].copy_from_slice(&x.to_be_bytes()); }
    out
}

fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 { out.push(if i <= c.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' }); }
    }
    out
}

/// Sec-WebSocket-Accept for the page's Sec-WebSocket-Key.
pub fn accept(key: &str) -> String { base64(&sha1(format!("{key}{GUID}").as_bytes())) }

pub fn handshake(stream: &mut TcpStream, key: &str) -> std::io::Result<()> {
    let answer = format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n", accept(key));
    stream.write_all(answer.as_bytes())
}

/// One whole server frame: unmasked, as RFC 6455 has a server's.
pub fn frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = vec![0x80 | opcode];
    match payload.len() {
        n if n < 126 => f.push(n as u8),
        n if n < 65_536 => { f.push(126); f.extend_from_slice(&(n as u16).to_be_bytes()); }
        n => { f.push(127); f.extend_from_slice(&(n as u64).to_be_bytes()); }
    }
    f.extend_from_slice(payload);
    f
}

pub enum Message { Text(String), Ping(Vec<u8>), Close, Other }

/// The first whole frame in `buf`, taken out of it; None until one has arrived. An error for what a page never sends:
/// an unmasked frame, a fragment, or one too large.
pub fn next(buf: &mut Vec<u8>) -> Result<Option<Message>, &'static str> {
    if buf.len() < 2 { return Ok(None); }
    let (b0, b1) = (buf[0], buf[1]);
    if b1 & 0x80 == 0 { return Err("an unmasked frame from the page"); }
    if b0 & 0x80 == 0 || (b0 & 0x0f == 0) { return Err("a fragmented frame from the page"); }
    let (len, at) = match b1 & 0x7f {
        126 if buf.len() < 4 => return Ok(None),
        126 => (u16::from_be_bytes([buf[2], buf[3]]) as usize, 4),
        127 => return Err("a frame too large"),
        n => (n as usize, 2),
    };
    if len > PAGE_FRAME_MAX { return Err("a frame too large"); }
    if buf.len() < at + 4 + len { return Ok(None); }
    let mask = [buf[at], buf[at + 1], buf[at + 2], buf[at + 3]];
    let payload: Vec<u8> = buf[at + 4..at + 4 + len].iter().enumerate().map(|(i, b)| b ^ mask[i % 4]).collect();
    buf.drain(..at + 4 + len);
    Ok(Some(match b0 & 0x0f {
        1 => Message::Text(String::from_utf8_lossy(&payload).into_owned()),
        8 => Message::Close,
        9 => Message::Ping(payload),
        _ => Message::Other,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_and_the_rfc_example() {
        let hex: String = sha1(b"abc").iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(accept("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn a_page_frame_split_anywhere() {
        let (mask, text) = ([1u8, 2, 3, 4], b"{\"follow\":\"e485-1\"}");
        let mut whole = vec![0x81, 0x80 | text.len() as u8];
        whole.extend_from_slice(&mask);
        whole.extend(text.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        for cut in 0..whole.len() {
            let mut buf = whole[..cut].to_vec();
            assert!(matches!(next(&mut buf), Ok(None)), "cut at {cut}");
            buf.extend_from_slice(&whole[cut..]);
            assert!(matches!(next(&mut buf), Ok(Some(Message::Text(t))) if t.as_bytes() == text));
            assert!(buf.is_empty());
        }
        assert!(next(&mut vec![0x81, 0x05, b'h', b'e', b'l', b'l', b'o']).is_err());
    }
}
