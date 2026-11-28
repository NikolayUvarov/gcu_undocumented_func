// MODE SENSE(6) for all pages (SPC-4, 6.11; the caching page, SBC-3, 6.4.5) as usb_storage reads it once at the start:
// write protection from the header, and whether the device's write cache is on (WCE) from its caching page, when the
// answer holds one (307-DRV-0055). Built on the host for tests (tests/flush_host.rs).

/// All pages (3Fh): first the 4-byte header, then at most 192 bytes, the length Linux and Windows ask of a USB disk.
pub const HEADER: usize = 4;
pub const LENGTH: usize = 192;
pub const fn all_pages(length: usize) -> [u8; 6] { [0x1A, 0, 0x3F, 0, length as u8, 0] }

/// The whole answer's length the header names (its mode data length and itself), at most LENGTH: asked exactly, so
/// that no data stage ends short, which a bridge may end in a stall (the Ryzen's JMicron JM20329, bench-54).
pub fn whole(header: &[u8]) -> usize { header.first().map_or(HEADER, |&n| (n as usize + 1).clamp(HEADER, LENGTH)) }

/// The header's WP bit (byte 2, bit 7), None in an answer shorter than the header.
pub fn protected(answer: &[u8]) -> Option<bool> { (answer.len() >= 4).then(|| answer[2] & 0x80 != 0) }

/// The caching page's (08h) WCE bit (byte 2, bit 2) in `answer`, the bytes the device sent; None where the answer, cut
/// at its mode data length, holds no caching page as far as that byte.
pub fn write_cache(answer: &[u8]) -> Option<bool> {
    if answer.len() < 4 { return None; }
    let end = answer.len().min(answer[0] as usize + 1);
    let mut at = 4 + answer[3] as usize; // past the block descriptors
    while at + 2 <= end {
        let (code, subpage) = (answer[at] & 0x3F, answer[at] & 0x40 != 0);
        let size = if subpage {
            if at + 4 > end { return None; }
            4 + u16::from_be_bytes([answer[at + 2], answer[at + 3]]) as usize
        } else { 2 + answer[at + 1] as usize };
        if code == 0x08 && !subpage { return (at + 3 <= end && answer[at + 1] >= 1).then(|| answer[at + 2] & 0x04 != 0); }
        at += size;
    }
    None
}
