// The compositor's second screen through a plug, an unplug and another plug (213-KRN-0278), as DRV's 213-DRV-0096 will
// drive it: the shape each offer gives, and the budget of lines the compositor says about it.
#[allow(dead_code)]
#[path = "../libmind/src/display.rs"]
mod display;
#[allow(unused_imports)]
use display::placed;
#[path = "../compositor/src/second.rs"]
mod second;
use second::{Budget, Shape};

const RYZEN: (usize, usize) = (2560, 1600); // the first screen on the bench

fn offer(width: usize, height: usize, stretch: bool) -> Option<Shape> {
    Shape::offered(width, height, width * 4, width * height * 4, RYZEN, stretch)
}

#[test]
fn a_screen_offered_gone_and_offered_again_at_another_size() {
    // 1920x1080 in shape: the full height, 1728 wide, centred.
    let mut screen = offer(1920, 1080, false);
    assert_eq!(screen, Some(Shape { width: 1920, height: 1080, at: (96, 0), size: (1728, 1080) }));
    // Gone: nothing is mirrored, so `mirror` answers no-screen.
    screen = None;
    assert!(screen.is_none());
    // 3440x1440 next: its own shape, nothing left of the first.
    screen = offer(3440, 1440, false);
    assert_eq!(screen, Some(Shape { width: 3440, height: 1440, at: (568, 0), size: (2304, 1440) }));
    // Stretched now: the whole screen; and kept again, back in the middle.
    let mut shape = screen.unwrap();
    shape.place(RYZEN, true);
    assert_eq!((shape.at, shape.size), ((0, 0), (3440, 1440)));
    shape.place(RYZEN, false);
    assert_eq!((shape.at, shape.size), ((568, 0), (2304, 1440)));
    // An offer made while stretched is placed stretched.
    assert_eq!(offer(1920, 1080, true).map(|s| (s.at, s.size)), Some(((0, 0), (1920, 1080))));
}

#[test]
fn an_offer_that_does_not_fit_what_it_lent_is_refused() {
    let lent = 1920 * 1080 * 4;
    assert!(Shape::offered(1920, 1080, 7680, lent, RYZEN, false).is_some());
    assert!(Shape::offered(1920, 1080, 7680, lent - 1, RYZEN, false).is_none()); // a row short
    assert!(Shape::offered(1920, 1080, 7676, lent, RYZEN, false).is_none()); // a pitch under the width
    assert!(Shape::offered(1920, 1080, 7682, lent * 2, RYZEN, false).is_none()); // not whole pixels
    assert!(Shape::offered(0, 1080, 7680, lent, RYZEN, false).is_none());
    assert!(Shape::offered(1920, 0, 7680, lent, RYZEN, false).is_none());
    // A wider pitch is fine when the lent range holds it.
    assert!(Shape::offered(1920, 1080, 8192, 8192 * 1080, RYZEN, false).is_some());
}

#[test]
fn the_budget_says_twenty_four_lines_then_one_a_minute_and_counts_the_rest() {
    let mut budget = Budget::new();
    for _ in 0..Budget::LINES { assert_eq!(budget.take(9_000), Some(0)); }
    // A dozen plugs and unplugs later the lines stop being said, and are counted.
    assert_eq!(budget.take(9_500), None);
    assert_eq!(budget.take(10_000), None);
    // A minute on, one line, which says how many were not.
    assert_eq!(budget.take(61_000), Some(2));
    assert_eq!(budget.take(61_001), None);
    assert_eq!(budget.take(120_000), Some(1));
    // A quiet day refills it to the full 24, no more.
    for _ in 0..Budget::LINES { assert_eq!(budget.take(24 * 3_600_000), Some(0)); }
    assert_eq!(budget.take(24 * 3_600_000), None);
}
