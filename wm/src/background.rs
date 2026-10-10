//! The desktop background of `wm` (000-APP-0047, 000-APP-0050), as `data/wm.conf` chooses it: the dim `░` cells as
//! before (`none`), a moving low-contrast pattern of one of four kinds (`abstract`, the default) or an image (BMP, PNG
//! or baseline JPEG), with the time, the date and the CPU load drawn over it, pale. The pattern's speed, contrast and
//! complexity and the information's brightness are set too. It is drawn into a frame of pixels that `wm` copies to the
//! desktop's cells nothing covers. Host-tested in tests/wm_host.rs.
use crate::{jpegdec, png};
use alloc::{format, string::String, vec, vec::Vec};

/// Where `wm` keeps the configuration, among the user's files.
pub const FILE: &str = "data/wm.conf";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picture { None, Abstract, Image(String) }

/// The abstract pattern's kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern { Waves, Rings, Aurora, Blobs }

pub const PATTERNS: [(&str, Pattern); 4] = [("waves", Pattern::Waves), ("rings", Pattern::Rings), ("aurora", Pattern::Aurora), ("blobs", Pattern::Blobs)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place { TopLeft, TopRight, Center, BottomLeft, BottomRight }

const PLACES: [(&str, Place); 5] = [("top-left", Place::TopLeft), ("top-right", Place::TopRight), ("center", Place::Center), ("bottom-left", Place::BottomLeft), ("bottom-right", Place::BottomRight)];

/// The ranges of the numbers: the pattern's speed (1: two pixels a second across a screen 1024 pixels wide), its
/// contrast and its complexity, and the information's brightness (percent).
pub const SPEED: (u8, u8) = (1, 50);
pub const CONTRAST: (u8, u8) = (0, 100);
pub const COMPLEXITY: (u8, u8) = (1, 5);
pub const INFO: (u8, u8) = (0, 100);

/// What the background shows. `None` shows nothing over the `░` cells, whatever `show` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub picture: Picture, pub pattern: Pattern, pub speed: u8, pub contrast: u8, pub complexity: u8, pub info: u8,
    pub time: bool, pub date: bool, pub cpu: bool, pub net: bool, pub place: Place,
}

impl Default for Config {
    fn default() -> Self {
        Self { picture: Picture::Abstract, pattern: Pattern::Waves, speed: 1, contrast: 20, complexity: 2, info: 30, time: true, date: true, cpu: true, net: false, place: Place::BottomRight }
    }
}

