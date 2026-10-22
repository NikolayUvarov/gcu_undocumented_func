//! The gateway's PNG of the screen (750-EFF-0007, effector_gw/src/png.rs): what it writes decodes, through MIND's own
//! decoder (libmind/src/png.rs, inflate.rs), to exactly the pixels it was given; a screen of text compresses far below
//! the 2 MiB the server takes; the shrink keeps proportions and averages.
#![allow(dead_code)]
extern crate alloc;
#[path = "../libmind/src/inflate.rs"]
mod inflate;
#[path = "../libmind/src/png.rs"]
mod png;
#[path = "../effector_gw/src/png.rs"]
mod encoder;

fn decoded(file: &[u8]) -> (usize, usize, Vec<u32>) {
    let mut pixels = Vec::new();
    let (w, h) = png::decode(file, 0, 8192, &mut |y, row| { assert_eq!(pixels.len(), y * row.len()); pixels.extend_from_slice(row); }).expect("decodes");
    (w, h, pixels)
}

fn round_trip(width: usize, height: usize, picture: &dyn Fn(usize, usize) -> u32) -> usize {
    let file = encoder::encode(width, height, &mut |y, row| for (x, p) in row.iter_mut().enumerate() { *p = picture(x, y); });
    let (w, h, pixels) = decoded(&file);
    assert_eq!((w, h), (width, height));
    for y in 0..height { for x in 0..width { assert_eq!(pixels[y * width + x], picture(x, y), "pixel {},{}", x, y); } }
    file.len()
}

#[test]
fn flat_and_tiny_pictures_round_trip() {
    round_trip(1, 1, &|_, _| 0x123456);
    round_trip(3, 2, &|x, y| (x * 80 + y * 7) as u32 * 0x010101);
    let size = round_trip(1280, 800, &|_, _| 0x202830);
    assert!(size < 32 * 1024, "a flat screen took {} bytes", size);
}

#[test]
fn every_filter_round_trips() {
    // Gradients choose sub, up, average and Paeth; noise chooses none.
    round_trip(97, 61, &|x, y| ((x * 3) as u32 & 255) << 16 | ((y * 5) as u32 & 255) << 8 | ((x + y) as u32 & 255));
    let mut seed = 0x2545F491u32;
    let noise: Vec<u32> = (0..64 * 40).map(|_| { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; seed & 0xFFFFFF }).collect();
    round_trip(64, 40, &|x, y| noise[y * 64 + x]);
}

#[test]
fn a_text_screen_compresses() {
    // A console: light glyph-like strokes on a dark background, 8x16 cells.
    let size = round_trip(1280, 800, &|x, y| {
        let (cx, cy) = (x / 8, y / 16);
        let on = (cx * 7 + cy * 13) % 5 != 0 && (x % 8 == 1 || y % 16 == 3 || (x + y) % 7 == 0) && x % 8 != 7 && y % 16 < 13;
        if on { 0xC8D0D8 } else { 0x101418 }
    });
    assert!(size < 400 * 1024, "a text screen took {} bytes", size);
}

#[test]
fn long_runs_and_far_matches() {
    // Rows that repeat beyond the window and runs longer than a match.
    round_trip(4000, 12, &|x, y| if y % 3 == 0 { (x as u32 / 300) * 0x111111 } else { 0xFFFFFF });
    let data: Vec<u8> = (0..200_000u32).map(|i| (i / 1000 % 7) as u8 * 30 + (i % 3) as u8).collect();
    let mut out = Vec::new();
    inflate::zlib(&encoder::zlib(&data), &mut |piece| { out.extend_from_slice(piece); true }).expect("inflates");
    assert_eq!(out, data);
}

#[test]
fn checksums_are_the_standard_ones() {
    assert_eq!(encoder::crc32(b"IEND"), 0xAE42_6082);
    assert_eq!(encoder::crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn the_shrink_keeps_proportions_and_averages() {
    assert_eq!(encoder::fit(800, 600, 1280, 800), (800, 600));
    assert_eq!(encoder::fit(2880, 1800, 1280, 800), (1280, 800));
    assert_eq!(encoder::fit(1920, 1080, 1280, 800), (1280, 720));
    assert_eq!(encoder::fit(1024, 1280, 1280, 800), (640, 800));
    // A 4x2 picture to 2x1: each pixel the mean of a 2x2 box.
    let pixels = [0x000000, 0x0000FF, 0xFF0000, 0xFF0000, 0xDEAD, 0x000000, 0x0000FF, 0x00FF00, 0x00FF00, 0xDEAD];
    let mut row = [0u32; 2];
    encoder::shrink_row(&pixels, 5, 4, 2, 1, 0, &mut row);
    assert_eq!(row, [0x000080, 0x808000]);
}
