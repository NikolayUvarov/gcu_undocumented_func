//! 24-bit BMP for `screenshot` (issue 086): a 54-byte header (BITMAPFILEHEADER + BITMAPINFOHEADER) and rows of BGR
//! bytes, bottom row first, each padded to 4 bytes. Host-tested in tests/bmp_host.rs.

/// Bytes of the headers before the pixels.
pub const HEADER: usize = 54;

/// Bytes of one row of `width` pixels, with the padding.
pub const fn row_bytes(width: usize) -> usize { (width * 3 + 3) & !3 }

/// Size of the whole file.
pub const fn file_bytes(width: usize, height: usize) -> usize { HEADER + row_bytes(width) * height }

/// The headers of a `width` x `height` picture.
pub fn header(width: usize, height: usize) -> [u8; HEADER] {
    let mut out = [0u8; HEADER];
    let mut put = |at: usize, bytes: &[u8]| out[at..at + bytes.len()].copy_from_slice(bytes);
    put(0, b"BM");
    put(2, &(file_bytes(width, height) as u32).to_le_bytes());
    put(10, &(HEADER as u32).to_le_bytes());
    put(14, &40u32.to_le_bytes()); // BITMAPINFOHEADER
    put(18, &(width as i32).to_le_bytes());
    put(22, &(height as i32).to_le_bytes()); // positive: bottom-up
    put(26, &1u16.to_le_bytes()); // planes
    put(28, &24u16.to_le_bytes()); // bits per pixel
    put(34, &((row_bytes(width) * height) as u32).to_le_bytes()); // image size (no compression)
    put(38, &2835u32.to_le_bytes()); // 72 DPI
    put(42, &2835u32.to_le_bytes());
    out
}

/// One row of `0x00RRGGBB` pixels as BGR bytes with the padding; `out` must hold `row_bytes(pixels.len())`.
pub fn row(pixels: &[u32], out: &mut [u8]) -> usize {
    let len = row_bytes(pixels.len());
    for (i, &p) in pixels.iter().enumerate() { out[i * 3..i * 3 + 3].copy_from_slice(&[p as u8, (p >> 8) as u8, (p >> 16) as u8]); }
    out[pixels.len() * 3..len].fill(0);
    len
}