impl Config {
    /// `key = value` lines (or parts of a line between `;`), `#` starting a comment: `background = none | abstract |
    /// image <file>`, `pattern = waves | rings | aurora | blobs`, `speed = 1..50`, `contrast = 0..100`,
    /// `complexity = 1..5`, `info = 0..100`, `show = time, date, cpu, net` (or `none`),
    /// `place = top-left | top-right | center | bottom-left | bottom-right`. What is not said stays the default; each
    /// part not understood is named in the second value.
    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let mut config = Self::default();
        let mut problems = Vec::new();
        let number = |value: &str, (lo, hi): (u8, u8)| value.trim_end_matches('%').trim_end().parse::<u8>().ok().filter(|n| (lo..=hi).contains(n));
        for (index, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("");
            let number_of_line = index + 1;
            for part in line.split(';').map(str::trim).filter(|p| !p.is_empty()) {
                let Some((key, value)) = part.split_once('=') else { problems.push(format!("line {}: no '='", number_of_line)); continue };
                let (key, value) = (key.trim(), value.trim());
                let mut wrong = |what: &str| problems.push(format!("line {}: {}", number_of_line, what));
                match key {
                    "background" => {
                        let (kind, rest) = value.split_once(char::is_whitespace).map_or((value, ""), |(k, r)| (k, r.trim()));
                        match (kind, rest) {
                            ("none", "") => config.picture = Picture::None,
                            ("abstract", "") => config.picture = Picture::Abstract,
                            ("image", file) if !file.is_empty() => config.picture = Picture::Image(String::from(file)),
                            _ => wrong("background is none, abstract or image <file>"),
                        }
                    }
                    "pattern" => match PATTERNS.iter().find(|p| p.0 == value) {
                        Some(&(_, pattern)) => config.pattern = pattern,
                        None => wrong("pattern is waves, rings, aurora or blobs"),
                    },
                    "speed" => match number(value, SPEED) { Some(n) => config.speed = n, None => wrong("speed is 1 to 50") },
                    "contrast" => match number(value, CONTRAST) { Some(n) => config.contrast = n, None => wrong("contrast is 0 to 100") },
                    "complexity" => match number(value, COMPLEXITY) { Some(n) => config.complexity = n, None => wrong("complexity is 1 to 5") },
                    "info" => match number(value, INFO) { Some(n) => config.info = n, None => wrong("info (its brightness) is 0 to 100") },
                    "show" => {
                        let words: Vec<&str> = value.split(',').map(str::trim).filter(|w| !w.is_empty()).collect();
                        match words.iter().find(|w| !["time", "date", "cpu", "net", "none"].contains(w)) {
                            Some(word) => wrong(&format!("show takes time, date, cpu, net or none, not {}", word)),
                            None => [config.time, config.date, config.cpu, config.net] = ["time", "date", "cpu", "net"].map(|name| words.contains(&name)),
                        }
                    }
                    "place" => match PLACES.iter().find(|p| p.0 == value) {
                        Some(&(_, place)) => config.place = place,
                        None => wrong("place is top-left, top-right, center, bottom-left or bottom-right"),
                    },
                    _ => wrong(&format!("unknown key {}", key)),
                }
            }
        }
        (config, problems)
    }

    /// The configuration as `parse` reads it back.
    pub fn format(&self) -> String {
        let picture = match &self.picture { Picture::None => String::from("none"), Picture::Abstract => String::from("abstract"), Picture::Image(file) => format!("image {}", file) };
        let shown: Vec<&str> = [(self.time, "time"), (self.date, "date"), (self.cpu, "cpu"), (self.net, "net")].iter().filter(|s| s.0).map(|s| s.1).collect();
        let place = PLACES.iter().find(|p| p.1 == self.place).map_or("bottom-right", |p| p.0);
        let pattern = PATTERNS.iter().find(|p| p.1 == self.pattern).map_or("waves", |p| p.0);
        format!("# wm's desktop background (000-APP-0047, 000-APP-0050)\n\
                 # background = none | abstract | image <file> (BMP, PNG or JPEG); pattern = waves | rings | aurora | blobs\n\
                 # speed = 1..50; contrast = 0..100; complexity = 1..5; info = 0..100 (the brightness of what is shown)\n\
                 # show = time, date, cpu, net | none; place = top-left | top-right | center | bottom-left | bottom-right\n\
                 background = {}\npattern = {}\nspeed = {}\ncontrast = {}\ncomplexity = {}\ninfo = {}\nshow = {}\nplace = {}\n",
                picture, pattern, self.speed, self.contrast, self.complexity, self.info, if shown.is_empty() { String::from("none") } else { shown.join(", ") }, place)
    }
}

/// The pattern's darkest and lightest colours at the default contrast: the desktop's own blue, little apart.
pub const DARK: u32 = 0x10_1A_24;
pub const LIGHT: u32 = 0x24_38_4C;
// The pattern's middle colour, and how far its ends lie from it at contrast 100, by channel.
const MID: u32 = 0x1A_29_38;
const REACH: [i32; 3] = [50, 75, 100];
// The information's colour at brightness 100, and its shadow's.
const BRIGHT: u32 = 0xC8_D8_E8;
const SHADE: u32 = 0x08_0E_14;
/// One sine period of the table.
const PERIOD: usize = 1024;

// A sine over PERIOD steps in -127..=127 (Bhaskara I's approximation, within about 1 %).
const SINE: [i16; PERIOD] = {
    let mut table = [0i16; PERIOD];
    let mut i = 0;
    while i < PERIOD {
        let x = ((i % (PERIOD / 2)) * 180 / (PERIOD / 2)) as i64; // degrees in the half period
        let p = x * (180 - x);
        let value = (4 * p * 127 / (40_500 - p)) as i16;
        table[i] = if i < PERIOD / 2 { value } else { -value };
        i += 1;
    }
    table
};

// The table at `i` (wrapped: a step back from 0 is the period's last).
fn at(i: usize) -> i32 { SINE[i % PERIOD] as i32 }

