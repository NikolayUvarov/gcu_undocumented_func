//! wm's scale (000-APP-0061): every cell of its grid, the pixel windows' content and the pointer drawn larger, so a
//! screen of many pixels shows text one can read and targets one can hit. `scale = auto` (the default) takes, at each
//! start, the largest scale that leaves at least 1280 × 720 pixels at 100%; the user may choose one of `SCALES` that
//! leaves room for Settings. No system calls: tests/wm_host.rs.

/// The scales offered, in percent of the 8 × 16 cell: each gives whole pixels (150%: 12 × 24).
pub const SCALES: [u16; 8] = [100, 150, 200, 250, 300, 400, 500, 600];
/// What `auto` leaves of the screen at most scaled: pixels at 100%, as a 1280 × 720 screen shows.
pub const AUTO_ROOM: (usize, usize) = (1280, 720);
/// The fewest cells a chosen scale must leave: Settings' window (84 × 17) and the top bar and status line around it.
pub const FEWEST: (usize, usize) = (84, 26);

/// A cell's size in pixels at `scale` percent.
pub fn cell(scale: u16) -> (usize, usize) { (8 * scale as usize / 100, 16 * scale as usize / 100) }

/// The cells of a screen of `screen` pixels at `scale`.
pub fn cells(scale: u16, screen: (usize, usize)) -> (usize, usize) { let (w, h) = cell(scale); (screen.0 / w, screen.1 / h) }

/// Whether `scale` leaves room for Settings on a screen of `screen` pixels (100% always does what it can).
pub fn fits(scale: u16, screen: (usize, usize)) -> bool {
    let (cols, rows) = cells(scale, screen);
    scale == 100 || (cols >= FEWEST.0 && rows >= FEWEST.1)
}

/// The scales a user may choose on a screen of `screen` pixels.
pub fn offered(screen: (usize, usize)) -> impl Iterator<Item = u16> { SCALES.into_iter().filter(move |&s| fits(s, screen)) }

/// `auto` on a screen of `screen` pixels: the largest scale that leaves `AUTO_ROOM`, or 100%. A 1920 × 1080 screen
/// gets 150%, a MacBook Pro's 2880 × 1800 200%, a 3840 × 2160 one 300%.
pub fn auto(screen: (usize, usize)) -> u16 {
    SCALES.into_iter().filter(|&s| screen.0 * 100 / s as usize >= AUTO_ROOM.0 && screen.1 * 100 / s as usize >= AUTO_ROOM.1 && fits(s, screen)).max().unwrap_or(100)
}

/// The scale wm uses for the configured one (None: auto): a chosen scale that no longer fits (another screen) falls
/// back to auto.
pub fn resolve(chosen: Option<u16>, screen: (usize, usize)) -> u16 {
    chosen.filter(|&s| SCALES.contains(&s) && fits(s, screen)).unwrap_or_else(|| auto(screen))
}
