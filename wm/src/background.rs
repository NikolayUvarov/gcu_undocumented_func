//! The desktop background of `wm` (000-APP-0047), as `data/wm.conf` chooses it: the dim `░` cells as before (`none`),
//! a slowly moving low-contrast pattern (`abstract`, the default) or an image (a BMP), with the time, the date and the
//! CPU load drawn over it. It is drawn into a frame of pixels that `wm` copies to the desktop's cells nothing covers.
//! Host-tested in tests/wm_host.rs.
use alloc::{format, string::String, vec, vec::Vec};

/// Where `wm` keeps the configuration, among the user's files.
pub const FILE: &str = "data/wm.conf";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picture { None, Abstract, Image(String) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place { TopLeft, TopRight, Center, BottomLeft, BottomRight }

const PLACES: [(&str, Place); 5] = [("top-left", Place::TopLeft), ("top-right", Place::TopRight), ("center", Place::Center), ("bottom-left", Place::BottomLeft), ("bottom-right", Place::BottomRight)];

/// What the background shows. `None` shows nothing over the `░` cells, whatever `show` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config { pub picture: Picture, pub time: bool, pub date: bool, pub cpu: bool, pub net: bool, pub place: Place }

impl Default for Config {
    fn default() -> Self { Self { picture: Picture::Abstract, time: true, date: true, cpu: true, net: false, place: Place::BottomRight } }
}

impl Config {
    /// `key = value` lines, `#` starting a comment: `background = none | abstract | image <file>`,
    /// `show = time, date, cpu, net` (or `none`), `place = top-left | top-right | center | bottom-left | bottom-right`.
    /// What a line does not say stays the default; each line not understood is named in the second value.
    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let mut config = Self::default();
        let mut problems = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() { continue; }
            let number = index + 1;
            let Some((key, value)) = line.split_once('=') else { problems.push(format!("line {}: no '='", number)); continue };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "background" => {
                    let (kind, rest) = value.split_once(char::is_whitespace).map_or((value, ""), |(k, r)| (k, r.trim()));
                    match (kind, rest) {
                        ("none", "") => config.picture = Picture::None,
                        ("abstract", "") => config.picture = Picture::Abstract,
                        ("image", file) if !file.is_empty() => config.picture = Picture::Image(String::from(file)),
                        _ => problems.push(format!("line {}: background is none, abstract or image <file>", number)),
                    }
                }
                "show" => {
                    let mut shown = [false; 4];
                    let words: Vec<&str> = value.split(',').map(str::trim).filter(|w| !w.is_empty()).collect();
                    let unknown = words.iter().find(|w| !["time", "date", "cpu", "net", "none"].contains(w));
                    match unknown {
                        Some(word) => problems.push(format!("line {}: show takes time, date, cpu, net or none, not {}", number, word)),
                        None => {
                            for (i, name) in ["time", "date", "cpu", "net"].iter().enumerate() { shown[i] = words.contains(name); }
                            [config.time, config.date, config.cpu, config.net] = shown;
                        }
                    }
                }
                "place" => match PLACES.iter().find(|p| p.0 == value) {
                    Some(&(_, place)) => config.place = place,
                    None => problems.push(format!("line {}: place is top-left, top-right, center, bottom-left or bottom-right", number)),
                },
                _ => problems.push(format!("line {}: unknown key {}", number, key)),
            }
        }
        (config, problems)
    }

    /// The configuration as `parse` reads it back.
    pub fn format(&self) -> String {
        let picture = match &self.picture { Picture::None => String::from("none"), Picture::Abstract => String::from("abstract"), Picture::Image(file) => format!("image {}", file) };
        let shown: Vec<&str> = [(self.time, "time"), (self.date, "date"), (self.cpu, "cpu"), (self.net, "net")].iter().filter(|s| s.0).map(|s| s.1).collect();
        let place = PLACES.iter().find(|p| p.1 == self.place).map_or("bottom-right", |p| p.0);
        format!("# wm's desktop background (000-APP-0047)\n# background = none | abstract | image <file>; show = time, date, cpu, net | none\nbackground = {}\nshow = {}\nplace = {}\n",
                picture, if shown.is_empty() { String::from("none") } else { shown.join(", ") }, place)
    }
}

/// The pattern's darkest and lightest colours: the desktop's own blue, little apart.
pub const DARK: u32 = 0x10_1A_24;
pub const LIGHT: u32 = 0x24_38_4C;
/// The information's colours: the text and the graph, low-contrast but readable, and a shadow under the text.
pub const TEXT: u32 = 0x78_90_A8;
pub const GRAPH: u32 = 0x3A_56_70;
pub const SHADOW: u32 = 0x08_0E_14;
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

