// Host cases of step 4's picture (213-DRV-0092, stage 10): its glyphs, where its lines and letters fall, its border,
// diagonals and cross, and that a pitch off by one pixel cannot pass unseen.
#![allow(dead_code)] // the driver uses parts these cases do not
#[path = "../display/src/picture.rs"] mod picture;

use picture::{glyph, pixel, Lines, BACKGROUND, HEIGHT, PANEL, TEXT, WHITE, WIDTH};

#[test]
fn every_character_it_draws_has_a_glyph_of_its_own() {
    let box_ = glyph(0x7F);
    for c in b"MIND CORE/DISPLAY/HDMI-A-1 1920X1080/BUILD 0123456789ABCDEF+CHANGES:.".iter().copied().filter(|&c| c != b' ') {
        assert_ne!(glyph(c), box_, "{}", c as char);
        assert!(glyph(c).iter().all(|row| row & !0x1F == 0), "{} wider than five dots", c as char);
    }
    assert_eq!(glyph(b' '), [0; 7]);
    // No two of the hex digits and letters alike.
    let all: Vec<[u8; 7]> = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ".iter().map(|&c| glyph(c)).collect();
    for i in 0..all.len() { for j in i + 1..all.len() { assert_ne!(all[i], all[j], "{} {}", i, j); } }
}

#[test]
fn the_lines_say_what_shows_them_and_the_build_is_drawn_in_upper_case_and_cut() {
    let l = Lines { build: "1c65c3cb3abc+CHANGES" };
    assert_eq!((l.len(0), l.len(1), l.len(2), l.len(3)), (9, 7, 18, 26));
    let long = Lines { build: "0123456789012345678901234567890123456789" };
    assert_eq!(long.len(3), picture::MAX_CHARS);
    // Each line centred on the panel and inside it.
    for n in 0..4 {
        let w = l.len(n) * picture::ADVANCE - picture::SCALE;
        let left = (WIDTH - w) / 2;
        assert!(picture::PANEL_X.contains(&left) && picture::PANEL_X.contains(&(left + w - 1)), "line {}", n);
    }
    // The first letter, M: its first row has dots at columns 0 and 4 only, each ten pixels square.
    let (left, top) = ((WIDTH - (9 * picture::ADVANCE - picture::SCALE)) / 2, picture::TEXT_TOP);
    for dx in 0..50 { assert_eq!(pixel(&l, left + dx, top + 5), if dx < 10 || dx >= 40 { TEXT } else { PANEL }, "{}", dx); }
    // The build's lower-case c is drawn as C.
    let c_left = (WIDTH - (26 * picture::ADVANCE - picture::SCALE)) / 2 + 7 * picture::ADVANCE;
    let y3 = top + 3 * picture::LINE_HEIGHT;
    let drawn: Vec<bool> = (0..5).map(|dx| pixel(&l, c_left + dx * 10 + 5, y3 + 5) == TEXT).collect();
    assert_eq!(drawn, [false, true, true, true, false], "the top row of C is .###.");
}

#[test]
fn the_border_the_diagonals_and_the_cross_are_where_they_should_be() {
    let l = Lines { build: "X" };
    for (x, y) in [(0, 0), (WIDTH - 1, 0), (0, HEIGHT - 1), (WIDTH - 1, HEIGHT - 1), (11, 500), (WIDTH - 12, 500), (900, 11), (900, HEIGHT - 12)] {
        assert_eq!(pixel(&l, x, y), WHITE, "border at {},{}", x, y);
    }
    assert_eq!(pixel(&l, 12, 100), BACKGROUND);
    // The diagonals at y = 100 (x = 100 x 1919 / 1079), the cross's upright at x = 960 and its arm at y = 540.
    assert_eq!(pixel(&l, 178, 100), WHITE);
    assert_eq!(pixel(&l, WIDTH - 1 - 178, 100), WHITE);
    assert_eq!(pixel(&l, 190, 100), BACKGROUND);
    assert_eq!(pixel(&l, 960, 100), WHITE);
    assert_eq!(pixel(&l, 50, 540), WHITE);
    assert_eq!(pixel(&l, 50, 560), BACKGROUND);
}

#[test]
fn a_pitch_off_by_one_pixel_leans_every_edge() {
    // The picture drawn at its pitch, then scanned a pixel a row too wide or too narrow, as a wrong PITCH would scan it.
    let l = Lines { build: "TEST" };
    let mut surface = vec![0u32; WIDTH * HEIGHT + 2 * HEIGHT];
    let mut row = [0u32; WIDTH];
    for y in 0..HEIGHT { picture::row(&l, y, &mut row); surface[y * WIDTH..][..WIDTH].copy_from_slice(&row); }
    for pitch in [WIDTH + 1, WIDTH - 1] {
        let scanned = |x: usize, y: usize| surface[y * pitch + x];
        // At its own pitch the left border is white on every row; off by one, it has left the left edge from row 12 on.
        let white_rows = (0..HEIGHT).filter(|&y| (0..picture::BORDER).all(|x| scanned(x, y) == WHITE)).count();
        assert!(white_rows < 40, "pitch {}: {} rows still start with the border", pitch, white_rows);
        // The cross's upright at x = 960 is no longer upright: on most rows outside the panel it is not at 960.
        let upright = (0..HEIGHT).filter(|&y| !picture::PANEL_Y.contains(&y)).filter(|&y| scanned(960, y) == WHITE).count();
        assert!(upright < 100, "pitch {}: {} rows keep the upright at 960", pitch, upright);
    }
    let own = |x: usize, y: usize| surface[y * WIDTH + x];
    assert_eq!((0..HEIGHT).filter(|&y| (0..picture::BORDER).all(|x| own(x, y) == WHITE)).count(), HEIGHT);
}