fn mix(a: u32, b: u32, k: u32) -> u32 {
    let channel = |shift: u32| { let (x, y) = ((a >> shift) & 0xFF, (b >> shift) & 0xFF); ((x * (255 - k) + y * k) / 255) << shift };
    channel(16) | channel(8) | channel(0)
}

/// The pattern's darkest and lightest colours at `contrast` (0 to 100; 20, the default, gives DARK and LIGHT).
pub fn colours(contrast: u8) -> (u32, u32) {
    let c = contrast.min(CONTRAST.1) as i32;
    let end = |sign: i32| (0..3).fold(0u32, |colour, i| {
        let shift = 16 - 8 * i as u32;
        colour | (((MID >> shift & 0xFF) as i32 + sign * REACH[i] * c / 100).clamp(0, 255) as u32) << shift
    });
    (end(-1), end(1))
}

/// The information's colours at brightness `info` (0 to 100): the text, the CPU graph's bars and the text's shadow.
pub fn ink(info: u8) -> (u32, u32, u32) {
    let k = info.min(INFO.1) as u32 * 255 / 100;
    let text = mix(MID, BRIGHT, k);
    (text, mix(MID, text, 160), mix(MID, SHADE, (k * 2).min(255)))
}

// Each 2 × 2 block of the frame the palette's colour `level` gives at its top-left pixel.
fn fill(frame: &mut [u32], width: usize, height: usize, palette: &[u32], level: impl Fn(usize, usize) -> usize) {
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let colour = palette[level(x, y).min(255)];
            for row in y..(y + 2).min(height) { for pixel in frame[row * width + x..row * width + (x + 2).min(width)].iter_mut() { *pixel = colour; } }
        }
    }
}

/// Frame `phase` of the pattern `config` chooses into `frame` (`width` × `height` pixels, row after row), between its
/// contrast's colours. The phase counts eighths of a table step: 16 a second at speed 1.
pub fn pattern(frame: &mut [u32], width: usize, height: usize, config: &Config, phase: u32) {
    if width == 0 || height == 0 { return; }
    let (dark, light) = colours(config.contrast);
    let palette: Vec<u32> = (0..256).map(|k| mix(dark, light, k)).collect();
    let k = config.complexity.clamp(COMPLEXITY.0, COMPLEXITY.1) as usize;
    let p = phase as usize;
    let (w, h) = (width as i64, height as i64);
    // Where a point on a slow path is: the table at `a` across `span` around `middle`.
    let swing = |middle: i64, span: i64, a: usize| middle + span * at(a) as i64 / 127;
    match config.pattern {
        // Four sines crossing at slants; complexity 2 puts one period across the screen.
        Pattern::Waves => fill(frame, width, height, &palette, |x, y| {
            let (u, v) = (x * PERIOD * k / (2 * width), y * PERIOD * k / (2 * height));
            let sum = at((v * 2 + PERIOD * 4).wrapping_sub(p / 16)) + at(u + p / 8) + at(u + v + p / 12) + at((u * 3 / 2 + PERIOD * 4 - v).wrapping_sub(p / 24));
            ((sum + 508) * 255 / 1016) as usize
        }),
        // Ripples spreading from centres that drift on slow ellipses: (complexity + 3) / 2 of them, complexity + 1
        // rings across the screen's height.
        Pattern::Rings => {
            let n = (k + 3) / 2;
            let centres: Vec<(i64, i64)> = (0..n).map(|i| {
                let a = p / 64 * (i + 1) + i * PERIOD / n;
                (swing(w / 2, w * 3 / 8, a), swing(h / 2, h * 3 / 8, a * 2 / 3 + PERIOD / 4))
            }).collect();
            let rings = ((k + 1) * PERIOD) as u64;
            fill(frame, width, height, &palette, |x, y| {
                let sum: i32 = centres.iter().map(|&(cx, cy)| {
                    let (dx, dy) = (x as i64 - cx, y as i64 - cy);
                    let d = ((dx * dx + dy * dy) as u64).isqrt() * rings / height as u64;
                    at((d as usize).wrapping_sub(p / 8))
                }).sum();
                ((sum + 127 * n as i32) * 255 / (254 * n as i32)) as usize
            })
        }
        // Curtains of light hanging from the top, bending and swaying, some brighter than others, fading downwards;
        // complexity + 1 of them across the screen.
        Pattern::Aurora => fill(frame, width, height, &palette, |x, y| {
            let (u, v) = (x * PERIOD * (k + 1) / width, y * PERIOD / (2 * height));
            let s = (u as i32 + at(v * 2 + u / 4 + p / 24) * 3 / 4 + PERIOD as i32) as usize;
            let ray = (at(s + p / 8) + at(s * 3 + p / 6) / 3).max(0) as usize; // 0..=169
            let glow = (191 + at(u / 3 + PERIOD - p / 32 % PERIOD)) as usize; // 64..=318
            let fade = 48 + (height - y) * (height - y) * 207 / (height * height);
            32 + ray * glow / 169 * fade / 255 * 223 / 318
        }),
        // Soft blobs of light drifting on crossing paths: complexity + 2 of them, smaller the more there are.
        Pattern::Blobs => {
            let n = k + 2;
            let r = (h / (k as i64 + 3)).max(1);
            let blobs: Vec<(i64, i64)> = (0..n).map(|i| {
                let a = p / 48 * (i % 3 + 1) + i * PERIOD * 2 / 7;
                let b = p / 64 * (i % 2 + 2) + i * PERIOD / 3 + PERIOD / 4;
                (swing(w / 2, w / 2 - r / 2, a), swing(h / 2, h / 2 - r / 2, b))
            }).collect();
            let r2 = (r * r) as u64;
            fill(frame, width, height, &palette, |x, y| {
                let field: u64 = blobs.iter().map(|&(bx, by)| { let (dx, dy) = (x as i64 - bx, y as i64 - by); r2 * 256 / ((dx * dx + dy * dy) as u64 + r2) }).sum();
                field as usize
            })
        }
    }
}

