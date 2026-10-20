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

/// The pixel face's layout on a framebuffer (its own copy of where it is and how large: a window's changes, u009).
pub struct View {
    info: BootInfo,
    x: usize,
    y: usize,
    half: usize,
    radius: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{ProgramImage, BOOT_IMAGES, BOOT_PREFIX_IMAGES, PIXEL_BGR};

    // A framebuffer of `width` × `height` at the start of `pixels`.
    fn framebuffer(pixels: &mut [u32], width: usize, height: usize) -> BootInfo {
        BootInfo {
            fb_ptr: pixels.as_mut_ptr(),
            width,
            height,
            stride: width,
            programs: [ProgramImage {
                data: core::ptr::null(),
                len: 0,
            }; BOOT_PREFIX_IMAGES],
            heap_ptr: core::ptr::null_mut(),
            heap_len: 0,
            ap_trampoline: 0,
            cpu_count: 0,
            apic_ids: [0; 8],
            memory_map: core::ptr::null(),
            memory_map_len: 0,
            pixel_format: PIXEL_BGR,
            pixel_masks: [0; 3],
            acpi_rsdp: 0,
            cpu_features: 0,
            abi_version: crate::abi::ABI_VERSION,
            boot_volume: Default::default(),
            boot_slot: Default::default(),
            launch: Default::default(),
            efi_runtime: 0,
            device_tree: 0,
            photo_pause_s: 0,
            extra_programs: [ProgramImage { data: core::ptr::null(), len: 0 }; BOOT_IMAGES - BOOT_PREFIX_IMAGES],
        }
    }

    #[test]
    fn full_turn_restores_orbit_ticks_and_hiding_restores_background() {
        let mut pixels = std::vec![0u32; 640 * 480];
        let info = framebuffer(&mut pixels, 640, 480);
        let view = View::new(&info);
        view.clear();
        let face = Face::at(19 * 3600 + 35 * 60).unwrap();
        view.face(face);
        let background = pixels.clone();
        let mut simple = std::vec::Vec::new();
        let corner = face.hour;
        for mode in [OrbitMode::Simple, OrbitMode::Ticks] {
            view.cycle(None, None, (OrbitMode::Off, corner), (mode, corner));
            let stationary = pixels.clone();
            if mode == OrbitMode::Simple {
                simple = stationary.clone();
            }
            let mut previous = None;
            for phase in (0..100000).step_by(100) {
                let current = Some(point_at(phase, view.half() * 3 / 4, corner));
                view.cycle(previous, current, (mode, corner), (mode, corner));
                previous = current;
            }
            view.cycle(previous, None, (mode, corner), (mode, corner));
            assert_eq!(
                pixels, stationary,
                "dot must restore every crossed orbit pixel and tick"
            );
            if mode == OrbitMode::Ticks {
                view.cycle(None, None, (mode, corner), (OrbitMode::Simple, corner));
                assert_eq!(
                    pixels, simple,
                    "switching P to C removes all nine extra ticks"
                );
                view.cycle(None, None, (OrbitMode::Simple, corner), (OrbitMode::Off, corner));
            } else {
                view.cycle(None, None, (mode, corner), (OrbitMode::Off, corner));
            }
            assert_eq!(
                pixels, background,
                "hiding must preserve all five indicators and erase the orbit"
            );
        }
    }

    #[test]
    fn the_zero_tick_points_at_the_active_hour_and_moves_with_it() {
        // 000-APP-0080: at 19:35 the hour is the top right corner's, and the orbit's zero tick points there, in the
        // ticks' grey; at 06:00 the bottom left takes the hour, and the tick moves there with nothing left behind.
        let (width, height) = (640, 480);
        let mut pixels = std::vec![0u32; width * height];
        let info = framebuffer(&mut pixels, width, height);
        let view = View::new(&info);
        let (cx, cy, radius) = (view.x as isize, view.y as isize, view.half() as isize * 3 / 4);
        let at = |pixels: &[u32], (x, y): (isize, isize)| pixels[(cy + y) as usize * width + (cx + x) as usize];
        let start = |pixels: &[u32]| -> std::vec::Vec<(isize, isize)> {
            (0..height as isize).flat_map(|y| (0..width as isize).map(move |x| (x - cx, y - cy)))
                .filter(|&(x, y)| x * x + y * y < (radius + 8).pow(2) && at(pixels, (x, y)) == START_TICK).collect()
        };
        view.clear();
        let evening = Face::at(19 * 3600 + 35 * 60).unwrap();
        assert_eq!(evening.hour, 0);
        view.face(evening);
        view.cycle(None, None, (OrbitMode::Off, evening.hour), (OrbitMode::Ticks, evening.hour));
        let tick = start(&pixels);
        assert!(tick.len() >= 4 && tick.iter().all(|&(x, y)| x > 0 && y < 0 && (x + y).abs() <= 1), "{:?}", tick);
        assert!(!pixels.iter().any(|&p| crate::face::COLORS.contains(&p) && p != evening.corners[0] && p != evening.center),
                "no colored mark besides the indicators");
        // The dot at zero seconds sits on the tick.
        let zero = point_at(0, view.half() * 3 / 4, evening.hour);
        assert!(tick.iter().any(|&(x, y)| (x - zero.0).abs() <= 1 && (y - zero.1).abs() <= 1), "{:?} {:?}", zero, tick);

        let (before, morning) = (Face::at(6 * 3600 - 1).unwrap(), Face::at(6 * 3600).unwrap());
        assert_eq!((before.hour, morning.hour), (1, 2));
        view.clear();
        view.face(before);
        view.cycle(None, None, (OrbitMode::Off, before.hour), (OrbitMode::Ticks, before.hour));
        view.face(morning);
        view.cycle(None, None, (OrbitMode::Ticks, before.hour), (OrbitMode::Ticks, morning.hour));
        let moved = pixels.clone();
        view.clear();
        view.face(morning);
        view.cycle(None, None, (OrbitMode::Off, morning.hour), (OrbitMode::Ticks, morning.hour));
        assert_eq!(moved, pixels, "the hour's change redraws the orbit from its new zero, with nothing left behind");
        assert!(start(&pixels).iter().all(|&(x, y)| x < 0 && y > 0), "06:00: toward the bottom left");
    }

