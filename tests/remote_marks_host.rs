//! The compositor's remote marks (800-KRN-0156): which badge lights which mark, and where and what each mark is.
#[path = "../libmind/src/display.rs"] mod display;
use display::*;

#[test]
fn only_the_two_badges_light_a_mark() {
    assert_eq!(remote_of(BADGE_REMOTE_LINK), Some(Remote::Link));
    assert_eq!(remote_of(BADGE_REMOTE_EFFECTOR), Some(Remote::Effector));
    // The shell's and the video gateway's clients are unbadged: they capture and light the camera mark, never these.
    for badge in [0, 3, 6, 7, u16::MAX] {
        assert_eq!(remote_of(badge), None, "badge {badge}");
    }
}

#[test]
fn the_marks_keep_fixed_places_left_of_the_camera_mark() {
    let (width, r) = (1280, 6);
    let camera = width - 8 * r;
    let (link, effector) = (remote_center(Remote::Link, width, r), remote_center(Remote::Effector, width, r));
    assert_eq!((link, effector), ((1280 - 72, 18), (1280 - 96, 18)));
    // Neither overlaps the camera mark (16 wide) or the other.
    assert!(link.0 + r < camera - 8 && effector.0 + r < link.0 - r);
    // A screen narrower than the corner keeps them on it, at its left edge at worst.
    assert_eq!(remote_center(Remote::Effector, 50, r).0, 0);
}

#[test]
fn the_link_is_a_ring_with_a_dot_and_the_effector_a_disc() {
    let r = 6;
    let pixels = |mark| (-r..=r).flat_map(|y| (-r..=r).map(move |x| (x, y))).filter(|&(x, y)| remote_pixel(mark, x, y, r)).collect::<Vec<_>>();
    let (link, effector) = (pixels(Remote::Link), pixels(Remote::Effector));
    // The disc is every pixel within the radius; the ring leaves a gap between its dot and its edge.
    assert!(effector.contains(&(0, 0)) && effector.contains(&(3, 0)) && effector.contains(&(6, 0)) && !effector.contains(&(6, 6)));
    assert!(link.contains(&(0, 0)) && link.contains(&(2, 0)) && !link.contains(&(3, 0)) && link.contains(&(5, 0)) && link.contains(&(6, 0)));
    assert!(link.len() < effector.len() && link.iter().all(|p| effector.contains(p)));
}
