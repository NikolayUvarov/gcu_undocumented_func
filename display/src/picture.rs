//! Step 4's picture (213-DRV-0092, stage 10): what `display` draws into its own surface before pipe 1 scans it, made so
//! that a photo of the monitor is its own evidence. Its text names what shows it, in block letters of a 5x7 font scaled
//! ten times. A thick white border, both diagonals and a centre cross lean by 45 degrees over the screen's height if the
//! pitch is off by one pixel, since every row then moves a pixel against the last. `tests/display_picture_host.rs`.

/// First light's surface: 1920x1080 at 4 bytes a pixel (0x00RRGGBB), its pitch the width.
pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
pub const PITCH: usize = WIDTH;
/// The widest surface drawn (a row of it is on the stack while stage 10 draws).
pub const MAX_WIDTH: usize = 4096;

/// A surface's shape: its width and height, and its pitch in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size { pub width: usize, pub height: usize, pub pitch: usize }
impl Size {
    /// First light's.
    pub const FIRST: Size = Size { width: WIDTH, height: HEIGHT, pitch: PITCH };
    /// For a mode: the pitch rounded up to 64 pixels, as Linux's framebuffer is (3456 for 3440); the hardware rounds the
    /// field to a unit of at most 128 pixels (213-DRV-0092), so a pitch of 64's multiple is what it scans.
    pub const fn for_mode(width: usize, height: usize) -> Size { Size { width, height, pitch: width.div_ceil(64) * 64 } }
    pub const fn bytes(&self) -> usize { self.pitch * self.height * 4 }
}

pub const BACKGROUND: u32 = 0x0020_3060;
pub const WHITE: u32 = 0x00FF_FFFF;
pub const PANEL: u32 = 0x0010_1820;
pub const TEXT: u32 = 0x00FF_D000;

pub const BORDER: usize = 12;
/// Half the thickness of the diagonals and of the cross.
pub const LINE: usize = 4;
/// The font's scale: a glyph of 5x7 dots is 50x70 pixels, one dot apart.
pub const SCALE: usize = 10;
pub const ADVANCE: usize = 6 * SCALE;
pub const LINE_HEIGHT: usize = 10 * SCALE;
/// The longest line drawn; a longer one is cut.
pub const MAX_CHARS: usize = 30;
/// The panel behind the text, so that the letters stay legible over the diagonals (first light's surface; `text_top`,
/// `panel_x` and `panel_y` for another).
pub const TEXT_TOP: usize = text_top(HEIGHT);
pub const PANEL_X: core::ops::Range<usize> = panel_x(WIDTH);
pub const PANEL_Y: core::ops::Range<usize> = panel_y(HEIGHT);
pub const fn text_top(height: usize) -> usize { (height - 4 * LINE_HEIGHT) / 2 }
pub const fn panel_x(width: usize) -> core::ops::Range<usize> { 100..width - 100 }
pub const fn panel_y(height: usize) -> core::ops::Range<usize> { text_top(height) - 30..text_top(height) + 4 * LINE_HEIGHT }

/// The 5x7 glyph of an upper-case letter, a digit or `- / + . :` (rows top first, bit 4 the leftmost dot); a filled
/// box for anything else, a blank for a space.
pub fn glyph(c: u8) -> [u8; 7] {
    match c {
        b'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11], b'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        b'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E], b'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        b'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F], b'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        b'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F], b'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        b'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E], b'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        b'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11], b'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        b'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11], b'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        b'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E], b'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        b'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D], b'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        b'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E], b'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        b'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E], b'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        b'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A], b'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        b'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04], b'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        b'0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E], b'1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        b'2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F], b'3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        b'4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02], b'5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        b'6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E], b'7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        b'8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E], b'9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        b'-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00], b'/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        b'+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00], b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        b':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00], b' ' => [0; 7],
        _ => [0x1F; 7],
    }
}

