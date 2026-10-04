//! Keyboard input of the focused program: key events (`common/abi.rs`, KEY_*) from the PS/2 driver and the UART,
//! decoded in ring 3 (`mind::keys`).
use crate::abi::SYSCALL_READ_KEY;
use crate::sys::call;

pub use crate::keys::{Code, Key};

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