/// The largest picture's side `cover` takes.
pub const LIMIT: usize = 8192;

// A picture scaled to cover a frame (cut at its longer sides, its middle kept), its rows taken in order as a decoder
// gives them: shrinking averages the pixels each frame pixel covers, growing repeats them.
struct Cover<'a> { frame: &'a mut [u32], width: usize, height: usize, num: usize, den: usize, ox: usize, oy: usize, sums: Vec<[u32; 4]>, row: Option<usize>, next: usize }

impl<'a> Cover<'a> {
    fn new(frame: &'a mut [u32], width: usize, height: usize, (iw, ih): (usize, usize)) -> Result<Self, &'static str> {
        if iw == 0 || ih == 0 || iw > LIMIT || ih > LIMIT { return Err("a picture of no size, or larger than 8192 pixels a side"); }
        // Frame pixels per picture pixel: num / den, the larger of the two ratios, so the picture covers the frame.
        let (num, den) = if width * ih >= height * iw { (width, iw) } else { (height, ih) };
        let (ox, oy) = ((iw * num / den).saturating_sub(width) / 2, (ih * num / den).saturating_sub(height) / 2);
        Ok(Self { frame, width, height, num, den, ox, oy, sums: vec![[0; 4]; if num < den { width } else { 0 }], row: None, next: 0 })
    }

    fn take(&mut self, iy: usize, pixels: &[u32]) {
        let (width, num, den) = (self.width, self.num, self.den);
        if pixels.is_empty() { return; }
        if num >= den {
            while self.next < self.height && (self.next + self.oy) * den / num <= iy {
                let line = &mut self.frame[self.next * width..self.next * width + width];
                for (x, p) in line.iter_mut().enumerate() { *p = pixels[((x + self.ox) * den / num).min(pixels.len() - 1)]; }
                self.next += 1;
            }
            return;
        }
        let fy = (iy * num / den).checked_sub(self.oy).filter(|&f| f < self.height);
        if fy != self.row { self.flush(); self.row = fy; }
        if fy.is_none() { return; }
        for (ix, &p) in pixels.iter().enumerate() {
            let Some(fx) = (ix * num / den).checked_sub(self.ox).filter(|&f| f < width) else { continue };
            let s = &mut self.sums[fx];
            s[0] += p >> 16 & 0xFF; s[1] += p >> 8 & 0xFF; s[2] += p & 0xFF; s[3] += 1;
        }
    }

    // The frame's row being summed, averaged into the frame.
    fn flush(&mut self) {
        let Some(fy) = self.row.take() else { return };
        for (fx, s) in self.sums.iter_mut().enumerate() {
            if s[3] > 0 { self.frame[fy * self.width + fx] = (s[0] / s[3]) << 16 | (s[1] / s[3]) << 8 | s[2] / s[3]; }
            *s = [0; 4];
        }
    }
}

// An uncompressed BMP (24 or 32 bits a pixel, rows from the bottom or the top): its size, and its rows top first.
fn bmp(data: &[u8], rows: &mut dyn FnMut(usize, &[u32])) -> Result<(), &'static str> {
    const BAD: &str = "not a BMP that can be read (24 or 32 bits a pixel, uncompressed)";
    let u32_at = |at: usize| data.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or(BAD);
    let u16_at = |at: usize| data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(BAD);
    let (offset, w, h) = (u32_at(10)? as usize, u32_at(18)? as i32, u32_at(22)? as i32);
    let (bits, compression) = (u16_at(28)?, u32_at(30)?);
    if !(bits == 24 || bits == 32) || !(compression == 0 || (compression == 3 && bits == 32)) || w <= 0 || h == 0 { return Err(BAD); }
    let (iw, ih) = (w as usize, h.unsigned_abs() as usize);
    let bytes = bits as usize / 8;
    let stride = (iw * bytes + 3) & !3;
    if iw > LIMIT || ih > LIMIT || offset + stride * ih > data.len() { return Err("a BMP cut short, or larger than 8192 pixels a side"); }
    let mut pixels = vec![0u32; iw];
    for y in 0..ih {
        let row = offset + if h > 0 { ih - 1 - y } else { y } * stride;
        for (x, p) in pixels.iter_mut().enumerate() { let at = row + x * bytes; *p = (data[at + 2] as u32) << 16 | (data[at + 1] as u32) << 8 | data[at] as u32; }
        rows(y, &pixels);
    }
    Ok(())
}

