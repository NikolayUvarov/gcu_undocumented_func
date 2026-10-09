//! USB HID reports (issue 164): a boot-protocol keyboard's reports become PS/2 set 1 scan codes for the shared decoder
//! (mind::keys), as virtio_input does for VirtIO keyboards; a mouse's or tablet's reports, laid out by its report
//! descriptor, become pointer events (relative, or absolute as a share of the screen). No system calls:
//! tests/hid_host.rs.
use crate::abi::{pointer_absolute, pointer_event, pointer_scroll, POINTER_LEFT, POINTER_RIGHT, POINTER_SCALE};

/// PS/2 set 1 code of a keyboard-page usage (press; a release adds 0x80): (E0 prefix, code). None for keys without one.
pub fn scancode(usage: u8) -> Option<(bool, u8)> {
    const LETTERS: [u8; 26] = [0x1E, 0x30, 0x2E, 0x20, 0x12, 0x21, 0x22, 0x23, 0x17, 0x24, 0x25, 0x26, 0x32, 0x31, 0x18, 0x19, 0x10, 0x13, 0x1F, 0x14, 0x16, 0x2F, 0x11, 0x2D, 0x15, 0x2C];
    const KEYPAD: [u8; 10] = [0x4F, 0x50, 0x51, 0x4B, 0x4C, 0x4D, 0x47, 0x48, 0x49, 0x52]; // 1-9, 0
    Some(match usage {
        0x04..=0x1D => (false, LETTERS[(usage - 0x04) as usize]),
        0x1E..=0x27 => (false, usage - 0x1E + 0x02), // 1-9, 0
        0x28 => (false, 0x1C), 0x29 => (false, 0x01), 0x2A => (false, 0x0E), 0x2B => (false, 0x0F), 0x2C => (false, 0x39),
        0x2D => (false, 0x0C), 0x2E => (false, 0x0D), 0x2F => (false, 0x1A), 0x30 => (false, 0x1B), 0x31 | 0x32 => (false, 0x2B),
        0x33 => (false, 0x27), 0x34 => (false, 0x28), 0x35 => (false, 0x29), 0x36 => (false, 0x33), 0x37 => (false, 0x34),
        0x38 => (false, 0x35), 0x39 => (false, 0x3A),
        0x3A..=0x43 => (false, usage - 0x3A + 0x3B), // F1-F10
        0x44 => (false, 0x57), 0x45 => (false, 0x58), // F11, F12
        0x46 => (true, 0x37), 0x47 => (false, 0x46), // Print Screen, Scroll Lock
        0x49 => (true, 0x52), 0x4A => (true, 0x47), 0x4B => (true, 0x49), 0x4C => (true, 0x53), 0x4D => (true, 0x4F), 0x4E => (true, 0x51),
        0x4F => (true, 0x4D), 0x50 => (true, 0x4B), 0x51 => (true, 0x50), 0x52 => (true, 0x48), // arrows
        0x53 => (false, 0x45), 0x54 => (true, 0x35), 0x55 => (false, 0x37), 0x56 => (false, 0x4A), 0x57 => (false, 0x4E), 0x58 => (true, 0x1C),
        0x59..=0x62 => (false, KEYPAD[(usage - 0x59) as usize]),
        0x63 => (false, 0x53), 0x64 => (false, 0x56), 0x65 => (true, 0x5D), // KP ., the non-US backslash, Menu
        _ => return None,
    })
}

// The modifier byte of a boot report, bit by bit: left Ctrl, Shift, Alt, GUI, then the right ones.
const MODIFIERS: [(bool, u8); 8] = [(false, 0x1D), (false, 0x2A), (false, 0x38), (true, 0x5B), (true, 0x1D), (false, 0x36), (true, 0x38), (true, 0x5C)];

