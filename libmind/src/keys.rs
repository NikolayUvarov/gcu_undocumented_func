//! Key decoders shared by the keyboard driver and the shell (they run in ring 3; the kernel only queues events).
//! `Ps2` turns PS/2 set 1 scan codes (as the i8042 translates them) into key events with modifiers and the US or
//! Russian layout; `Vt` turns UART bytes from a VT100/xterm terminal (UTF-8 text, CSI/SS3 sequences) into the same
//! events. The event word is described in `common/abi.rs` (`input_event`: legacy byte, KEY_*, MOD_*, pressed,
//! character); the decoders produce presses only, with the scan code or UART byte as the legacy byte. This file builds
//! on the host for tests.
use crate::abi::*;

/// Which key: a plain character or a special key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code { Char, Enter, Esc, Backspace, Tab, Up, Down, Left, Right, Home, End, PageUp, PageDown, Insert, Delete, F(u8), Unknown }

/// One key press: an input event word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key(pub usize);

/// F12 (the function keys are KEY_F1..=KEY_F12).
pub const KEY_F12: u16 = KEY_F1 + 11;

impl Key {
    /// The key press an event word describes; None for a release or an event that carries only a legacy byte.
    pub fn from_event(word: usize) -> Option<Self> { (event_pressed(word) && event_key(word) != 0 && event_key(word) != KEY_POINTER).then_some(Self(word)) }
    pub fn code(self) -> Code {
        match event_key(self.0) {
            KEY_CHAR => Code::Char, KEY_ENTER => Code::Enter, KEY_ESC => Code::Esc, KEY_BACKSPACE => Code::Backspace, KEY_TAB => Code::Tab,
            KEY_UP => Code::Up, KEY_DOWN => Code::Down, KEY_LEFT => Code::Left, KEY_RIGHT => Code::Right,
            KEY_HOME => Code::Home, KEY_END => Code::End, KEY_PAGE_UP => Code::PageUp, KEY_PAGE_DOWN => Code::PageDown,
            KEY_INSERT => Code::Insert, KEY_DELETE => Code::Delete,
            code @ KEY_F1..=KEY_F12 => Code::F((code - KEY_F1 + 1) as u8),
            _ => Code::Unknown,
        }
    }
    /// The character the key produced (also '\n', '\t', Esc and Backspace for those keys).
    pub fn char(self) -> Option<char> { match event_char(self.0) { 0 => None, c => char::from_u32(c) } }
    /// A printable character typed without Ctrl or Alt.
    pub fn text(self) -> Option<char> { if self.code() == Code::Char && !self.ctrl() && !self.alt() { self.char().filter(|c| !c.is_control()) } else { None } }
    pub fn shift(self) -> bool { event_mods(self.0) & MOD_SHIFT != 0 }
    pub fn ctrl(self) -> bool { event_mods(self.0) & MOD_CTRL != 0 }
    pub fn alt(self) -> bool { event_mods(self.0) & MOD_ALT != 0 }
    /// The legacy byte: the scan code or UART byte that completed the key.
    pub fn byte(self) -> u8 { event_byte(self.0) }
    pub fn is_escape(self) -> bool { self.code() == Code::Esc }
    /// Ctrl + the given lower-case letter.
    pub fn is_ctrl(self, letter: char) -> bool { self.ctrl() && self.code() == Code::Char && self.char() == Some(letter) }
    /// The typed character as the US layout puts it on the same key (`з` → `p`, `З` → `P`), so letter commands work in
    /// either layout; other characters are returned as they are.
    pub fn latin(self) -> Option<char> {
        let ch = self.text()?;
        if ch.is_ascii() { return Some(ch); }
        if let Some(i) = RU.iter().position(|&c| c == ch) { return Some(US[i] as char); }
        if let Some(i) = RU_SHIFT.iter().position(|&c| c == ch) { return Some(US_SHIFT[i] as char); }
        Some(ch)
    }
}

/// What a decoder produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// A key press (an event word).
    Key(usize),
    /// The attention key (Ctrl+Z): focus goes back to the shell.
    Attention,
    /// The keyboard layout changed.
    Layout(Layout),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout { Us, Ru }

