//! Video (issue 158): the synthetic test source the video gateway serves when the boot disk asks for it, and the
//! pixel conversion a camera's YUY2 frames need. No system calls: tests/video_host.rs; `video_gw` and `camera` use it.

/// The test pattern's eight bars, left to right: white, yellow, cyan, green, magenta, red, blue, black.
pub const BARS: [u32; 8] = [0x00FF_FFFF, 0x00FF_FF00, 0x0000_FFFF, 0x0000_FF00, 0x00FF_00FF, 0x00FF_0000, 0x0000_00FF, 0x0000_0000];
/// Rows at the bottom that carry the frame number: 32 cells, the most significant bit left, white for 1.
pub const COUNTER_ROWS: usize = 16;
/// How far the bars move per frame, in pixels (to the left).
pub const STEP: usize = 4;
/// The sizes the synthetic source gives: at least 64 × 32, at most 640 × 480, widths a multiple of 32.
pub const MAX_WIDTH: usize = 640;
pub const MAX_HEIGHT: usize = 480;
pub const MAX_RATE: u8 = 30;

/// Whether the synthetic source gives `width` × `height` at `rate` frames a second.
pub fn supported(width: usize, height: usize, rate: u8) -> bool {
    (64..=MAX_WIDTH).contains(&width) && width % 32 == 0 && (COUNTER_ROWS * 2..=MAX_HEIGHT).contains(&height) && (1..=MAX_RATE).contains(&rate)
}

/// The pixel of frame `sequence` at (x, y): the bars moved left by STEP a frame, and the counter below them.
pub fn pixel(sequence: u32, x: usize, y: usize, width: usize, height: usize) -> u32 {
    if y >= height - COUNTER_ROWS {
        let bit = 31 - x * 32 / width;
        return if sequence >> bit & 1 != 0 { 0x00FF_FFFF } else { 0 };
    }
    let shift = sequence as usize * STEP % width;
    BARS[(x + shift) % width * 8 / width]
}

/// Frame `sequence` of the pattern into `out` (`width * height` pixels, row after row).
pub fn fill(sequence: u32, width: usize, height: usize, out: &mut [u32]) {
    for (y, row) in out.chunks_exact_mut(width).take(height).enumerate() {
        for (x, p) in row.iter_mut().enumerate() { *p = pixel(sequence, x, y, width, height); }
    }
}

/// The frame number the counter rows of a picture carry (what a reader of a frame sees), or None if a cell is not
/// plainly black or white (a picture that is not the pattern, or one a lossy codec blurred too much: the cells are
/// read at their centers with a threshold).
pub fn counter(width: usize, height: usize, at: impl Fn(usize, usize) -> u32) -> Option<u32> {
    let y = height - COUNTER_ROWS / 2;
    let mut value = 0u32;
    for cell in 0..32 {
        let x = cell * width / 32 + width / 64;
        let p = at(x, y);
        let light = ((p >> 16 & 0xFF) + (p >> 8 & 0xFF) + (p & 0xFF)) / 3;
        let bit = match light { 0..=64 => 0, 192..=255 => 1, _ => return None };
        value = value << 1 | bit;
    }
    Some(value)
}

/// YUY2 (YUYV 4:2:2, BT.601 limited range, as UVC cameras send it) into 0x00RRGGBB pixels: each 4 bytes Y0 U Y1 V
/// give two pixels. Returns the pixels written.
pub fn yuy2_to_rgb(source: &[u8], out: &mut [u32]) -> usize {
    let mut written = 0;
    for (quad, pair) in source.chunks_exact(4).zip(out.chunks_exact_mut(2)) {
        for (luma, p) in [quad[0], quad[2]].into_iter().zip(pair.iter_mut()) {
            *p = yuv(luma, quad[1], quad[3]);
            written += 1;
        }
    }
    written
}

/// A YUY2 picture of `width` × `height` into `out_width` × `out_height` pixels, each the nearest one (a camera's frame
/// size to the size a program asked for); pixels the source lacks are black.
pub fn yuy2_scaled(source: &[u8], width: usize, height: usize, out: &mut [u32], out_width: usize, out_height: usize) {
    for (y, row) in out.chunks_exact_mut(out_width).take(out_height).enumerate() {
        let line = y * height / out_height * width;
        for (x, p) in row.iter_mut().enumerate() {
            let column = x * width / out_width;
            let at = (line + (column & !1)) * 2;
            *p = match source.get(at..at + 4) { Some(q) => yuv(if column & 1 == 0 { q[0] } else { q[2] }, q[1], q[3]), None => 0 };
        }
    }
}

// One pixel from its luma and its pair's chroma (BT.601 limited range).
fn yuv(luma: u8, u: u8, v: u8) -> u32 {
    let (c, u, v) = (298 * (luma as i32 - 16), u as i32 - 128, v as i32 - 128);
    let clamp = |value: i32| ((value + 128) >> 8).clamp(0, 255) as u32;
    clamp(c + 409 * v) << 16 | clamp(c - 100 * u - 208 * v) << 8 | clamp(c + 516 * u)
}
