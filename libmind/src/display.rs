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

/// The picture's columns (or rows) `[first, last]` that show source columns `from..=to`, for a picture `picture`
/// wide of a source `source` wide; None when the picture shows none of them.
pub fn shown(from: usize, to: usize, picture: usize, source: usize) -> Option<(usize, usize)> {
    if from > to || picture == 0 { return None; }
    // The first picture column at or after `from`, and the last at or before `to`, by source_of's own rounding.
    let first = ((from as u64 * picture as u64).div_ceil(source.max(1) as u64) as usize).min(picture);
    let last = (((to as u64 + 1) * picture as u64).div_ceil(source.max(1) as u64) as usize).min(picture).checked_sub(1)?;
    (first <= last).then_some((first, last))
}