/// What switches the layout (idl/keyboard.wit): Ctrl+Shift or Alt+Shift, only one of them, Caps Lock, or nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch { CtrlOrAltShift, CtrlShift, AltShift, CapsLock, None }

/// The event word of a key press from its parts; `code` 0 stands for a plain character (KEY_CHAR).
pub const fn event(code: u16, ch: u32, mods: u8) -> usize { input_event(0, if code == 0 { KEY_CHAR } else { code }, mods, true, ch) }
/// `word` with the legacy byte `byte`.
pub const fn with_byte(word: usize, byte: u8) -> usize { word & !0xFF | byte as usize }

// Scan codes 0x00..0x3A of the main block; 0 where the key has no character.
const US: &[u8; 58] = b"\0\x1b1234567890-=\x08\tqwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";
const US_SHIFT: &[u8; 58] = b"\0\x1b!@#$%^&*()_+\x08\tQWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";
// Russian ЙЦУКЕН (as in Windows "Russian"): the same positions, other characters.
const RU: [char; 58] = ['\0', '\x1b', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\x08', '\t',
    'й', 'ц', 'у', 'к', 'е', 'н', 'г', 'ш', 'щ', 'з', 'х', 'ъ', '\n', '\0', 'ф', 'ы', 'в', 'а', 'п', 'р', 'о', 'л', 'д', 'ж', 'э', 'ё', '\0', '\\',
    'я', 'ч', 'с', 'м', 'и', 'т', 'ь', 'б', 'ю', '.', '\0', '*', '\0', ' '];
const RU_SHIFT: [char; 58] = ['\0', '\x1b', '!', '"', '№', ';', '%', ':', '?', '*', '(', ')', '_', '+', '\x08', '\t',
    'Й', 'Ц', 'У', 'К', 'Е', 'Н', 'Г', 'Ш', 'Щ', 'З', 'Х', 'Ъ', '\n', '\0', 'Ф', 'Ы', 'В', 'А', 'П', 'Р', 'О', 'Л', 'Д', 'Ж', 'Э', 'Ё', '\0', '/',
    'Я', 'Ч', 'С', 'М', 'И', 'Т', 'Ь', 'Б', 'Ю', ',', '\0', '*', '\0', ' '];
// Keypad 0x47..0x53 with Num Lock off (navigation) and on (characters).
const PAD_KEYS: [u16; 13] = [KEY_HOME, KEY_UP, KEY_PAGE_UP, 0, KEY_LEFT, 0, KEY_RIGHT, 0, KEY_END, KEY_DOWN, KEY_PAGE_DOWN, KEY_INSERT, KEY_DELETE];
const PAD_CHARS: &[u8; 13] = b"789-456+1230.";

/// PS/2 scan code set 1 decoder with modifier state, Caps/Num Lock and the layout switch.
pub struct Ps2 {
    extended: bool, skip: u8, shift: [bool; 2], ctrl: [bool; 2], alt: [bool; 2], caps: bool, num: bool,
    layout: Layout, chord: bool, switch: Switch,
}

impl Default for Ps2 { fn default() -> Self { Self::new() } }

impl Ps2 {
    pub const fn new() -> Self { Self { extended: false, skip: 0, shift: [false; 2], ctrl: [false; 2], alt: [false; 2], caps: false, num: false, layout: Layout::Us, chord: false, switch: Switch::CtrlOrAltShift } }
    pub fn layout(&self) -> Layout { self.layout }
    pub fn set_layout(&mut self, layout: Layout) { self.layout = layout; }
    pub fn switch(&self) -> Switch { self.switch }
    /// With Caps Lock as the switch it no longer locks capitals (a lock already on is released).
    pub fn set_switch(&mut self, switch: Switch) { self.switch = switch; self.chord = false; if switch == Switch::CapsLock { self.caps = false; } }
    fn toggle(&mut self) -> Option<Event> {
        self.layout = if self.layout == Layout::Us { Layout::Ru } else { Layout::Us };
        Some(Event::Layout(self.layout))
    }
    fn shift(&self) -> bool { self.shift[0] || self.shift[1] }
    fn ctrl(&self) -> bool { self.ctrl[0] || self.ctrl[1] }
    fn alt(&self) -> bool { self.alt[0] || self.alt[1] }
    pub fn mods(&self) -> u8 { (self.shift() as u8 * MOD_SHIFT) | (self.ctrl() as u8 * MOD_CTRL) | (self.alt() as u8 * MOD_ALT) | (self.caps as u8 * MOD_CAPS) }