/// The picture in `data` (a BMP, a PNG or a baseline JPEG, told by its first bytes) scaled to cover `frame` of
/// `width` × `height` pixels, transparency over DARK; why not when it cannot be read.
pub fn cover(data: &[u8], frame: &mut [u32], width: usize, height: usize) -> Result<(), &'static str> {
    if width == 0 || height == 0 || frame.len() < width * height { return Err("no frame to cover"); }
    let size = if data.starts_with(b"BM") && data.len() >= 26 {
        let u32_at = |at: usize| u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as i32;
        Some((u32_at(18).max(0) as usize, u32_at(22).unsigned_abs() as usize))
    } else if data.starts_with(b"\x89PNG") { png::size(data) } else if data.starts_with(&[0xFF, 0xD8]) { jpegdec::size(data) } else { return Err("not a BMP, PNG or JPEG") };
    let mut cover = Cover::new(frame, width, height, size.ok_or("a picture cut short")?)?;
    let mut take = |y: usize, row: &[u32]| cover.take(y, row);
    match data[0] {
        b'B' => bmp(data, &mut take)?,
        0x89 => { png::decode(data, DARK, LIMIT, &mut take)?; }
        _ => { jpegdec::decode(data, LIMIT, &mut take)?; }
    }
    cover.flush();
    Ok(())
}

/// What is drawn over the picture: the seconds since midnight, the date (year, month, day), the CPU load of the last
/// samples (percent, oldest first) and a line about the network.
#[derive(Clone, Copy, Default)]
pub struct Info<'a> { pub seconds: Option<usize>, pub date: Option<(u32, u32, u32)>, pub cpu: &'a [u8], pub net: Option<&'a str> }

/// A glyph of the 8 × 16 font: its rows, the leftmost pixel in the top bit.
pub type Glyph<'a> = &'a dyn Fn(char) -> [u8; 16];

/// The samples the CPU graph shows.
pub const CPU_SAMPLES: usize = 60;

// `text` at (x, y) in the 8 × 16 font, each pixel `scale` × `scale`, over its shadow.
#[allow(clippy::too_many_arguments)]
fn text(frame: &mut [u32], width: usize, height: usize, x: usize, y: usize, text: &str, scale: usize, (colour, shadow): (u32, u32), glyph: Glyph) {
    for (shift, colour) in [(scale.div_ceil(2), shadow), (0, colour)] {
        for (index, ch) in text.chars().enumerate() {
            let rows = glyph(ch);
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..8 {
                    if bits & (0x80 >> col) == 0 { continue; }
                    let (px, py) = (x + shift + (index * 8 + col) * scale, y + shift + row * scale);
                    for dy in 0..scale { for dx in 0..scale { if px + dx < width && py + dy < height { frame[(py + dy) * width + px + dx] = colour; } } }
                }
            }
        }
    }
}