fn mix(a: u32, b: u32, k: u32) -> u32 {
    let channel = |shift: u32| { let (x, y) = ((a >> shift) & 0xFF, (b >> shift) & 0xFF); ((x * (255 - k) + y * k) / 255) << shift };
    channel(16) | channel(8) | channel(0)
}

/// Frame `step` of the abstract pattern into `frame` (`width` × `height` pixels, row after row): four sines crossing at
/// slants, each step moving them by a quarter of a table step, between DARK and LIGHT; computed in 2 × 2 blocks.
pub fn pattern(frame: &mut [u32], width: usize, height: usize, step: u32) {
    let palette: Vec<u32> = (0..256).map(|k| mix(DARK, LIGHT, k)).collect();
    let t = step as usize;
    let at = |i: usize| SINE[i % PERIOD] as i32;
    for y in (0..height).step_by(2) {
        let v = y * PERIOD / height.max(1);
        let a = at(v * 2 + PERIOD * 4 - t / 2);
        for x in (0..width).step_by(2) {
            let u = x * PERIOD / width.max(1);
            let sum = a + at(u + t) + at(u + v + t * 2 / 3) + at(u * 3 / 2 + PERIOD * 4 - v - t / 3);
            let colour = palette[((sum + 508) * 255 / 1016).clamp(0, 255) as usize];
            for row in y..(y + 2).min(height) {
                let line = &mut frame[row * width..row * width + width];
                for pixel in line[x..(x + 2).min(width)].iter_mut() { *pixel = colour; }
            }
        }
    }
}