    /// One byte from the controller; a key press carries the scan code as its legacy byte.
    pub fn feed(&mut self, byte: u8) -> Option<Event> {
        match self.decode(byte) { Some(Event::Key(word)) => Some(Event::Key(with_byte(word, byte))), other => other }
    }

    fn decode(&mut self, byte: u8) -> Option<Event> {
        if self.skip > 0 { self.skip -= 1; return None; }
        if byte == 0xE1 { self.skip = 5; return None; } // Pause: E1 1D 45 E1 9D C5
        if byte == 0xE0 { self.extended = true; return None; }
        let extended = core::mem::take(&mut self.extended);
        let (code, released) = (byte & 0x7F, byte & 0x80 != 0);
        // Modifiers. E0 2A / E0 36 are fake shifts sent around Print Screen and the navigation block.
        let side = extended as usize;
        let modifier = match code {
            0x2A | 0x36 if extended => return None,
            0x2A => Some(&mut self.shift[0]), 0x36 => Some(&mut self.shift[1]),
            0x1D => Some(&mut self.ctrl[side]), 0x38 => Some(&mut self.alt[side]),
            _ => None,
        };
        if let Some(state) = modifier {
            *state = !released;
            if !released {
                // Ctrl+Shift or Alt+Shift (as `switch` says): the layout switches when one of them is released with no
                // other key between.
                let chord = match self.switch {
                    Switch::CtrlOrAltShift => self.shift() && (self.ctrl() || self.alt()),
                    Switch::CtrlShift => self.shift() && self.ctrl() && !self.alt(),
                    Switch::AltShift => self.shift() && self.alt() && !self.ctrl(),
                    Switch::CapsLock | Switch::None => false,
                };
                if chord { self.chord = true; }
                return None;
            }
            if core::mem::take(&mut self.chord) { return self.toggle(); }
            return None;
        }
        if released { return None; }
        self.chord = false;
        let mods = self.mods();
        let key = |code: u16| Some(Event::Key(event(code, 0, mods)));
        if extended {
            return match code {
                0x48 => key(KEY_UP), 0x50 => key(KEY_DOWN), 0x4B => key(KEY_LEFT), 0x4D => key(KEY_RIGHT),
                0x47 => key(KEY_HOME), 0x4F => key(KEY_END), 0x49 => key(KEY_PAGE_UP), 0x51 => key(KEY_PAGE_DOWN),
                0x52 => key(KEY_INSERT), 0x53 => key(KEY_DELETE),
                0x1C => Some(Event::Key(event(KEY_ENTER, '\n' as u32, mods))),
                0x35 => Some(Event::Key(event(0, '/' as u32, mods))),
                _ => None,
            };
        }
        match code {
            0x3A if self.switch == Switch::CapsLock => return self.toggle(),
            0x3A => { self.caps = !self.caps; return None; }
            0x45 => { self.num = !self.num; return None; }
            0x46 => return None,
            0x3B..=0x44 => return key(KEY_F1 + (code - 0x3B) as u16),
            0x57 => return key(KEY_F1 + 10), 0x58 => return key(KEY_F12),
            0x47..=0x53 => {
                let index = (code - 0x47) as usize;
                if self.num || PAD_KEYS[index] == 0 { return Some(Event::Key(event(0, PAD_CHARS[index] as u32, mods))); }
                return key(PAD_KEYS[index]);
            }
            0x2C if self.ctrl() => return Some(Event::Attention), // Ctrl+Z by position, in any layout
            _ => {}
        }
        let index = code as usize;
        if index >= US.len() { return None; }
        let special = match US[index] { b'\x1b' => KEY_ESC, b'\x08' => KEY_BACKSPACE, b'\t' => KEY_TAB, b'\n' => KEY_ENTER, _ => 0 };
        if special != 0 { return Some(Event::Key(event(special, US[index] as u32, mods))); }
        if US[index] == 0 { return None; }
        // Shortcuts are by position: Ctrl/Alt + a key gives the US character, so Ctrl+C is the same in both layouts.
        if self.ctrl() || self.alt() { return Some(Event::Key(event(0, US[index].to_ascii_lowercase() as u32, mods))); }
        let letter = match self.layout { Layout::Us => US[index].is_ascii_alphabetic(), Layout::Ru => RU[index].is_alphabetic() };
        let upper = self.shift() != (self.caps && letter);
        let ch = match (self.layout, upper) {
            (Layout::Us, false) => US[index] as char, (Layout::Us, true) => US_SHIFT[index] as char,
            (Layout::Ru, false) => RU[index], (Layout::Ru, true) => RU_SHIFT[index],
        };
        Some(Event::Key(event(0, ch as u32, mods)))
    }
}

