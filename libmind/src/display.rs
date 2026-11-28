//! The compositor's marks of who can reach the machine from outside (800-KRN-0156, `display.wit` 1.2): the badges
//! `init` gives the clients that may light them, and where each mark is and what it looks like.

/// `link`'s client of the compositor (800-LNK-0020): its mark is a ring with a dot in its middle.
pub const BADGE_REMOTE_LINK: u16 = 1;
/// The Effector gateway's (750-EFF-0012): its mark is a filled circle.
pub const BADGE_REMOTE_EFFECTOR: u16 = 2;

/// Whose mark a call lights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remote { Link, Effector }

/// The mark a caller's badge lights, from init's grant alone (MC-3.11); none for any other client.
pub fn remote_of(badge: u16) -> Option<Remote> {
    match badge { BADGE_REMOTE_LINK => Some(Remote::Link), BADGE_REMOTE_EFFECTOR => Some(Remote::Effector), _ => None }
}

/// A mark's centre on a `width`-wide screen, for marks of `radius`: fixed, left of the camera mark (whose centre is
/// at `width - 8 * radius`), so none moves when another appears.
pub fn remote_center(mark: Remote, width: usize, radius: usize) -> (usize, usize) {
    let left = match mark { Remote::Link => 12, Remote::Effector => 16 };
    (width.saturating_sub(left * radius), 3 * radius)
}

/// Whether the pixel `(dx, dy)` from a mark's centre is part of it: a filled disc for the Effector's, a ring of width 2
/// with a disc of radius 2 in its middle for the link's.
pub fn remote_pixel(mark: Remote, dx: isize, dy: isize, radius: isize) -> bool {
    let d2 = dx * dx + dy * dy;
    match mark {
        Remote::Effector => d2 <= radius * radius,
        Remote::Link => (d2 <= radius * radius && d2 >= (radius - 2) * (radius - 2)) || d2 <= 4,
    }
}

/// `display`'s client of the compositor (213-KRN-0272): it offers the second screen it drives (`display.wit` 1.3).
pub const BADGE_SCREEN: u16 = 3;
/// `display`'s second client of the compositor (213-KRN-0285): it offers the third screen (`display.wit` 1.5).
pub const BADGE_THIRD_SCREEN: u16 = 4;
/// The driver screens the compositor holds: the second and the third.
pub const DRIVER_SCREENS: usize = 2;
/// The driver screen a caller's badge offers or lets go, from init's grant alone (MC-3.11): 0 the second, 1 the third.
pub fn screen_of(badge: u16) -> Option<usize> {
    match badge { BADGE_SCREEN => Some(0), BADGE_THIRD_SCREEN => Some(1), _ => None }
}
/// What the log calls a driver screen.
pub fn screen_name(screen: usize) -> &'static str { if screen == 0 { "SECOND" } else { "THIRD" } }
/// `display`'s slots for the third screen (213-KRN-0285): its pixels, a window of BAR 0, and its client of the compositor.
pub const SLOT_THIRD_PIXELS: usize = 7;
pub const SLOT_THIRD_SCREEN: usize = 8;

/// Where the mirror of a `source`-sized screen goes on a `target`-sized one (213-KRN-0272): the largest picture of the
/// source's shape the target holds, centred, as (x, y, width, height) on the target; at least one pixel each way.
pub fn fit(source: (usize, usize), target: (usize, usize)) -> (usize, usize, usize, usize) {
    let ((sw, sh), (tw, th)) = ((source.0.max(1), source.1.max(1)), (target.0.max(1), target.1.max(1)));
    let (w, h) = if sw as u64 * th as u64 <= tw as u64 * sh as u64 { ((sw as u64 * th as u64 / sh as u64) as usize, th) } else { (tw, (sh as u64 * tw as u64 / sw as u64) as usize) };
    let (w, h) = (w.clamp(1, tw), h.clamp(1, th));
    ((tw - w) / 2, (th - h) / 2, w, h)
}