/// An uncompressed BMP (24 or 32 bits a pixel, rows from the bottom or the top) scaled to cover `width` × `height`
/// (cut at its longer sides, its middle kept) into `frame`. None if the file is not such a BMP.
pub fn bmp_cover(data: &[u8], frame: &mut [u32], width: usize, height: usize) -> Option<()> {
    if data.len() < 54 || &data[..2] != b"BM" || width == 0 || height == 0 { return None; }
    let u32_at = |at: usize| data.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let u16_at = |at: usize| data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let (offset, w, h) = (u32_at(10)? as usize, u32_at(18)? as i32, u32_at(22)? as i32);
    let (bits, compression) = (u16_at(28)?, u32_at(30)?);
    if !(bits == 24 || bits == 32) || !(compression == 0 || (compression == 3 && bits == 32)) { return None; }
    if w <= 0 || h == 0 || w > 16_384 || h.unsigned_abs() > 16_384 { return None; }
    let (iw, ih) = (w as usize, h.unsigned_abs() as usize);
    let bytes = bits as usize / 8;
    let stride = (iw * bytes + 3) & !3;
    if offset.checked_add(stride.checked_mul(ih)?)? > data.len() { return None; }
    // Frame pixels per image pixel: num / den, the larger of the two ratios, so the image covers the frame.
    let (num, den) = if width * ih >= height * iw { (width, iw) } else { (height, ih) };
    let (sw, sh) = (iw * num / den, ih * num / den);
    let (ox, oy) = (sw.saturating_sub(width) / 2, sh.saturating_sub(height) / 2);
    for y in 0..height {
        let iy = ((y + oy) * den / num).min(ih - 1);
        let row = offset + if h > 0 { ih - 1 - iy } else { iy } * stride;
        for x in 0..width {
            let p = row + ((x + ox) * den / num).min(iw - 1) * bytes;
            frame[y * width + x] = (data[p + 2] as u32) << 16 | (data[p + 1] as u32) << 8 | data[p] as u32;
        }
    }
    Some(())
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
fn text(frame: &mut [u32], width: usize, height: usize, x: usize, y: usize, text: &str, scale: usize, colour: u32, glyph: Glyph) {
    for (shift, colour) in [(scale.div_ceil(2), SHADOW), (0, colour)] {
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
    // Each line right-aligned in the block on the right, left-aligned elsewhere.
    let right = matches!(config.place, Place::TopRight | Place::BottomRight);
    let start = |line_w: usize| if right { x + w - line_w } else { x };
    if config.time {
        let line = match info.seconds { Some(s) => format!("{:02}:{:02}", s / 3600 % 24, s / 60 % 60), None => String::from("--:--") };
        text(frame, width, height, start(5 * 8 * big), y, &line, big, TEXT, glyph);
        y += 16 * big + gap;
    }
    if config.date {
        let line = match info.date { Some((year, month, day)) => format!("{:04}-{:02}-{:02}", year, month, day), None => String::from("----------") };
        text(frame, width, height, start(10 * 8 * small), y, &line, small, TEXT, glyph);
        y += 16 * small + gap;
    }
    if config.cpu {
        let graph_w = 5 * 8 * big;
        let label = match info.cpu.last() { Some(p) => format!("CPU {:3}%", p), None => String::from("CPU    -") };
        text(frame, width, height, start(graph_w), y, &label, small, TEXT, glyph);
        y += 16 * small + gap / 2;
        let (gx, gh) = (start(graph_w), 48 / unit.max(1));
        // A bar a sample, the newest at the right; a line along the bottom.
        let samples = &info.cpu[info.cpu.len().saturating_sub(CPU_SAMPLES)..];
        let bar = (graph_w / CPU_SAMPLES).max(1);
        for (i, &p) in samples.iter().enumerate() {
            let bx = gx + graph_w - (samples.len() - i) * bar;
            let bh = (p.min(100) as usize * gh).div_ceil(100);
            for py in y + gh - bh..y + gh { for px in bx..bx + bar { if px < width && py < height { frame[py * width + px] = GRAPH; } } }
        }
        for px in gx..gx + graph_w { if px < width && y + gh < height { frame[(y + gh) * width + px] = TEXT; } }
        y += gh + gap;
    }
    if config.net {
        let line = info.net.unwrap_or("NET: NO COUNTERS");
        text(frame, width, height, start(24 * 8 * small), y, line, small, TEXT, glyph);
    }
}

/// The background's state in `wm`: its frame (the screen's pixels, or half of them each way when that would not fit
/// in 4 MiB), the image scaled once, the pattern's step and the CPU samples.
pub struct Backdrop { pub config: Config, pub width: usize, pub height: usize, pub unit: usize, pub frame: Vec<u32>, base: Option<Vec<u32>>, pub step: u32, pub cpu: Vec<u8> }

impl Backdrop {
    /// For a screen of `screen` pixels; `image` reads the file `image <file>` names (None: the pattern is drawn instead,
    /// and the configuration keeps the image).
    pub fn new(config: Config, screen: (usize, usize), image: &mut dyn FnMut(&str) -> Option<Vec<u8>>) -> (Self, Option<String>) {
        let unit = if screen.0 * screen.1 * 4 <= 4 << 20 { 1 } else { 2 };
        let (width, height) = (screen.0 / unit, screen.1 / unit);
        let mut problem = None;
        let base = match &config.picture {
            Picture::Image(file) => {
                let mut base = vec![0u32; width * height];
                match image(file).and_then(|data| bmp_cover(&data, &mut base, width, height)) {
                    Some(()) => Some(base),
                    None => { problem = Some(format!("{}: not a BMP that can be read (24 or 32 bits, uncompressed); the pattern instead", file)); None }
                }
            }
            _ => None,
        };
        let frame = if config.picture == Picture::None { Vec::new() } else { vec![DARK; width * height] };
        (Self { config, width, height, unit, frame, base, step: 0, cpu: Vec::new() }, problem)
    }

    /// The desktop shows the background, not the `░` cells.
    pub fn shown(&self) -> bool { self.config.picture != Picture::None }

    /// A CPU sample (percent), kept for the graph.
    pub fn sample(&mut self, percent: u8) {
        if self.cpu.len() == CPU_SAMPLES { self.cpu.remove(0); }
        self.cpu.push(percent.min(100));
    }

    /// Draws the frame again: the pattern at its step (or the image), the information over it.
    pub fn render(&mut self, info: &Info, glyph: Glyph) {
        if !self.shown() { return; }
        match &self.base { Some(base) => self.frame.copy_from_slice(base), None => pattern(&mut self.frame, self.width, self.height, self.step) }
        let info = Info { cpu: &self.cpu, ..*info };
        overlay(&mut self.frame, self.width, self.height, self.unit, &self.config, &info, glyph);
    }

    /// The colour of the screen's pixel (x, y).
    pub fn pixel(&self, x: usize, y: usize) -> u32 {
        let (fx, fy) = (x / self.unit, y / self.unit);
        if fx < self.width && fy < self.height { self.frame[fy * self.width + fx] } else { DARK }
    }

    /// The pattern moves (an image does not): `abstract`, or an image that could not be read.
    pub fn moving(&self) -> bool { self.shown() && self.base.is_none() }
}
