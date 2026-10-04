//! Keyboard input of the focused program: key events (`common/abi.rs`, KEY_*) from the PS/2 driver and the UART,
//! decoded in ring 3 (`mind::keys`).
use crate::abi::*;
use crate::sys::call;

/// Which key: a plain character or a special key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code { Char, Enter, Esc, Backspace, Tab, Up, Down, Left, Right, Home, End, PageUp, PageDown, Insert, Delete, F(u8), Unknown }

/// One key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key(pub u32);

impl Key {
    pub fn code(self) -> Code {
        match (self.0 >> KEY_CODE_SHIFT) & KEY_CODE_MASK {
            0 => Code::Char, KEY_ENTER => Code::Enter, KEY_ESC => Code::Esc, KEY_BACKSPACE => Code::Backspace, KEY_TAB => Code::Tab,
            KEY_UP => Code::Up, KEY_DOWN => Code::Down, KEY_LEFT => Code::Left, KEY_RIGHT => Code::Right,
            KEY_HOME => Code::Home, KEY_END => Code::End, KEY_PGUP => Code::PageUp, KEY_PGDN => Code::PageDown,
            KEY_INSERT => Code::Insert, KEY_DELETE => Code::Delete,
            code @ KEY_F1..=KEY_F12 => Code::F((code - KEY_F1 + 1) as u8),
            _ => Code::Unknown,
        }
    }
    /// The character the key produced (also '\n', '\t', Esc and Backspace for those keys).
    pub fn char(self) -> Option<char> { match self.0 & KEY_CHAR_MASK { 0 => None, c => char::from_u32(c) } }
    /// A printable character typed without Ctrl or Alt.
    pub fn text(self) -> Option<char> { if self.code() == Code::Char && !self.ctrl() && !self.alt() { self.char().filter(|c| !c.is_control()) } else { None } }
    pub fn shift(self) -> bool { self.0 & KEY_MOD_SHIFT != 0 }
    pub fn ctrl(self) -> bool { self.0 & KEY_MOD_CTRL != 0 }
    pub fn alt(self) -> bool { self.0 & KEY_MOD_ALT != 0 }
    pub fn is_escape(self) -> bool { self.code() == Code::Esc }
    /// Ctrl + the given lower-case letter.
    pub fn is_ctrl(self, letter: char) -> bool { self.ctrl() && self.code() == Code::Char && self.char() == Some(letter) }
}

/// Next key event of the calling (focused) task.
pub fn read_key() -> Option<Key> { match call(SYSCALL_READ_KEY, 0, 0) as u32 { 0 => None, word => Some(Key(word)) } }

/// Esc on either input path.
pub fn is_escape(key: Key) -> bool { key.is_escape() }

/// Waits up to `ms` for a key (input wakes the program early).
pub fn wait_key(ms: usize) -> Option<Key> {
    if let Some(key) = read_key() { return Some(key); }
    crate::time::sleep(ms);
    read_key()
}

/// Drains pending input, exits the process on Esc, then sleeps `ms`. Returns the last key.
pub fn wait_or_exit(ms: usize) -> Option<Key> {
    let mut last = None;
    while let Some(key) = read_key() {
        if key.is_escape() { crate::process::exit(); }
        last = Some(key);
    }
    crate::time::sleep(ms);
    last
}
