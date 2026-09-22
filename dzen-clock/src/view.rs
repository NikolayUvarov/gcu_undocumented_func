use crate::{
    abi::BootInfo,
    cycle::{point_at, OrbitMode},
    face::{Face, OFF},
    font::FONT,
};

const BACKGROUND: u32 = OFF;
const RIM: u32 = 0x1C2632;
const DIGITS: u32 = 0x697583;
const HINT: u32 = 0x394553;
const CYCLE: u32 = 0x808080;
const ORBIT_SIMPLE: u32 = 0x181818;
const ORBIT_TICKS: u32 = 0x282828;
const START_TICK: u32 = 0x606060;
const SMALL_TICK: u32 = 0x404040;

pub struct View<'a> {
    info: &'a BootInfo,
    x: usize,
    y: usize,
    half: usize,
    radius: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{ProgramImage, PROGRAM_COUNT};

    #[test]
    fn full_turn_restores_orbit_ticks_and_hiding_restores_background() {
        let mut pixels = std::vec![0u32; 640 * 480];
        let info = BootInfo {
            fb_ptr: pixels.as_mut_ptr(),
            width: 640,
            height: 480,
            stride: 640,
            programs: [ProgramImage {
                data: core::ptr::null(),
                len: 0,
            }; PROGRAM_COUNT],
            heap_ptr: core::ptr::null_mut(),
            heap_len: 0,
            ap_trampoline: 0,
            cpu_count: 0,
            apic_ids: [0; 8],
        };
        let view = View::new(&info);
        view.clear();
        view.face(Face::at(19 * 3600 + 35 * 60).unwrap());
        let background = pixels.clone();
        let mut simple = std::vec::Vec::new();
        for mode in [OrbitMode::Simple, OrbitMode::Ticks] {
            view.cycle(None, None, OrbitMode::Off, mode);
            let stationary = pixels.clone();
            if mode == OrbitMode::Simple {
                simple = stationary.clone();
            }
            let mut previous = None;
            for phase in (0..100000).step_by(100) {
                let current = Some(point_at(phase, view.half() * 3 / 4));
                view.cycle(previous, current, mode, mode);
                previous = current;
            }
            view.cycle(previous, None, mode, mode);
            assert_eq!(
                pixels, stationary,
                "dot must restore every crossed orbit pixel and tick"
            );
            if mode == OrbitMode::Ticks {
                view.cycle(None, None, mode, OrbitMode::Simple);
                assert_eq!(
                    pixels, simple,
                    "switching P to C removes all nine extra ticks"
                );
                view.cycle(None, None, OrbitMode::Simple, OrbitMode::Off);
            } else {
                view.cycle(None, None, mode, OrbitMode::Off);
            }
            assert_eq!(
                pixels, background,
                "hiding must preserve all five indicators and erase the orbit"
            );
        }
    }
}

impl<'a> View<'a> {
    pub fn new(info: &'a BootInfo) -> Self {
        let half = (info.width.min(info.height) / 5).clamp(1, 160);
        Self {
            info,
            x: info.width / 2,
            y: (info.height / 2).saturating_sub(24),
            half,
            radius: (half / 3).max(1),
        }
    }

    fn rect(&self, x: usize, y: usize, width: usize, height: usize, color: u32) {
        for py in y..y.saturating_add(height).min(self.info.height) {
            for px in x..x.saturating_add(width).min(self.info.width) {
                unsafe {
                    core::ptr::write_volatile(
                        self.info.fb_ptr.add(py * self.info.stride + px),
                        color,
                    );
                }
            }
        }
    }

    pub fn clear(&self) {
        self.rect(0, 0, self.info.width, self.info.height, BACKGROUND);
    }

    pub fn half(&self) -> usize {
        self.half
    }

    pub fn cycle(
        &self,
        previous: Option<(isize, isize)>,
        current: Option<(isize, isize)>,
        old_mode: OrbitMode,
        mode: OrbitMode,
    ) {
        if let Some(point) = previous {
            self.dot(point, BACKGROUND);
        }
        if old_mode != mode {
            self.orbit(old_mode, true);
        }
        // Restore the thin orbit and any tick uncovered by the moving dot.
        self.orbit(mode, false);
        if let Some(point) = current {
            self.dot(point, CYCLE);
        }
    }

    fn plot(&self, x: isize, y: isize, color: u32) {
        let px = self.x as isize + x;
        let py = self.y as isize + y;
        if px >= 0 && py >= 0 {
            self.rect(px as usize, py as usize, 1, 1, color);
        }
    }

    fn dot(&self, (x, y): (isize, isize), color: u32) {
        // Old moving tick length was 2*(half/24); the dot diameter is half
        // that length (6 pixels at the usual size), with a half-pixel center.
        let diameter = (self.half / 24).max(2) as isize;
        for dy in 0..diameter {
            for dx in 0..diameter {
                if (2 * dx + 1 - diameter).pow(2) + (2 * dy + 1 - diameter).pow(2)
                    <= diameter.pow(2)
                {
                    self.plot(x + dx - diameter / 2, y + dy - diameter / 2, color);
                }
            }
        }
    }

