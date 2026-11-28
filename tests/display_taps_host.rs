//! The mirror's shrink (213-KRN-0290): each picture pixel blends the source pixels it covers, so no source row or
//! column is left out, and a picture as large as its source is a copy. The Ryzen's screens: 2560x1600 shown at
//! 2304x1440 (0.9) and at 1228x768 (0.48), where the nearest pixel dropped a glyph's strokes (DRV, 2026-11-27).
#[allow(dead_code)]
#[path = "../libmind/src/display.rs"]
mod display;
use display::{blend, sampled, source_of, taps_of};

fn first(t: u32) -> usize { (t >> 16) as usize }
// The weights of `first` and the two after it.
fn weights(t: u32) -> [u32; 3] { let (w1, w2) = ((t >> 8) & 0xFF, t & 0xFF); [256 - w1 - w2, w1, w2] }

#[test]
fn a_picture_as_large_as_its_source_is_a_copy() {
    for at in 0..1600 { assert_eq!(taps_of(at, 1600, 1600), (at as u32) << 16); }
}

#[test]
fn the_first_tap_is_the_nearest_and_never_past_the_source() {
    for (picture, source) in [(1440, 1600), (768, 1600), (2304, 2560), (1228, 2560), (3440, 2560), (7, 3)] {
        for at in 0..picture {
            let t = taps_of(at, picture, source);
            assert_eq!(first(t), source_of(at, picture, source));
            // A tap only where there is a source pixel for it.
            let w = weights(t);
            assert!((w[1] == 0 || first(t) + 1 < source) && (w[2] == 0 || first(t) + 2 < source), "{} of {} from {}", at, picture, source);
        }
    }
}

#[test]
fn a_shrink_leaves_no_source_line_out() {
    // A source line counts as shown when a picture line takes it with a weight of at least a fifth.
    for (picture, source) in [(1440, 1600), (768, 1600), (2304, 2560), (1228, 2560)] {
        let mut weight_of = vec![0u32; source];
        for at in 0..picture {
            let t = taps_of(at, picture, source);
            for (i, w) in weights(t).into_iter().enumerate() { if w > 0 { weight_of[first(t) + i] = weight_of[first(t) + i].max(w); } }
        }
        let left_out = (0..source).filter(|&s| weight_of[s] < 256 / 5).count();
        assert_eq!(left_out, 0, "{} of {} source lines left out at {} of {}", left_out, source, picture, source);
        // The nearest pixel alone leaves lines out at both scales: what the maintainer saw.
        let nearest: std::collections::BTreeSet<usize> = (0..picture).map(|at| source_of(at, picture, source)).collect();
        assert!(nearest.len() < source);
    }
}

#[test]
fn a_stroke_one_pixel_thick_comes_out_lighter_but_whole() {
    // White strokes on black, one source row in every three, shrunk to 0.48 and 0.9: every stroke still shows.
    let (sw, sh) = (64usize, 300usize);
    let pixel = |_x: usize, y: usize| if y % 3 == 0 { 0x00FF_FFFF } else { 0 };
    for picture in [144usize, 270] {
        let rows: Vec<u32> = (0..picture).map(|r| taps_of(r, picture, sh)).collect();
        let col = taps_of(10, sw, sw);
        for stroke in (0..sh).step_by(3) {
            let brightest = rows.iter().filter(|&&t| (0..3).any(|i| weights(t)[i] > 0 && first(t) + i == stroke))
                .map(|&t| sampled(col, t, pixel) & 0xFF).max().unwrap_or(0);
            assert!(brightest >= 0x33, "stroke {} gone at {} rows of {}", stroke, picture, sh);
        }
    }
}

#[test]
fn blending_keeps_each_channel_apart() {
    assert_eq!(blend(0x00FF_0000, 0x0000_00FF, 0, 0, 0), 0x00FF_0000);
    assert_eq!(blend(0x00FF_0000, 0x0000_00FF, 0, 128, 0), 0x007F_007F);
    assert_eq!(blend(0x0000_FF00, 0x00FF_0000, 0x0000_00FF, 64, 64), 0x003F_7F3F); // each share rounded down
    assert_eq!(blend(0x00FF_FFFF, 0x00FF_FFFF, 0x00FF_FFFF, 100, 100), 0x00FF_FFFF);
    assert_eq!(blend(0xFF00_0000, 0, 0, 1, 0) >> 24, 0, "the top byte is dropped");
}