/// Where the information goes on a frame of `width` × `height` pixels, each `unit` screen pixels wide: its rectangle
/// (x, y, w, h), and the scales of the time's and the other lines' glyphs. None when it shows nothing.
pub fn layout(config: &Config, width: usize, height: usize, unit: usize) -> Option<((usize, usize, usize, usize), usize, usize)> {
    if config.picture == Picture::None || !(config.time || config.date || config.cpu || config.net) { return None; }
    let unit = unit.max(1);
    let (big, small, gap) = ((6 / unit).max(1), (2 / unit).max(1), 8 / unit);
    let mut w = 0;
    let mut h = 0;
    let mut add = |line_w: usize, line_h: usize| { w = w.max(line_w); h += if h > 0 { gap } else { 0 } + line_h; };
    if config.time { add(5 * 8 * big, 16 * big); }
    if config.date { add(10 * 8 * small, 16 * small); }
    if config.cpu { add(5 * 8 * big, 16 * small + gap / 2 + 48 / unit); }
    if config.net { add(24 * 8 * small, 16 * small); }
    // Two cells from the edges: clear of the top bar and the status line.
    let (mx, my) = (32 / unit, 32 / unit);
    if w + 2 * mx > width || h + 2 * my > height { return None; }
    let x = match config.place { Place::TopLeft | Place::BottomLeft => mx, Place::Center => (width - w) / 2, _ => width - mx - w };
    let y = match config.place { Place::TopLeft | Place::TopRight => my, Place::Center => (height - h) / 2, _ => height - my - h };
    Some(((x, y, w, h), big, small))
}

/// Draws the information `config` asks for over `frame`, at its place.
pub fn overlay(frame: &mut [u32], width: usize, height: usize, unit: usize, config: &Config, info: &Info, glyph: Glyph) {
    let Some(((x, mut y, w, _), big, small)) = layout(config, width, height, unit) else { return };
    let gap = 8 / unit.max(1);
    let (colour, graph, shadow) = ink(config.info);
    let ink = (colour, shadow);
    // Each line right-aligned in the block on the right, left-aligned elsewhere.
    let right = matches!(config.place, Place::TopRight | Place::BottomRight);
    let start = |line_w: usize| if right { x + w - line_w } else { x };
    if config.time {
        let line = match info.seconds { Some(s) => format!("{:02}:{:02}", s / 3600 % 24, s / 60 % 60), None => String::from("--:--") };
        text(frame, width, height, start(5 * 8 * big), y, &line, big, ink, glyph);
        y += 16 * big + gap;
    }
    if config.date {
        let line = match info.date { Some((year, month, day)) => format!("{:04}-{:02}-{:02}", year, month, day), None => String::from("----------") };
        text(frame, width, height, start(10 * 8 * small), y, &line, small, ink, glyph);
        y += 16 * small + gap;
    }
    if config.cpu {
        let graph_w = 5 * 8 * big;
        let label = match info.cpu.last() { Some(p) => format!("CPU {:3}%", p), None => String::from("CPU    -") };
        text(frame, width, height, start(graph_w), y, &label, small, ink, glyph);
        y += 16 * small + gap / 2;
        let (gx, gh) = (start(graph_w), 48 / unit.max(1));
        // A bar a sample, the newest at the right; a line along the bottom.
        let samples = &info.cpu[info.cpu.len().saturating_sub(CPU_SAMPLES)..];
        let bar = (graph_w / CPU_SAMPLES).max(1);
        for (i, &p) in samples.iter().enumerate() {
            let bx = gx + graph_w - (samples.len() - i) * bar;
            let bh = (p.min(100) as usize * gh).div_ceil(100);
            for py in y + gh - bh..y + gh { for px in bx..bx + bar { if px < width && py < height { frame[py * width + px] = graph; } } }
        }
        for px in gx..gx + graph_w { if px < width && y + gh < height { frame[(y + gh) * width + px] = colour; } }
        y += gh + gap;
    }
    if config.net {
        let line = info.net.unwrap_or("NET: NO COUNTERS");
        text(frame, width, height, start(24 * 8 * small), y, line, small, ink, glyph);
    }
}

