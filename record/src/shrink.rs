//! A frame made smaller for `record -s` (000-APP-0089): each `scale` × `scale` block of pixels averaged into one, per
//! colour. No system calls: tests/record_host.rs.
use alloc::vec::Vec;

/// `width` × `height` pixels, `stride` apart from row to row, shrunk by `scale` into `out` (`width / scale` ×
/// `height / scale`, rows packed); the pixels at the right and bottom that make no whole block are left out.
pub fn shrink(pixels: &[u32], width: usize, height: usize, stride: usize, scale: usize, out: &mut Vec<u32>) {
    let (columns, rows) = (width / scale, height / scale);
    out.clear();
    out.reserve(columns * rows);
    let count = (scale * scale) as u32;
    for row in 0..rows {
        for column in 0..columns {
            let (mut red, mut green, mut blue) = (0u32, 0u32, 0u32);
            for y in row * scale..(row + 1) * scale {
                for &pixel in &pixels[y * stride + column * scale..y * stride + (column + 1) * scale] {
                    red += pixel >> 16 & 0xFF; green += pixel >> 8 & 0xFF; blue += pixel & 0xFF;
                }
            }
            out.push((red + count / 2) / count << 16 | (green + count / 2) / count << 8 | (blue + count / 2) / count);
        }
    }
}
