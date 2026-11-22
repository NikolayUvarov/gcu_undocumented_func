// The compositor's mirror of the first screen on a second one (213-KRN-0272): where the picture goes, which source
// pixel each of its pixels shows, and which picture pixels a changed source span touches.
#[allow(dead_code)]
#[path = "../libmind/src/display.rs"]
mod display;
use display::{fit, shown, source_of};

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