/// The background's state in `wm`: its frame (the screen's pixels, or half of them each way when that would not fit
/// in 4 MiB), the picture scaled once, the pattern's phase and the CPU samples.
pub struct Backdrop { pub config: Config, pub width: usize, pub height: usize, pub unit: usize, pub frame: Vec<u32>, base: Option<Vec<u32>>, pub phase: u32, carry: usize, pub cpu: Vec<u8> }

impl Backdrop {
    /// For a screen of `screen` pixels; `image` reads the file `image <file>` names (None: the pattern is drawn instead,
    /// and the configuration keeps the image).
    pub fn new(config: Config, screen: (usize, usize), image: &mut dyn FnMut(&str) -> Option<Vec<u8>>) -> (Self, Option<String>) {
        let unit = if screen.0 * screen.1 * 4 <= 4 << 20 { 1 } else { 2 };
        let (width, height) = (screen.0 / unit, screen.1 / unit);
        let mut backdrop = Self { config: Config { picture: Picture::None, ..config.clone() }, width, height, unit, frame: Vec::new(), base: None, phase: 0, carry: 0, cpu: Vec::new() };
        let problem = backdrop.change(config, image);
        (backdrop, problem)
    }

    /// Another configuration: the picture read again only when it is another.
    pub fn change(&mut self, config: Config, image: &mut dyn FnMut(&str) -> Option<Vec<u8>>) -> Option<String> {
        // An image that could not be read is tried again.
        let same = config.picture == self.config.picture && !(matches!(config.picture, Picture::Image(_)) && self.base.is_none());
        self.config = config;
        if same { return None; }
        // The old picture goes before the new one is read.
        (self.frame, self.base) = (Vec::new(), None);
        let mut problem = None;
        if let Picture::Image(file) = &self.config.picture {
            let mut base = vec![DARK; self.width * self.height];
            match image(file).ok_or("not found, or larger than wm reads (32 MiB)").and_then(|data| cover(&data, &mut base, self.width, self.height)) {
                Ok(()) => self.base = Some(base),
                Err(why) => problem = Some(format!("{}: {}; the pattern instead", file, why)),
            }
        }
        if self.shown() { self.frame = vec![DARK; self.width * self.height]; }
        problem
    }

    /// The desktop shows the background, not the `░` cells.
    pub fn shown(&self) -> bool { self.config.picture != Picture::None }

    /// A CPU sample (percent), kept for the graph.
    pub fn sample(&mut self, percent: u8) {
        if self.cpu.len() == CPU_SAMPLES { self.cpu.remove(0); }
        self.cpu.push(percent.min(100));
    }

    /// How often the pattern is drawn again (ms): twice a second at speed 1, 20 times from speed 10.
    pub fn interval(&self) -> usize { 500 / self.config.speed.clamp(1, 10) as usize }

    /// The pattern `ms` milliseconds on: `speed` × 16 eighths of a table step a second.
    pub fn advance(&mut self, ms: usize) {
        let total = self.carry + ms * self.config.speed.max(1) as usize * 16;
        self.phase = self.phase.wrapping_add((total / 1000) as u32);
        self.carry = total % 1000;
    }

    /// Draws the frame again: the pattern at its phase (or the picture), the information over it.
    pub fn render(&mut self, info: &Info, glyph: Glyph) {
        if !self.shown() { return; }
        match &self.base { Some(base) => self.frame.copy_from_slice(base), None => pattern(&mut self.frame, self.width, self.height, &self.config, self.phase) }
        let info = Info { cpu: &self.cpu, ..*info };
        overlay(&mut self.frame, self.width, self.height, self.unit, &self.config, &info, glyph);
    }

    /// The colour of the screen's pixel (x, y).
    pub fn pixel(&self, x: usize, y: usize) -> u32 {
        let (fx, fy) = (x / self.unit, y / self.unit);
        if fx < self.width && fy < self.height { self.frame[fy * self.width + fx] } else { DARK }
    }

    /// The pattern moves (a picture does not): `abstract`, or a picture that could not be read.
    pub fn moving(&self) -> bool { self.shown() && self.base.is_none() }
}