/// Key repeat, which a USB keyboard leaves to the host: after `REPEAT_DELAY_MS`, every `REPEAT_MS`.
pub const REPEAT_DELAY_MS: u64 = 500;
pub const REPEAT_MS: u64 = 33;

/// A boot-protocol keyboard (8-byte reports: modifiers, reserved, six key usages) between reports.
#[derive(Default)]
pub struct Keyboard { modifiers: u8, keys: [u8; 6], held: Option<u8>, since: u64 }

impl Keyboard {
    pub fn new() -> Self { Self::default() }

    /// One report at `now` (ms); `out` gets the scan code bytes of what changed: releases first, then presses.
    pub fn feed(&mut self, report: &[u8], now: u64, out: &mut impl FnMut(u8)) {
        if report.len() < 8 { return; }
        let keys: [u8; 6] = report[2..8].try_into().unwrap();
        if keys.iter().all(|&k| k == 1) { return; } // rollover error: the state is unknown, keep the last one
        let modifiers = report[0];
        for (bit, &(extended, code)) in MODIFIERS.iter().enumerate() {
            let (was, is) = (self.modifiers >> bit & 1 != 0, modifiers >> bit & 1 != 0);
            if was != is { emit(extended, if is { code } else { code | 0x80 }, out); }
        }
        for &key in self.keys.iter().filter(|&&k| k > 3 && !keys.contains(&k)) {
            if let Some((extended, code)) = scancode(key) { emit(extended, code | 0x80, out); }
            if self.held == Some(key) { self.held = None; }
        }
        for &key in keys.iter().filter(|&&k| k > 3 && !self.keys.contains(&k)) {
            if let Some((extended, code)) = scancode(key) { emit(extended, code, out); self.held = Some(key); self.since = now; }
        }
        (self.modifiers, self.keys) = (modifiers, keys);
    }

    /// The make code of the last pressed key again when it is due to repeat at `now` (ms).
    pub fn repeat(&mut self, now: u64, out: &mut impl FnMut(u8)) {
        let Some(key) = self.held else { return };
        if now < self.since + REPEAT_DELAY_MS { return; }
        self.since = now - REPEAT_DELAY_MS + REPEAT_MS;
        if let Some((extended, code)) = scancode(key) { emit(extended, code, out); }
    }

    /// Releases of everything held (the keyboard went away).
    pub fn release_all(&mut self, out: &mut impl FnMut(u8)) { self.feed(&[0; 8], 0, out); self.held = None; }
}

fn emit(extended: bool, code: u8, out: &mut impl FnMut(u8)) { if extended { out(0xE0); } out(code); }

/// Where one value is in a report: bit offset (after the report ID), size in bits, signed, its logical range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Field { pub offset: u16, pub size: u8, pub signed: bool, pub min: i32, pub max: i32 }

impl Field {
    fn read(self, report: &[u8]) -> Option<i32> {
        if self.size == 0 || self.size > 32 { return None; }
        let mut value: u64 = 0;
        for bit in 0..self.size as usize {
            let at = self.offset as usize + bit;
            if *report.get(at / 8)? >> (at % 8) & 1 != 0 { value |= 1 << bit; }
        }
        Some(if self.signed && value >> (self.size - 1) & 1 != 0 { (value as i64 - (1i64 << self.size)) as i32 } else { value as i32 })
    }
}

/// A pointing device's report layout: up to three buttons, X and Y (relative or absolute), the wheel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pointer { pub id: u8, pub buttons: [Option<Field>; 3], pub x: Field, pub y: Field, pub wheel: Option<Field>, pub absolute: bool }

impl Pointer {
    /// The boot protocol's mouse report: three buttons, then X, Y and the wheel as signed bytes.
    pub fn boot() -> Self {
        let byte = |at: u16| Field { offset: at, size: 8, signed: true, min: -127, max: 127 };
        let button = |bit: u16| Some(Field { offset: bit, size: 1, signed: false, min: 0, max: 1 });
        Self { id: 0, buttons: [button(0), button(1), button(2)], x: byte(8), y: byte(16), wheel: Some(byte(24)), absolute: false }
    }

