use crate::abi::SYSCALL_READ_KEY;
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