/// The picture's four lines: its name, its program, its output and mode, and the build (lower case drawn as upper); and
/// the surface it is drawn for.
pub struct Lines<'a> { pub build: &'a str, pub size: Size, mode: [u8; 20], mode_len: usize }
impl<'a> Lines<'a> {
    pub fn new(build: &'a str, size: Size) -> Lines<'a> {
        let mut mode = [0u8; 20];
        let mut n = 0;
        let mut put = |b: u8| { if n < mode.len() { mode[n] = b; n += 1; } };
        for &b in b"HDMI-A-1 " { put(b); }
        for (k, v) in [size.width, size.height].into_iter().enumerate() {
            let digits = [v / 1000 % 10, v / 100 % 10, v / 10 % 10, v % 10];
            let first = digits.iter().position(|&d| d != 0).unwrap_or(3);
            for &d in &digits[first..] { put(b'0' + d as u8); }
            if k == 0 { put(b'X'); }
        }
        Lines { build, size, mode, mode_len: n }
    }
    fn line(&self, n: usize, i: usize) -> Option<u8> {
        let fixed: &[u8] = match n { 0 => b"MIND CORE", 1 => b"DISPLAY", 2 => &self.mode[..self.mode_len], _ => b"BUILD " };
        if n < 3 { return fixed.get(i).copied(); }
        if i < fixed.len() { return Some(fixed[i]); }
        self.build.as_bytes().get(i - fixed.len()).map(|c| c.to_ascii_uppercase()).filter(|_| i < MAX_CHARS)
    }
    /// How many characters line `n` draws.
    pub fn len(&self, n: usize) -> usize { (0..MAX_CHARS).take_while(|&i| self.line(n, i).is_some()).count() }
}

/// The pixel at (x, y) of the surface `lines` is drawn for.
pub fn pixel(lines: &Lines, x: usize, y: usize) -> u32 {
    let (width, height) = (lines.size.width, lines.size.height);
    if x < BORDER || y < BORDER || x >= width - BORDER || y >= height - BORDER { return WHITE; }
    let top = text_top(height);
    if panel_x(width).contains(&x) && panel_y(height).contains(&y) {
        let row = y.wrapping_sub(top);
        let n = row / LINE_HEIGHT;
        if y >= top && n < 4 && row % LINE_HEIGHT < 7 * SCALE {
            let len = lines.len(n);
            let left = (width - len * ADVANCE + SCALE) / 2;
            if x >= left && x < left + len * ADVANCE {
                let (i, dx) = ((x - left) / ADVANCE, (x - left) % ADVANCE);
                if let (Some(c), true) = (lines.line(n, i), dx < 5 * SCALE) {
                    if glyph(c)[row % LINE_HEIGHT / SCALE] & (0x10 >> (dx / SCALE)) != 0 { return TEXT; }
                }
            }
        }
        return PANEL;
    }
    // Both diagonals, corner to corner, and the cross through the centre.
    let (xi, yi, w, h) = (x as i64, y as i64, (width - 1) as i64, (height - 1) as i64);
    let near = |d: i64| d.abs() * d.abs() <= (LINE * LINE) as i64 * (w * w + h * h);
    if near(xi * h - yi * w) || near((w - xi) * h - yi * w) { return WHITE; }
    if x.abs_diff(width / 2) < LINE || y.abs_diff(height / 2) < LINE { return WHITE; }
    BACKGROUND
}

/// Row `y` of the picture, into `out`'s first `lines.size.width` pixels.
pub fn row(lines: &Lines, y: usize, out: &mut [u32]) {
    for (x, p) in out.iter_mut().take(lines.size.width).enumerate() { *p = pixel(lines, x, y); }
}

/// The pixels stage 10 reads back: the corners, the border's middle, the cross, both diagonals at row 100, a pixel of
/// the background between them, and the first letter's first two columns (its dot, the panel after it).
pub fn samples(size: Size) -> [(usize, usize); 9] {
    let (w, h) = (size.width, size.height);
    let dx = (100 * (w - 1) + (h - 1) / 2) / (h - 1);
    let left = (w - 9 * ADVANCE + SCALE) / 2;
    [(0, 0), (w - 1, h - 1), (5, h / 2), (w / 2, 100), (dx, 100), (w - 1 - dx, 100), (300, 100), (left, text_top(h)), (left + SCALE, text_top(h))]
}