    #[test]
    fn the_face_is_laid_out_for_the_window_it_is_given() {
        // A window resized (issue u009): the face centred in its new size, larger in a larger one, and nothing drawn
        // outside it.
        const GUARD: u32 = 0x00ABCDEF;
        let mut halves = std::vec::Vec::new();
        for (width, height) in [(400, 320), (1264, 752), (160, 96), (624, 736)] {
            let mut pixels = std::vec![GUARD; width * height + 4096];
            let info = framebuffer(&mut pixels, width, height);
            let view = View::new(&info);
            view.clear();
            view.face(Face::at(19 * 3600 + 35 * 60).unwrap());
            let lit: std::vec::Vec<usize> = (0..width * height).filter(|&i| pixels[i] != BACKGROUND).map(|i| i % width).collect();
            let (left, right) = (*lit.iter().min().unwrap(), *lit.iter().max().unwrap());
            assert!((left + right) / 2 >= width / 2 - 2 && (left + right) / 2 <= width / 2 + 2, "{}x{}: {}..{}", width, height, left, right);
            view.hints(true, false);
            view.digital(b"19:35:05", true);
            view.cycle(None, Some(point_at(25000, view.half() * 3 / 4, 0)), (OrbitMode::Off, 0), (OrbitMode::Ticks, 0));
            assert!(pixels[width * height..].iter().all(|&p| p == GUARD), "{}x{}: drawn past the framebuffer", width, height);
            halves.push(view.half());
        }
        assert!(halves[1] > halves[0] && halves[0] > halves[2], "{:?}", halves);
    }
}

impl View {
    pub fn new(info: &BootInfo) -> Self {
        let half = (info.width.min(info.height) / 5).clamp(1, 160);
        Self {
            info: *info,
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

    /// The dot moved from `previous` to `current`; the orbit's mode, and the corner its ticks start at (the active
    /// hour's, 000-APP-0080), from `old` to the new ones: the old orbit goes when either changes.
    pub fn cycle(
        &self,
        previous: Option<(isize, isize)>,
        current: Option<(isize, isize)>,
        (old_mode, old_corner): (OrbitMode, usize),
        (mode, corner): (OrbitMode, usize),
    ) {
        if let Some(point) = previous {
            self.dot(point, BACKGROUND);
        }
        if old_mode != mode || old_corner != corner {
            self.orbit(old_mode, old_corner, true);
        }
        // Restore the thin orbit and any tick uncovered by the moving dot.
        self.orbit(mode, corner, false);
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

    fn orbit(&self, mode: OrbitMode, corner: usize, erase: bool) {
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
                point_at(index * 10000, radius.saturating_sub(length), corner),
                point_at(index * 10000, radius + length, corner),
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

    /// The title and the keys; `switch`: T switches to the text face (on the program's own screen).
    pub fn hints(&self, visible: bool, switch: bool) {
        let color = if visible { HINT } else { BACKGROUND };
        self.text(b"DZEN CLOCK", 24, color);
        let keys: &[u8] = if switch { b"D: DIGITS   C: ORBIT   P: 10S TICKS   H: HINTS   T: TEXT FACE   CTRL+Z: SHELL   ESC: EXIT" }
                          else { b"D: DIGITS   C: ORBIT   P: 10S TICKS   H: HINTS   CTRL+Z: SHELL   ESC: EXIT" };
        self.text(keys, self.info.height.saturating_sub(32), color);
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
