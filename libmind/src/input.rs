//! Keyboard input of the focused program: event words (`common/abi.rs`: legacy byte, key, modifiers, pressed,
//! character) from the PS/2 driver and the UART, decoded in ring 3 (`mind::keys`). `read_key` gives key presses,
//! `read_event` every event; `modifiers` tells which of Shift, Ctrl and Alt are held, as far as the events said.
use crate::abi::*;
use crate::sys::call;
use core::sync::atomic::{AtomicU8, Ordering};

static MODIFIERS: AtomicU8 = AtomicU8::new(0);

/// The modifiers (MOD_SHIFT, MOD_CTRL, MOD_ALT) held, as the last modifier event read reported them: a key bar shows
/// what F1–F10 do with them. The PS/2 keyboard reports a modifier going down or up on its own; a terminal sends a
/// modifier only with a key and never its release, so its keys leave the modifiers alone (they would stick).
pub fn modifiers() -> u8 { MODIFIERS.load(Ordering::Relaxed) & (MOD_SHIFT | MOD_CTRL | MOD_ALT) }

fn seen(word: usize) -> usize { if crate::keys::is_modifier(event_key(word)) { MODIFIERS.store(event_mods(word), Ordering::Relaxed); } word }

pub use crate::keys::{Code, Key};

/// Next key press of the calling (focused) task; releases and events without a decoded key are skipped.
pub fn read_key() -> Option<Key> {
    loop {
        match call(SYSCALL_READ_INPUT, 0, 0) { 0 => return None, word => if let Some(key) = Key::from_event(seen(word)) { return Some(key); } }
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

/// Waits for a key; None as soon as the modifiers held are no longer `shown` (a key bar to draw again).
pub fn wait_key_or_modifiers(shown: u8) -> Option<Key> {
    loop {
        if let Some(key) = wait_key(1000) { return Some(key); }
        if modifiers() != shown { return None; }
    }
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

/// A pointer event (issue 156): buttons held (`POINTER_*`), movement (dy grows downwards) and wheel steps; from a
/// tablet (issue 160) the position instead, `at` = (x, y) in 1/POINTER_SCALE of the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pointer { pub buttons: u8, pub dx: i32, pub dy: i32, pub wheel: i32, pub at: Option<(u32, u32)> }

impl Pointer {
    /// The position on a screen of `width` × `height` pixels, for a tablet event.
    pub fn position(&self, width: usize, height: usize) -> Option<(usize, usize)> {
        self.at.map(|(x, y)| (x as usize * width / POINTER_SCALE as usize, y as usize * height / POINTER_SCALE as usize))
    }
}

/// A key or a pointer event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input { Key(KeyEvent), Pointer(Pointer) }

/// Asks for pointer events (`read_input` returns them) or stops them; without it a program gets none.
pub fn pointer(enable: bool) { call(SYSCALL_INPUT_POINTER, enable as usize, 0); }

/// Takes `key` with exactly the modifiers `mods` (MOD_*) out of the focused program's input and into the caller's own,
/// or gives it back (`on` false). Needs process control (issue 154).
pub fn listen(key: u16, mods: u8, on: bool) -> crate::sys::Result<()> {
    crate::sys::check(call(SYSCALL_INPUT_LISTEN, key as usize | (mods as usize) << 16, on as usize)).map(drop)
}

/// Next key or pointer event of the active program, if any.
pub fn read_input() -> Option<Input> {
    match call(SYSCALL_READ_INPUT, 0, 0) {
        0 => None,
        word if event_key(word) == KEY_POINTER => { let (buttons, dx, dy, wheel) = pointer_fields(word); Some(Input::Pointer(Pointer { buttons, dx, dy, wheel, at: pointer_position(word) })) }
        word => Some(Input::Key(KeyEvent::from_word(seen(word)))),
    }
}

/// Next input event of the active program, if any (pointer events: `read_input`).
pub fn read_event() -> Option<KeyEvent> {
    loop { match read_input()? { Input::Key(event) => return Some(event), Input::Pointer(_) => {} } }
}

/// Waits up to `ms` for an input event (the sleep ends early when one arrives).
pub fn wait_event(ms: usize) -> Option<KeyEvent> {
    if let Some(event) = read_event() { return Some(event); }
    crate::time::sleep(ms);
    read_event()
}