    fn orbit(&self, mode: OrbitMode, erase: bool) {
        if mode == OrbitMode::Off {
            return;
        }
        let color = if erase {
            BACKGROUND
        } else if mode == OrbitMode::Simple {
            ORBIT_SIMPLE
        } else {
            ORBIT_TICKS
        };
        let radius = self.half * 3 / 4;
        let (mut x, mut y) = (radius as isize, 0isize);
        let mut error = 1 - x;
        while x >= y {
            for (px, py) in [
                (x, y),
                (y, x),
                (-y, x),
                (-x, y),
                (-x, -y),
                (-y, -x),
                (y, -x),
                (x, -y),
            ] {
                self.plot(px, py, color);
            }
            y += 1;
            if error < 0 {
                error += 2 * y + 1;
            } else {
                x -= 1;
                error += 2 * (y - x) + 1;
            }
        }
        for index in 0..if mode == OrbitMode::Ticks { 10 } else { 1 } {
            let length = if index == 0 {
                (self.half / 24).max(1)
            } else {
                (self.half / 48).max(1)
            };
            let color = if erase {
                BACKGROUND
            } else if index == 0 {
                START_TICK
            } else {
                SMALL_TICK
            };
            self.line(
                point_at(index * 10000, radius.saturating_sub(length)),
                point_at(index * 10000, radius + length),
                color,
            );
        }
    }

    fn line(&self, (mut x, mut y): (isize, isize), (end_x, end_y): (isize, isize), color: u32) {
        let dx = (end_x - x).abs();
        let dy = -(end_y - y).abs();
        let sx = if x < end_x { 1 } else { -1 };
        let sy = if y < end_y { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.plot(x, y, color);
            if x == end_x && y == end_y {
                break;
            }
            let twice = error * 2;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
        }
    }

    fn circle(&self, x: usize, y: usize, color: u32) {
        let r = self.radius as isize;
        for dy in -r..=r {
            for dx in -r..=r {
                let distance = dx * dx + dy * dy;
                if distance > r * r {
                    continue;
                }
                let px = x as isize + dx;
                let py = y as isize + dy;
                if px < 0 || py < 0 {
                    continue;
                }
                let pixel = if distance > (r - 1) * (r - 1) {
                    RIM
                } else {
                    color
                };
                self.rect(px as usize, py as usize, 1, 1, pixel);
            }
        }
    }

    pub fn face(&self, face: Face) {
        let left = self.x.saturating_sub(self.half);
        let top = self.y.saturating_sub(self.half);
        self.circle(self.x + self.half, top, face.corners[0]);
        self.circle(self.x + self.half, self.y + self.half, face.corners[1]);
        self.circle(left, self.y + self.half, face.corners[2]);
        self.circle(left, top, face.corners[3]);
        self.circle(self.x, self.y, face.center);
    }

    pub fn digital(&self, text: &[u8; 8], visible: bool) {
        // One-pixel seven-segment strokes keep the digital aid visually quiet.
        let x = self.x.saturating_sub(62);
        let y = self.y + self.half + self.radius + 28;
        self.rect(x, y, 124, 21, BACKGROUND);
        if !visible {
            return;
        }
        for (i, &ch) in text.iter().enumerate() {
            let x = x + i * 16;
            if ch == b':' {
                self.rect(x + 5, y + 6, 1, 1, DIGITS);
                self.rect(x + 5, y + 14, 1, 1, DIGITS);
                continue;
            }
            // Bits a,b,c,d,e,f,g: top; upper/lower right; bottom;
            // lower/upper left; middle.
            let bits = match ch {
                b'0'..=b'9' => [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f]
                    [(ch - b'0') as usize],
                _ => 0x40,
            };
            for (bit, sx, sy, w, h) in [
                (0, 1, 0, 9, 1),
                (1, 10, 1, 1, 9),
                (2, 10, 11, 1, 9),
                (3, 1, 20, 9, 1),
                (4, 0, 11, 1, 9),
                (5, 0, 1, 1, 9),
                (6, 1, 10, 9, 1),
            ] {
                if bits & (1 << bit) != 0 {
                    self.rect(x + sx, y + sy, w, h, DIGITS);
                }
            }
        }
    }

    pub fn hints(&self, visible: bool) {
        let color = if visible { HINT } else { BACKGROUND };
        self.text(b"DZEN CLOCK", 24, color);
        self.text(
            b"D: DIGITS   C: ORBIT   P: 10S TICKS   H: TEXT   CTRL+Z: SHELL   ESC: EXIT",
            self.info.height.saturating_sub(32),
            color,
        );
    }

    fn text(&self, text: &[u8], y: usize, color: u32) {
        let x = self.info.width.saturating_sub(text.len() * 8) / 2;
        for (i, &ch) in text.iter().enumerate() {
            let glyph = FONT[ch.saturating_sub(32).min(63) as usize];
            for row in 0..8 {
                for col in 0..8 {
                    if glyph & (1 << ((7 - row) * 8 + 7 - col)) != 0 {
                        self.rect(x + i * 8 + col, y + row, 1, 1, color);
                    }
                }
            }
        }
    }
}
