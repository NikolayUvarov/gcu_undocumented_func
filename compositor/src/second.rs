// The parts of the second screen that need no device (213-KRN-0278): the shape a driver's offer gives, and the budget of
// lines the compositor says about it. Built on the host too (tests/compositor_second_host.rs).

/// The second screen as the compositor shows it: its size, and where the first screen's picture sits on it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shape { pub width: usize, pub height: usize, pub at: (usize, usize), pub size: (usize, usize) }

impl Shape {
    /// A driver's offer of `width`x`height`, `pitch` bytes a row, in `lent` bytes: the shape, or None when it does not fit.
    pub fn offered(width: usize, height: usize, pitch: usize, lent: usize, first: (usize, usize), stretch: bool) -> Option<Shape> {
        if width == 0 || height == 0 || pitch % 4 != 0 || pitch < width * 4 || lent < pitch * height { return None; }
        let mut shape = Shape { width, height, at: (0, 0), size: (1, 1) };
        shape.place(first, stretch);
        Some(shape)
    }
    /// The picture of the first screen for a fit: in shape and centred, or stretched over the whole screen.
    pub fn place(&mut self, first: (usize, usize), stretch: bool) {
        let (x, y, w, h) = crate::placed(stretch, first, (self.width, self.height));
        (self.at, self.size) = ((x, y), (w, h));
    }
}

/// Log lines that refill: `LINES` at once, then one more each `REFILL_MS`, so a screen plugged daily is always heard and
/// one that comes and goes all the time cannot fill the system log's ring.
pub struct Budget { left: u32, since_ms: usize, unsaid: u32 }

impl Budget {
    pub const LINES: u32 = 24;
    pub const REFILL_MS: usize = 60_000;
    pub const fn new() -> Budget { Budget { left: Self::LINES, since_ms: 0, unsaid: 0 } }
    /// Whether a line may be said at `now_ms`: Some with how many lines before it were not, or None (counted as unsaid).
    pub fn take(&mut self, now_ms: usize) -> Option<u32> {
        let refills = now_ms.saturating_sub(self.since_ms) / Self::REFILL_MS;
        if refills > 0 {
            self.left = (self.left as usize + refills).min(Self::LINES as usize) as u32;
            self.since_ms += refills * Self::REFILL_MS;
        }
        if self.left == 0 { self.unsaid = self.unsaid.saturating_add(1); return None; }
        self.left -= 1;
        Some(core::mem::take(&mut self.unsaid))
    }
}
