//! Keyboard input of the focused program: event words (`common/abi.rs`: legacy byte, key, modifiers, pressed,
//! character) from the PS/2 driver and the UART, decoded in ring 3 (`mind::keys`). `read_key` gives key presses,
//! `read_event` every event.
use crate::abi::*;
use crate::sys::call;

pub use crate::keys::{Code, Key};

/// Next key press of the calling (focused) task; releases and events without a decoded key are skipped.
pub fn read_key() -> Option<Key> {
    loop {
        match call(SYSCALL_READ_INPUT, 0, 0) { 0 => return None, word => if let Some(key) = Key::from_event(word) { return Some(key); } }
    }
}

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

/// One key press or release (see `common/abi.rs`): the legacy byte, the decoded key (0 if not decoded), modifiers and
/// the character of the active layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent { pub byte: u8, pub key: u16, pub mods: u8, pub pressed: bool, pub ch: Option<char> }

impl KeyEvent {
    pub fn from_word(word: usize) -> Self {
        Self { byte: event_byte(word), key: event_key(word), mods: event_mods(word), pressed: event_pressed(word), ch: char::from_u32(event_char(word)).filter(|&c| c != '\0') }
    }
    pub fn to_word(self) -> usize { input_event(self.byte, self.key, self.mods, self.pressed, self.ch.map_or(0, |c| c as u32)) }
}

/// Next input event of the active program, if any.
pub fn read_event() -> Option<KeyEvent> { match call(SYSCALL_READ_INPUT, 0, 0) { 0 => None, word => Some(KeyEvent::from_word(word)) } }

/// Waits up to `ms` for an input event (the sleep ends early when one arrives).
pub fn wait_event(ms: usize) -> Option<KeyEvent> {
    if let Some(event) = read_event() { return Some(event); }
    crate::time::sleep(ms);
    read_event()
}