    /// The layout of the first report with X and Y in a report descriptor; None if it has none.
    pub fn parse(descriptor: &[u8]) -> Option<Self> {
        let (mut page, mut min, mut max, mut size, mut count, mut id) = (0u32, 0i32, 0i32, 0u32, 0u32, 0u8);
        let mut usages = [0u32; 16];
        let (mut used, mut range) = (0usize, None::<(u32, u32)>);
        let mut offsets = [0u32; 256]; // per report ID
        let mut found: [Option<Pointer>; 8] = [None; 8]; // by report ID, in order of appearance
        let mut at = 0;
        while at < descriptor.len() {
            let prefix = descriptor[at];
            if prefix == 0xFE { at += 3 + *descriptor.get(at + 1)? as usize; continue; } // a long item
            let length = [0, 1, 2, 4][(prefix & 3) as usize];
            let bytes = descriptor.get(at + 1..at + 1 + length)?;
            let data = bytes.iter().rev().fold(0u32, |v, &b| v << 8 | b as u32);
            let signed = match length { 1 => data as u8 as i8 as i32, 2 => data as u16 as i16 as i32, _ => data as i32 };
            at += 1 + length;
            match (prefix >> 2) & 3 {
                1 => match prefix >> 4 { // global
                    0 => page = data, 1 => min = signed, 2 => max = if min < 0 { signed } else { data as i32 },
                    7 => size = data, 8 => id = data as u8, 9 => count = data, _ => {}
                },
                2 => match prefix >> 4 { // local
                    0 => { if used < usages.len() { usages[used] = if length == 4 { data } else { page << 16 | data }; used += 1; } }
                    1 => range = Some((page << 16 | data, range.map_or(0, |r| r.1))),
                    2 => range = Some((range.map_or(0, |r| r.0), page << 16 | data)),
                    _ => {}
                },
                0 => {
                    if prefix >> 4 == 8 { // Input
                        let offset = &mut offsets[id as usize];
                        let constant = data & 1 != 0; let variable = data & 2 != 0; let relative = data & 4 != 0;
                        for n in 0..count {
                            let usage = if constant || !variable { 0 } else if used > 0 { usages[(n as usize).min(used - 1)] } else { range.map_or(0, |(low, high)| (low + n).min(high)) };
                            let field = Field { offset: *offset as u16, size: size as u8, signed: min < 0, min, max };
                            if let Some(at) = found.iter().position(|p| p.is_none_or(|p| p.id == id)) {
                                let pointer = found[at].get_or_insert(Pointer { id, ..Default::default() });
                                match usage {
                                    0x0001_0030 => { pointer.x = field; pointer.absolute = !relative; }
                                    0x0001_0031 => pointer.y = field,
                                    0x0001_0038 => pointer.wheel = Some(field),
                                    0x0009_0001..=0x0009_0003 => pointer.buttons[(usage & 0xFF) as usize - 1] = Some(field),
                                    _ => {}
                                }
                            }
                            *offset += size;
                        }
                    }
                    used = 0; range = None; // local items end at every main item
                }
                _ => {}
            }
        }
        found.into_iter().flatten().find(|p| p.x.size > 0 && p.y.size > 0)
    }

