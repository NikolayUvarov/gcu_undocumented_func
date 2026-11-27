// The compositor's mirror of the first screen on a second one (213-KRN-0272): where the picture goes, kept in shape or
// stretched (213-KRN-0273), which source pixel each of its pixels shows, and which picture pixels a changed span touches.
#[allow(dead_code)]
#[path = "../libmind/src/display.rs"]
mod display;
use display::{fit, placed, screen_name, screen_of, shown, source_of, BADGE_REMOTE_EFFECTOR, BADGE_REMOTE_LINK, BADGE_SCREEN, BADGE_THIRD_SCREEN, DRIVER_SCREENS};

#[test]
fn the_ryzens_screens_fit_centred() {
    // 2560x1600 (16:10) on 1920x1080 (16:9): the full height, 1728 wide, 96 black columns each side.
    assert_eq!(fit((2560, 1600), (1920, 1080)), (96, 0, 1728, 1080));
    // The same shape fills the target; a wider source is centred with bars above and below.
    assert_eq!(fit((1920, 1080), (1920, 1080)), (0, 0, 1920, 1080));
    assert_eq!(fit((1280, 800), (3440, 1440)), (568, 0, 2304, 1440));
    assert_eq!(fit((1920, 1080), (1280, 1024)), (0, 152, 1280, 720));
    assert_eq!(fit((0, 0), (10, 10)), (0, 0, 10, 10));
}

#[test]
fn each_picture_pixel_shows_the_nearest_source_pixel_in_bounds() {
    for (source, picture) in [(2560, 1728), (1600, 1080), (800, 1440), (7, 3), (3, 7)] {
        let mut last = 0;
        for at in 0..picture {
            let s = source_of(at, picture, source);
            assert!(s < source && s >= last, "{source} on {picture}: {at} -> {s}");
            last = s;
        }
        assert_eq!(source_of(0, picture, source), 0);
    }
}

#[test]
fn a_changed_span_touches_exactly_the_picture_pixels_that_show_it() {
    for (source, picture) in [(2560, 1728), (1600, 1080), (800, 1440), (7, 3), (3, 7), (10, 10)] {
        for from in 0..source.min(40) {
            for to in from..source.min(from + 13) {
                let expected: Vec<usize> = (0..picture).filter(|&c| (from..=to).contains(&source_of(c, picture, source))).collect();
                let got: Vec<usize> = shown(from, to, picture, source).map_or(vec![], |(a, b)| (a..=b).collect());
                assert_eq!(got, expected, "{source} on {picture}: span {from}..={to}");
            }
        }
    }
    assert_eq!(shown(5, 4, 10, 10), None);
}

#[test]
fn a_stretched_mirror_fills_the_second_screen_each_axis_on_its_own() {
    // The Ryzen's two shapes: the whole of 1920x1080, no black columns; kept, 1.3's picture.
    assert_eq!(placed(true, (2560, 1600), (1920, 1080)), (0, 0, 1920, 1080));
    assert_eq!(placed(false, (2560, 1600), (1920, 1080)), fit((2560, 1600), (1920, 1080)));
    assert_eq!(placed(true, (1280, 800), (3440, 1440)), (0, 0, 3440, 1440));
    assert_eq!(placed(true, (10, 10), (0, 0)), (0, 0, 1, 1));
    // Each axis on its own: the first picture column and row show the source's first; the last show one of the source's
    // last `source / picture` (rounded up), all of them when the picture is the larger.
    for (source, target) in [((2560, 1600), (1920, 1080)), ((1280, 800), (3440, 1440)), ((1920, 1080), (1280, 1024))] {
        let (_, _, w, h) = placed(true, source, target);
        for (s, p) in [(source.0, w), (source.1, h)] {
            assert_eq!(source_of(0, p, s), 0);
            assert!(source_of(p - 1, p, s) >= s - s.div_ceil(p), "{s} on {p}: {}", source_of(p - 1, p, s));
            if p >= s { assert_eq!(source_of(p - 1, p, s), s - 1); }
        }
    }
}

#[test]
fn each_drivers_badge_names_one_screen_and_no_other_badge_names_any() {
    // 213-KRN-0285: the second screen through BADGE_SCREEN, the third through BADGE_THIRD_SCREEN; a remote mark's client,
    // the shell's (no badge) and any other badge offer none (MC-3.11).
    assert_eq!(screen_of(BADGE_SCREEN), Some(0));
    assert_eq!(screen_of(BADGE_THIRD_SCREEN), Some(1));
    for badge in [0, BADGE_REMOTE_LINK, BADGE_REMOTE_EFFECTOR, 5, u16::MAX] { assert_eq!(screen_of(badge), None, "badge {badge}"); }
    assert!((0..=u16::MAX).filter_map(screen_of).all(|s| s < DRIVER_SCREENS));
    assert_eq!((screen_name(0), screen_name(1)), ("SECOND", "THIRD"));
}
