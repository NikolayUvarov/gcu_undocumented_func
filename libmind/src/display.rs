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
