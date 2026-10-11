//! Host tests of libmind/src/video.rs (issue 158): the synthetic test source the video gateway serves and that the
//! QEMU test reads back from `camera`'s files, and the YUY2 conversion a UVC camera's frames need.
#[path = "../libmind/src/video.rs"]
mod video;
use video::*;

#[test]
fn the_bars_move_left_four_pixels_a_frame() {
    let (w, h) = (320, 240);
    assert_eq!(pixel(0, 0, 0, w, h), BARS[0]);
    assert_eq!(pixel(0, 40, 0, w, h), BARS[1], "eight bars of 40 pixels");
    assert_eq!(pixel(0, w - 1, 100, w, h), BARS[7]);
    assert_eq!(pixel(1, 36, 0, w, h), BARS[1], "frame 1: what was at x 40 is at 36");
    assert_eq!(pixel(80, 0, 0, w, h), BARS[0], "a whole turn after 80 frames");
}

#[test]
fn the_counter_carries_the_frame_number() {
    for (w, h) in [(64, 32), (320, 240), (640, 480)] {
        let mut frame = vec![0u32; w * h];
        for sequence in [0, 1, 2, 3, 0x8000_0001, 12345, u32::MAX] {
            fill(sequence, w, h, &mut frame);
            assert_eq!(counter(w, h, |x, y| frame[y * w + x]), Some(sequence), "{w}x{h} frame {sequence}");
        }
    }
    let grey = counter(320, 240, |_, _| 0x0080_8080);
    assert_eq!(grey, None, "not the pattern");
}

#[test]
fn sizes_and_rates() {
    assert!(supported(320, 240, 10) && supported(64, 32, 1) && supported(640, 480, 30));
    for (w, h, r) in [(32, 240, 10), (330, 240, 10), (672, 480, 10), (320, 31, 10), (320, 481, 10), (320, 240, 0), (320, 240, 31)] {
        assert!(!supported(w, h, r), "{w}x{h}@{r}");
    }
}

#[test]
fn yuy2_white_black_and_the_primaries() {
    let mut out = [0u32; 8];
    // Y0 U Y1 V: white and black (U = V = 128), then red and blue at BT.601's limited-range values.
    let source = [235, 128, 16, 128, 81, 90, 81, 240, 41, 240, 41, 110];
    assert_eq!(yuy2_to_rgb(&source, &mut out), 6);
    assert_eq!(out[0], 0x00FF_FFFF);
    assert_eq!(out[1], 0);
    let near = |p: u32, rgb: (i32, i32, i32)| ((p >> 16 & 0xFF) as i32 - rgb.0).abs() <= 3 && ((p >> 8 & 0xFF) as i32 - rgb.1).abs() <= 3 && ((p & 0xFF) as i32 - rgb.2).abs() <= 3;
    assert!(near(out[2], (255, 0, 0)), "red {:06x}", out[2]);
    assert!(near(out[4], (0, 0, 255)), "blue {:06x}", out[4]);
    assert_eq!(yuy2_to_rgb(&source[..6], &mut out), 2, "only whole pairs");
}

#[test]
fn yuy2_scaled_to_the_size_asked_for() {
    // 4 x 2 pixels, each a different grey: luma 16 + 20 n, neutral chroma.
    let luma = |n: usize| (16 + 20 * n) as u8;
    let source: Vec<u8> = (0..8).step_by(2).flat_map(|n| [luma(n), 128, luma(n + 1), 128]).collect();
    let mut same = [0u32; 8];
    let mut plain = [0u32; 8];
    yuy2_scaled(&source, 4, 2, &mut same, 4, 2);
    yuy2_to_rgb(&source, &mut plain);
    assert_eq!(same, plain, "the same size is the plain conversion");
    let mut half = [0u32; 2];
    yuy2_scaled(&source, 4, 2, &mut half, 2, 1);
    assert_eq!(half, [plain[0], plain[2]], "every other pixel of the first row");
    let mut double = [0u32; 32];
    yuy2_scaled(&source, 4, 2, &mut double, 8, 4);
    assert_eq!((double[0], double[1], double[2], double[3 * 8 + 7]), (plain[0], plain[0], plain[1], plain[7]), "each pixel twice each way");
    let mut beyond = [1u32; 4];
    yuy2_scaled(&source[..4], 4, 2, &mut beyond, 2, 2);
    assert_eq!(beyond[2..], [0, 0], "rows the source lacks are black");
}
