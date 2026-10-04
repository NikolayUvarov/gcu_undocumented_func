//! Host test of the BMP writer of `screenshot` (shell/src/bmp.rs): the headers and the rows, read back by a decoder
//! written from the format description.
#![allow(dead_code)]
#[path = "../shell/src/bmp.rs"]
mod bmp;

fn u16_at(b: &[u8], at: usize) -> u16 { u16::from_le_bytes([b[at], b[at + 1]]) }
fn u32_at(b: &[u8], at: usize) -> u32 { u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) }

/// The file for a picture, as `screenshot` writes it: headers, then the rows from the bottom up.
fn file(pixels: &[u32], width: usize, height: usize) -> Vec<u8> {
    let mut out = bmp::header(width, height).to_vec();
    let mut row = vec![0u8; bmp::row_bytes(width)];
    for y in (0..height).rev() { let n = bmp::row(&pixels[y * width..(y + 1) * width], &mut row); out.extend_from_slice(&row[..n]); }
    out
}

/// Pixel (x, y) of a 24-bit bottom-up BMP as 0x00RRGGBB.
fn read(file: &[u8], x: usize, y: usize) -> u32 {
    let (offset, width, height) = (u32_at(file, 10) as usize, u32_at(file, 18) as usize, u32_at(file, 22) as usize);
    let stride = (width * 3 + 3) / 4 * 4;
    let at = offset + (height - 1 - y) * stride + x * 3;
    (file[at + 2] as u32) << 16 | (file[at + 1] as u32) << 8 | file[at] as u32
}

#[test]
fn headers_and_rows() {
    for (width, height) in [(1, 1), (3, 2), (4, 4), (5, 3), (1280, 2)] {
        let pixels: Vec<u32> = (0..width * height).map(|i| (i as u32).wrapping_mul(0x01_0203_07) & 0x00FF_FFFF).collect();
        let data = file(&pixels, width, height);
        assert_eq!(&data[..2], b"BM");
        assert_eq!(data.len(), bmp::file_bytes(width, height));
        assert_eq!(u32_at(&data, 2) as usize, data.len());
        assert_eq!((u32_at(&data, 10), u32_at(&data, 14)), (54, 40));
        assert_eq!((u32_at(&data, 18) as usize, u32_at(&data, 22) as usize), (width, height));
        assert_eq!((u16_at(&data, 26), u16_at(&data, 28), u32_at(&data, 30)), (1, 24, 0), "one plane, 24 bits, no compression");
        assert_eq!(u32_at(&data, 34) as usize, bmp::row_bytes(width) * height);
        assert_eq!(bmp::row_bytes(width) % 4, 0);
        for y in 0..height { for x in 0..width { assert_eq!(read(&data, x, y), pixels[y * width + x], "{}x{} at ({}, {})", width, height, x, y); } }
    }
    // Padding bytes are zero.
    let mut row = [0xAAu8; 16];
    assert_eq!(bmp::row(&[0x123456], &mut row), 4);
    assert_eq!(row[..4], [0x56, 0x34, 0x12, 0]);
}