    /// Pointer events of one report; nothing for a report with another ID.
    pub fn events(&self, report: &[u8], out: &mut impl FnMut(usize)) {
        let report = if self.id != 0 { match report.split_first() { Some((&id, rest)) if id == self.id => rest, _ => return } } else { report };
        let mut buttons = 0u8;
        for (bit, field) in self.buttons.iter().enumerate() { if field.and_then(|f| f.read(report)).is_some_and(|v| v != 0) { buttons |= 1 << bit; } }
        let (Some(x), Some(y)) = (self.x.read(report), self.y.read(report)) else { return };
        // HID counts the wheel away from the user as positive; pointer events the other way.
        let wheel = -self.wheel.and_then(|f| f.read(report)).unwrap_or(0);
        if self.absolute {
            let scale = |f: Field, v: i32| { let span = (f.max as i64 - f.min as i64).max(1); ((v as i64 - f.min as i64).clamp(0, span) * (POINTER_SCALE as i64 - 1) / span) as usize };
            out(pointer_absolute(buttons, scale(self.x, x), scale(self.y, y), wheel.clamp(-8, 7)));
            return;
        }
        let (mut dx, mut dy, mut wheel) = (x, y, wheel);
        loop {
            let (sx, sy, sw) = (dx.clamp(-256, 255), dy.clamp(-256, 255), wheel.clamp(-8, 7));
            out(pointer_event(buttons, sx, sy, sw));
            (dx, dy, wheel) = (dx - sx, dy - sy, wheel - sw);
            if dx == 0 && dy == 0 && wheel == 0 { break; }
        }
    }
}

/// Apple's trackpads of 2011-2013 (Wellspring 5 to 7A: MacBook Pro 8 to 10, MacBook Air 4 to 5) in their multitouch
/// mode (211-DRV-0018). After their mode switch (`WELLSPRING_MODE`, a feature report to interface 0) they send each
/// touch's fingers on the vendor interface: a 30-byte header with the built-in button at byte 15, then 28 bytes a
/// finger (X at 2, Y at 4, growing upwards, signed; the touch's major axis at 16, zero for a finger lifted). Only the
/// facts of the protocol are taken from Linux's bcm5974 driver.
pub fn wellspring(vendor: u16, product: u16) -> bool {
    vendor == 0x05AC && matches!(product, 0x0245..=0x0247 | 0x0249..=0x024E | 0x0252..=0x0254 | 0x0259..=0x025B | 0x0262..=0x0264)
}
/// The mode switch: read feature report 0 of interface 0 (GET_REPORT, value 0x300, 8 bytes), set byte 0 to 1 (0x08: the
/// mouse mode again) and write it back (SET_REPORT).
pub const WELLSPRING_MODE: (u16, u16, u8) = (0x300, 8, 0x01);
/// The longest packet: the header and 16 fingers.
pub const WELLSPRING_LONGEST: u16 = (TRACKPAD_HEADER + 16 * TRACKPAD_FINGER) as u16;
const TRACKPAD_HEADER: usize = 30;
const TRACKPAD_FINGER: usize = 28;
const TRACKPAD_BUTTON: usize = 15;
// Raw units a screen pixel, a scroll step, and a tap's limits.
const MOTION_DIVISOR: i32 = 8;
const SCROLL_STEP: i32 = 160;
const AXIS_LOCK: i32 = 60;
const TAP_MS: u64 = 250;
const TAP_TRAVEL: i32 = 200;

/// What a trackpad's touches mean (211-DRV-0018): one finger moves the pointer; pressing the pad is the left button,
/// with two fingers on it the right one; a quick tap of two fingers is a right click; three fingers scroll, vertically
/// or horizontally by the way they first move, in the content's direction (macOS's natural scrolling).
#[derive(Default)]
pub struct Trackpad { fingers: usize, last: Option<(i32, i32)>, carry: (i32, i32), scroll: (i32, i32), axis: u8, held: u8, touch: Option<(u64, usize, i32, bool)> }

impl Trackpad {
    pub fn new() -> Self { Self::default() }