/// Where the mirror goes for a fit (213-KRN-0273): `fit`'s picture when the shape is kept, the whole target stretched.
pub fn placed(stretch: bool, source: (usize, usize), target: (usize, usize)) -> (usize, usize, usize, usize) {
    if stretch { (0, 0, target.0.max(1), target.1.max(1)) } else { fit(source, target) }
}

/// The source column or row a picture's column or row `at` shows: the nearest, `at * source / picture`.
pub fn source_of(at: usize, picture: usize, source: usize) -> usize {
    ((at as u64 * source as u64 / picture.max(1) as u64) as usize).min(source.saturating_sub(1))
}

/// The source columns (or rows) a picture's column or row `at` blends, as `first << 16 | w1 << 8 | w2` (213-KRN-0290):
/// `first` and the two after it, weighted `256 - w1 - w2`, `w1` and `w2` of 256 by their shares of the span the
/// picture's pixel covers. Three are all a span holds whole down to half the size, and a pixel past them is the larger
/// part of its neighbour's. A picture as large as its source has no weight past `first`, a copy pixel for pixel; a shrunk
/// one leaves no source row or column out, so a stroke one pixel thick comes out lighter but whole. `first` is
/// `source_of`'s nearest.
pub fn taps_of(at: usize, picture: usize, source: usize) -> u32 {
    let (p, s) = (picture.max(1) as u64, source.max(1).min(0xFFFF) as u64);
    let (start, end) = (at as u64 * s * 256 / p, (at as u64 + 1) * s * 256 / p);
    let first = (start / 256).min(s - 1);
    let part = |i: u64| if first + i < s { end.min((first + i + 1) * 256).saturating_sub(start.max((first + i) * 256)) } else { 0 };
    let (c0, c1, c2) = (part(0), part(1), part(2));
    let total = (c0 + c1 + c2).max(1);
    (first as u32) << 16 | ((c1 * 256 / total).min(255) as u32) << 8 | (c2 * 256 / total).min(255) as u32
}

/// `a`, `b` and `c` (a channel to each byte, the top one dropped) blended, weighted `256 - w1 - w2`, `w1` and `w2` of
/// 256; `a` itself when both are 0.
pub fn blend(a: u32, b: u32, c: u32, w1: u32, w2: u32) -> u32 {
    if w1 == 0 && w2 == 0 { return a; }
    let w0 = 256 - w1 - w2;
    let mix = |mask: u32| (((a & mask) * w0 + (b & mask) * w1 + (c & mask) * w2) >> 8) & mask;
    mix(0xFF00FF) | mix(0x00FF00)
}

/// The picture's pixel at the column and row `taps_of` gave: up to nine source pixels of `pixel(x, y)`, blended.
pub fn sampled(col: u32, row: u32, pixel: impl Fn(usize, usize) -> u32) -> u32 {
    let (x, x1, x2) = ((col >> 16) as usize, (col >> 8) & 0xFF, col & 0xFF);
    let (y, y1, y2) = ((row >> 16) as usize, (row >> 8) & 0xFF, row & 0xFF);
    let line = |y| {
        let at = |i: usize, w: u32| if w == 0 { 0 } else { pixel(x + i, y) };
        blend(pixel(x, y), at(1, x1), at(2, x2), x1, x2)
    };
    let at = |i: usize, w: u32| if w == 0 { 0 } else { line(y + i) };
    blend(line(y), at(1, y1), at(2, y2), y1, y2)
}

/// The picture's columns (or rows) `[first, last]` that show source columns `from..=to`, for a picture `picture`
/// wide of a source `source` wide; None when the picture shows none of them.
pub fn shown(from: usize, to: usize, picture: usize, source: usize) -> Option<(usize, usize)> {
    if from > to || picture == 0 { return None; }
    // The first picture column at or after `from`, and the last at or before `to`, by source_of's own rounding.
    let first = ((from as u64 * picture as u64).div_ceil(source.max(1) as u64) as usize).min(picture);
    let last = (((to as u64 + 1) * picture as u64).div_ceil(source.max(1) as u64) as usize).min(picture).checked_sub(1)?;
    (first <= last).then_some((first, last))
}