/// Terminal (VT100/xterm) input decoder for the UART.
pub struct Vt { state: VtState, since: u64, params: [u16; 4], count: usize, linux: bool, utf8: u32, need: u8, cr: bool }

#[derive(Clone, Copy, PartialEq, Eq)]
enum VtState { Ground, Escape, Csi, Ss3, Utf8 }

/// A lone Esc is reported once no sequence byte follows within this time.
pub const ESC_TIMEOUT_MS: u64 = 50;

impl Default for Vt { fn default() -> Self { Self::new() } }

impl Vt {
    pub const fn new() -> Self { Self { state: VtState::Ground, since: 0, params: [0; 4], count: 0, linux: false, utf8: 0, need: 0, cr: false } }

    /// Whether a lone Esc or an unfinished sequence is waiting for `poll`.
    pub fn pending(&self) -> bool { self.state != VtState::Ground }

    /// Reports a waiting Esc once the timeout has passed; an unfinished sequence is dropped.
    pub fn poll(&mut self, now_ms: u64, emit: &mut impl FnMut(Event)) {
        if self.state == VtState::Ground || now_ms.saturating_sub(self.since) < ESC_TIMEOUT_MS { return; }
        if self.state == VtState::Escape { emit(Event::Key(event(KEY_ESC, 0x1B, 0))); }
        self.state = VtState::Ground;
    }

    /// One byte from the UART at time `now_ms`; a key press carries the byte that completed it as its legacy byte.
    pub fn feed(&mut self, byte: u8, now_ms: u64, emit: &mut impl FnMut(Event)) {
        self.step(byte, &mut |event| emit(match event { Event::Key(word) => Event::Key(with_byte(word, byte)), other => other }));
        // An unfinished sequence (Esc, CSI, SS3, UTF-8) times out ESC_TIMEOUT_MS after its last byte.
        if self.state != VtState::Ground { self.since = now_ms; }
    }