    /// One packet at `now` (ms); `out` gets pointer events.
    pub fn feed(&mut self, packet: &[u8], now: u64, out: &mut impl FnMut(usize)) {
        if packet.len() < TRACKPAD_HEADER { return; }
        let (mut count, mut sum) = (0usize, (0i32, 0i32));
        for finger in packet[TRACKPAD_HEADER..].chunks_exact(TRACKPAD_FINGER) {
            if u16::from_le_bytes([finger[16], finger[17]]) == 0 { continue; }
            count += 1;
            sum = (sum.0 + i16::from_le_bytes([finger[2], finger[3]]) as i32, sum.1 + i16::from_le_bytes([finger[4], finger[5]]) as i32);
        }
        let centre = if count > 0 { Some((sum.0 / count as i32, sum.1 / count as i32)) } else { None };
        // Movement since the last packet, only while the same fingers touch: a finger put down or lifted moves the centre.
        let delta = match (centre, self.last) { (Some(c), Some(l)) if count == self.fingers => (c.0 - l.0, c.1 - l.1), _ => (0, 0) };
        if count != self.fingers { self.scroll = (0, 0); self.axis = 0; }
        self.last = centre;
        self.fingers = count;
        // A touch, for taps: when it began, the most fingers, how far they went, whether the pad was pressed.
        let pressed = packet[TRACKPAD_BUTTON] & 1 != 0;
        match (&mut self.touch, count) {
            (None, n) if n > 0 => self.touch = Some((now, n, 0, pressed)),
            (Some(t), n) if n > 0 => { t.1 = t.1.max(n); t.2 += delta.0.abs() + delta.1.abs(); t.3 |= pressed; }
            (Some(t), _) => {
                let (start, most, travel, was_pressed) = *t;
                self.touch = None;
                if most == 2 && !was_pressed && now.saturating_sub(start) <= TAP_MS && travel <= TAP_TRAVEL && self.held == 0 {
                    out(pointer_event(POINTER_RIGHT, 0, 0, 0));
                    out(pointer_event(0, 0, 0, 0));
                }
            }
            _ => {}
        }
        // The button: left, or right with two fingers or more on the pad when it went down; it stays so until it is up.
        let held = if pressed { if self.held != 0 { self.held } else if count >= 2 { POINTER_RIGHT } else { POINTER_LEFT } } else { 0 };
        let changed = held != self.held;
        self.held = held;
        if count >= 3 {
            self.scroll = (self.scroll.0 + delta.0, self.scroll.1 + delta.1);
            if self.axis == 0 && self.scroll.0.abs().max(self.scroll.1.abs()) >= AXIS_LOCK { self.axis = if self.scroll.1.abs() >= self.scroll.0.abs() { 1 } else { 2 }; }
            let (mut wheel, mut across) = (0, 0);
            if self.axis == 1 { wheel = self.scroll.1 / SCROLL_STEP; self.scroll.1 -= wheel * SCROLL_STEP; }
            if self.axis == 2 { across = -(self.scroll.0 / SCROLL_STEP); self.scroll.0 += across * SCROLL_STEP; }
            if wheel != 0 || across != 0 || changed { out(pointer_scroll(held, 0, 0, wheel.clamp(-8, 7), across.clamp(-8, 7))); }
            return;
        }
        // One finger moves the pointer, and any number while the pad is held (a drag); faster strokes go further.
        let (dx, dy) = if count == 1 || held != 0 {
            let speed = delta.0.abs().max(delta.1.abs());
            let gain = if speed > 80 { 3 } else if speed > 30 { 2 } else { 1 };
            let x = delta.0 * gain + self.carry.0; let y = -delta.1 * gain + self.carry.1;
            self.carry = (x % MOTION_DIVISOR, y % MOTION_DIVISOR);
            (x / MOTION_DIVISOR, y / MOTION_DIVISOR)
        } else { (0, 0) };
        if dx == 0 && dy == 0 && !changed { return; }
        let (mut dx, mut dy) = (dx, dy);
        loop {
            let (sx, sy) = (dx.clamp(-256, 255), dy.clamp(-256, 255));
            out(pointer_event(held, sx, sy, 0));
            (dx, dy) = (dx - sx, dy - sy);
            if dx == 0 && dy == 0 { break; }
        }
    }
}
