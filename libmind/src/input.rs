//! Keyboard input of the focused program: event words (`common/abi.rs`: legacy byte, key, modifiers, pressed,
//! character) from the PS/2 driver and the UART, decoded in ring 3 (`mind::keys`). `read_key` gives key presses,
//! `read_event` every event; `modifiers` tells which of Shift, Ctrl and Alt are held, as far as the events said.
//! A program in a window (`mind::windowed`, issue 088) reads the events the window manager queues in its surface.
use crate::abi::*;
use crate::sys::call;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

static MODIFIERS: AtomicU8 = AtomicU8::new(0);
// The pointer on a screen (issue u001): its pixel within the grid's area, which `pointer_area` sets, and whether a
// pointer event came (until then nothing shows it).
static POINTER_X: AtomicUsize = AtomicUsize::new(0);
static POINTER_Y: AtomicUsize = AtomicUsize::new(0);
static AREA_W: AtomicUsize = AtomicUsize::new(0);
static AREA_H: AtomicUsize = AtomicUsize::new(0);
static POINTER_SEEN: AtomicBool = AtomicBool::new(false);

/// The modifiers (MOD_SHIFT, MOD_CTRL, MOD_ALT) held, as the last modifier event read reported them: a key bar shows
/// what F1–F10 do with them. The PS/2 keyboard reports a modifier going down or up on its own; a terminal sends a
/// modifier only with a key and never its release, so its keys leave the modifiers alone (they would stick).
pub fn modifiers() -> u8 { MODIFIERS.load(Ordering::Relaxed) & (MOD_SHIFT | MOD_CTRL | MOD_ALT) }

fn seen(word: usize) -> usize { if crate::keys::is_modifier(event_key(word)) { MODIFIERS.store(event_mods(word), Ordering::Relaxed); } word }

pub use crate::keys::{Code, Key};

// The next event word: the focused task's, or what the window manager queued for the program's window; 0 if none.
// The kernel's pointer events move the pointer of the screen, whoever reads them.
fn next_word() -> usize {
    let word = if crate::windowed::active() { crate::windowed::event() } else { call(SYSCALL_READ_INPUT, 0, 0) };
    if event_key(word) == KEY_POINTER && crate::window::pointer_position(word).is_none() {
        let (_, dx, dy, _) = pointer_fields(word);
        let follow = |at: &AtomicUsize, area: &AtomicUsize, d: i32| {
            let limit = area.load(Ordering::Relaxed).max(1) as i64 - 1;
            at.store((at.load(Ordering::Relaxed) as i64 + d as i64).clamp(0, limit) as usize, Ordering::Relaxed);
        };
        follow(&POINTER_X, &AREA_W, dx);
        follow(&POINTER_Y, &AREA_H, dy);
        POINTER_SEEN.store(true, Ordering::Relaxed);
    }
    word
}

/// The area the pointer of a screen moves in: `width` × `height` pixels of 8 × 16 cells (`Terminal` sets its grid's).
/// The pointer starts in the middle.
pub fn pointer_area(width: usize, height: usize) {
    AREA_W.store(width, Ordering::Relaxed);
    AREA_H.store(height, Ordering::Relaxed);
    POINTER_X.store(width / 2, Ordering::Relaxed);
    POINTER_Y.store(height / 2, Ordering::Relaxed);
}

/// The cell the pointer of the screen is on, once a pointer event came (a window's manager draws its own pointer).
pub fn pointer_cell() -> Option<(usize, usize)> {
    POINTER_SEEN.load(Ordering::Relaxed).then(|| (POINTER_X.load(Ordering::Relaxed) / 8, POINTER_Y.load(Ordering::Relaxed) / 16))
}

/// Next key press of the calling (focused) task; releases and events without a decoded key are skipped.
pub fn read_key() -> Option<Key> {
    loop {
        match next_word() { 0 => return None, word => if let Some(key) = Key::from_event(seen(word)) { return Some(key); } }
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

/// Waits for a key; None as soon as the modifiers held are no longer `shown` (a key bar to draw again), or the window
/// manager asked for another size (the program draws again at that size).
pub fn wait_key_or_modifiers(shown: u8) -> Option<Key> {
    loop {
        if let Some(key) = wait_key(1000) { return Some(key); }
        if modifiers() != shown || crate::windowed::resize_pending() { return None; }
    }
}

/// A key press or a pointer event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOrPointer { Key(Key), Pointer(Pointer) }

/// As `wait_key_or_modifiers`, with pointer events too (a program that asked for them with `pointer`).
pub fn wait_key_pointer_or_modifiers(shown: u8) -> Option<KeyOrPointer> {
    let next = || loop {
        match read_input()? {
            Input::Key(event) => if let Some(key) = Key::from_event(event.to_word()) { return Some(KeyOrPointer::Key(key)); },
            Input::Pointer(pointer) => return Some(KeyOrPointer::Pointer(pointer)),
        }
    };
    loop {
        if let Some(input) = next() { return Some(input); }
        crate::time::sleep(1000);
        if let Some(input) = next() { return Some(input); }
        if modifiers() != shown || crate::windowed::resize_pending() { return None; }
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

/// A pointer event (issues 156, u001): buttons held (`POINTER_*`), movement (dy grows downwards), wheel steps
/// (negative: away from the user, to scroll up) and the cell the pointer is on. In a window the window manager says
/// the cell and the movement is 0; on a screen the cell follows the movement within `pointer_area`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pointer { pub buttons: u8, pub dx: i32, pub dy: i32, pub wheel: i32, pub x: usize, pub y: usize }

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
    match next_word() {
        0 => None,
        word if event_key(word) == KEY_POINTER => {
            let (buttons, dx, dy, wheel) = pointer_fields(word);
            Some(Input::Pointer(match crate::window::pointer_position(word) {
                Some((x, y)) => Pointer { buttons, dx: 0, dy: 0, wheel, x, y },
                None => { let (x, y) = pointer_cell().unwrap_or((0, 0)); Pointer { buttons, dx, dy, wheel, x, y } }
            }))
        }
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