    fn step(&mut self, byte: u8, emit: &mut impl FnMut(Event)) {
        match self.state {
            VtState::Ground => self.ground(byte, emit),
            VtState::Utf8 => {
                if byte & 0xC0 != 0x80 { self.state = VtState::Ground; return self.ground(byte, emit); }
                self.utf8 = self.utf8 << 6 | (byte & 0x3F) as u32; self.need -= 1;
                if self.need == 0 {
                    self.state = VtState::Ground;
                    if let Some(ch) = char::from_u32(self.utf8) { emit(Event::Key(event(0, ch as u32, 0))); }
                }
            }
            VtState::Escape => match byte {
                b'[' => { self.state = VtState::Csi; self.params = [0; 4]; self.count = 0; self.linux = false; }
                b'O' => self.state = VtState::Ss3,
                _ => { emit(Event::Key(event(KEY_ESC, 0x1B, 0))); self.state = VtState::Ground; self.ground(byte, emit); }
            },
            VtState::Ss3 => {
                self.state = VtState::Ground;
                let code = match byte { b'A' => KEY_UP, b'B' => KEY_DOWN, b'C' => KEY_RIGHT, b'D' => KEY_LEFT, b'H' => KEY_HOME, b'F' => KEY_END, b'P'..=b'S' => KEY_F1 + (byte - b'P') as u16, b'M' => KEY_ENTER, _ => 0 };
                if code == KEY_ENTER { emit(Event::Key(event(KEY_ENTER, '\n' as u32, 0))); } else if code != 0 { emit(Event::Key(event(code, 0, 0))); }
            }
            VtState::Csi => match byte {
                b'0'..=b'9' => { let p = &mut self.params[self.count.min(3)]; *p = p.saturating_mul(10).saturating_add((byte - b'0') as u16); }
                b';' => self.count += 1,
                b'[' if self.count == 0 && self.params[0] == 0 => self.linux = true, // Linux console: CSI [ A..E = F1..F5
                0x40..=0x7E => {
                    self.state = VtState::Ground;
                    let mods = match self.params[1] { 0 | 1 => 0, m => { let bits = (m - 1) as u8; (bits & 1) * MOD_SHIFT | (bits >> 1 & 1) * MOD_ALT | (bits >> 2 & 1) * MOD_CTRL } };
                    let code = if self.linux { match byte { b'A'..=b'E' => KEY_F1 + (byte - b'A') as u16, _ => 0 } } else { match byte {
                        b'A' => KEY_UP, b'B' => KEY_DOWN, b'C' => KEY_RIGHT, b'D' => KEY_LEFT, b'H' => KEY_HOME, b'F' => KEY_END,
                        b'P'..=b'S' => KEY_F1 + (byte - b'P') as u16,
                        b'Z' => { emit(Event::Key(event(KEY_TAB, '\t' as u32, MOD_SHIFT))); 0 }
                        b'~' => match self.params[0] {
                            1 | 7 => KEY_HOME, 2 => KEY_INSERT, 3 => KEY_DELETE, 4 | 8 => KEY_END, 5 => KEY_PAGE_UP, 6 => KEY_PAGE_DOWN,
                            11..=15 => KEY_F1 + (self.params[0] - 11), 17..=21 => KEY_F1 + 5 + (self.params[0] - 17),
                            23 => KEY_F1 + 10, 24 => KEY_F12, _ => 0,
                        },
                        _ => 0,
                    } };
                    if code != 0 { emit(Event::Key(event(code, 0, mods))); }
                }
                _ => self.state = VtState::Ground, // not a sequence we know: drop it
            },
        }
    }

    fn ground(&mut self, byte: u8, emit: &mut impl FnMut(Event)) {
        let after_cr = core::mem::take(&mut self.cr);
        let key = |code: u16, ch: u32| Event::Key(event(code, ch, 0));
        match byte {
            0x1B => self.state = VtState::Escape,
            b'\r' => { self.cr = true; emit(key(KEY_ENTER, '\n' as u32)); }
            b'\n' => if !after_cr { emit(key(KEY_ENTER, '\n' as u32)) },
            0x08 | 0x7F => emit(key(KEY_BACKSPACE, 0x08)),
            b'\t' => emit(key(KEY_TAB, '\t' as u32)),
            0x1A => emit(Event::Attention),
            0x01..=0x19 => emit(Event::Key(event(0, (b'a' + byte - 1) as u32, MOD_CTRL))),
            0x20..=0x7E => emit(key(0, byte as u32)),
            0xC2..=0xDF => { self.state = VtState::Utf8; self.utf8 = (byte & 0x1F) as u32; self.need = 1; }
            0xE0..=0xEF => { self.state = VtState::Utf8; self.utf8 = (byte & 0x0F) as u32; self.need = 2; }
            0xF0..=0xF4 => { self.state = VtState::Utf8; self.utf8 = (byte & 0x07) as u32; self.need = 3; }
            _ => {}
        }
    }
}
