use crate::abi::*;
use crate::sys::call;

/// Next key code for the active program (PS/2 scancode or UART byte).
pub fn read_key() -> Option<u8> { match call(SYSCALL_READ_KEY, 0, 0) as u8 { 0 => None, key => Some(key) } }

/// Esc on both input paths: scancode 0x01 and byte 0x1B.
pub fn is_escape(key: u8) -> bool { key == 0x01 || key == 0x1B }

/// Drains pending input, exits the process on Esc, then sleeps `ms`. Returns the last key.
pub fn wait_or_exit(ms: usize) -> Option<u8> {
    let mut last = None;
    while let Some(key) = read_key() {
        if is_escape(key) { crate::process::exit(); }
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
