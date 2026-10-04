//! Host tests of the key decoders (libmind/src/keys.rs): PS/2 set 1 and VT100/xterm input.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/keys.rs"]
mod keys;
use abi::*;
use keys::{event, with_byte, Event, Key, Layout, Ps2, Switch, Vt};

// Events without their legacy byte (the scan code or UART byte), which `legacy_bytes` checks.
fn plain(event: Event) -> Event { match event { Event::Key(word) => Event::Key(with_byte(word, 0)), other => other } }
// Not a modifier going down or up (`ps2_modifiers_going_down_and_up` checks those).
fn not_modifier(event: &Event) -> bool { !matches!(event, Event::Key(word) if keys::is_modifier(event_key(*word))) }

fn ps2(bytes: &[u8]) -> (Vec<Event>, Ps2) {
    let mut decoder = Ps2::new();
    let events = bytes.iter().filter_map(|&b| decoder.feed(b)).filter(not_modifier).map(plain).collect();
    (events, decoder)
}
fn ch(c: char, mods: u8) -> Event { Event::Key(event(0, c as u32, mods)) }
fn key(code: u16, mods: u8) -> Event { Event::Key(event(code, 0, mods)) }

#[test]
fn ps2_letters_shift_caps_and_release() {
    // a, A (Shift), release codes produce nothing, Caps Lock inverts letters only (and is reported as MOD_CAPS).
    let (events, _) = ps2(&[0x1E, 0x9E, 0x2A, 0x1E, 0x9E, 0xAA, 0x3A, 0xBA, 0x1E, 0x02]);
    assert_eq!(events, vec![ch('a', 0), ch('A', MOD_SHIFT), ch('A', MOD_CAPS), ch('1', MOD_CAPS)]);
}

#[test]
fn ps2_extended_navigation_keys_and_fake_shifts() {
    let (events, _) = ps2(&[0xE0, 0x48, 0xE0, 0xC8, 0xE0, 0x50, 0xE0, 0x4B, 0xE0, 0x4D, 0xE0, 0x47, 0xE0, 0x4F, 0xE0, 0x49, 0xE0, 0x51, 0xE0, 0x52, 0xE0, 0x53, 0xE0, 0x2A, 0xE0, 0x1C]);
    assert_eq!(events, vec![key(KEY_UP, 0), key(KEY_DOWN, 0), key(KEY_LEFT, 0), key(KEY_RIGHT, 0), key(KEY_HOME, 0), key(KEY_END, 0),
        key(KEY_PAGE_UP, 0), key(KEY_PAGE_DOWN, 0), key(KEY_INSERT, 0), key(KEY_DELETE, 0), Event::Key(event(KEY_ENTER, '\n' as u32, 0))]);
}

#[test]
fn ps2_function_keys_specials_and_modifiers() {
    let (events, _) = ps2(&[0x3B, 0x44, 0x57, 0x58, 0x01, 0x0E, 0x0F, 0x1C, 0x1D, 0xE0, 0x4D, 0x9D, 0x38, 0x3D, 0xB8, 0xE0, 0x1D, 0x2E, 0xE0, 0x9D]);
    assert_eq!(events, vec![key(KEY_F1, 0), key(KEY_F1 + 9, 0), key((KEY_F1 + 10), 0), key((KEY_F1 + 11), 0),
        Event::Key(event(KEY_ESC, 0x1B, 0)), Event::Key(event(KEY_BACKSPACE, 8, 0)), Event::Key(event(KEY_TAB, 9, 0)), Event::Key(event(KEY_ENTER, 10, 0)),
        key(KEY_RIGHT, MOD_CTRL), key(KEY_F1 + 2, MOD_ALT), ch('c', MOD_CTRL)]);
}

#[test]
fn ps2_ctrl_z_is_attention_in_any_layout_and_pause_is_skipped() {
    let (events, _) = ps2(&[0xE1, 0x1D, 0x45, 0xE1, 0x9D, 0xC5, 0x1D, 0x2C, 0xAC, 0x9D]);
    assert_eq!(events, vec![Event::Attention]);
}

#[test]
fn ps2_russian_layout_switch_by_clean_chord() {
    // Ctrl+Shift pressed and released alone switches; Ctrl+Shift+Right does not.
    let (events, decoder) = ps2(&[0x1D, 0x2A, 0xAA, 0x9D, 0x10, 0x2A, 0x10, 0xAA, 0x29, 0x33, 0x2A, 0x35, 0xAA]);
    assert_eq!(events, vec![Event::Layout(Layout::Ru), ch('й', 0), ch('Й', MOD_SHIFT), ch('ё', 0), ch('б', 0), ch(',', MOD_SHIFT)]);
    assert_eq!(decoder.layout(), Layout::Ru);
    let (events, decoder) = ps2(&[0x1D, 0x2A, 0xE0, 0x4D, 0xAA, 0x9D]);
    assert_eq!(events, vec![key(KEY_RIGHT, MOD_CTRL | MOD_SHIFT)]);
    assert_eq!(decoder.layout(), Layout::Us);
    // Alt+Shift switches too; shortcuts stay positional: Ctrl+С is Ctrl+c.
    let (events, _) = ps2(&[0x38, 0x2A, 0xB8, 0xAA, 0x1D, 0x2E, 0x9D, 0x2E, 0x3A, 0x10]);
    assert_eq!(events, vec![Event::Layout(Layout::Ru), ch('c', MOD_CTRL), ch('с', 0), ch('Й', MOD_CAPS)]);
}

#[test]
fn ps2_switch_key_and_layout_are_set() {
    let feed = |decoder: &mut Ps2, bytes: &[u8]| bytes.iter().filter_map(|&b| decoder.feed(b)).filter(not_modifier).map(plain).collect::<Vec<_>>();
    let (ctrl_shift, alt_shift, caps) = ([0x1D, 0x2A, 0xAA, 0x9D], [0x38, 0x2A, 0xB8, 0xAA], [0x3A, 0xBA]);
    let mut decoder = Ps2::new();
    assert_eq!(decoder.switch(), Switch::CtrlOrAltShift);
    decoder.set_layout(Layout::Ru);
    assert_eq!(feed(&mut decoder, &[0x10]), vec![ch('й', 0)], "set_layout takes effect at the next key");
    // Only Alt+Shift switches.
    decoder.set_switch(Switch::AltShift);
    assert!(feed(&mut decoder, &ctrl_shift).is_empty());
    assert_eq!(feed(&mut decoder, &alt_shift), vec![Event::Layout(Layout::Us)]);
    // Only Ctrl+Shift.
    decoder.set_switch(Switch::CtrlShift);
    assert!(feed(&mut decoder, &alt_shift).is_empty());
    assert_eq!(feed(&mut decoder, &ctrl_shift), vec![Event::Layout(Layout::Ru)]);
    // Caps Lock switches and no longer locks capitals; the chords do nothing.
    decoder.set_switch(Switch::CapsLock);
    assert_eq!(feed(&mut decoder, &caps), vec![Event::Layout(Layout::Us)]);
    assert!(feed(&mut decoder, &ctrl_shift).is_empty() && feed(&mut decoder, &alt_shift).is_empty());
    assert_eq!(feed(&mut decoder, &[0x1E]), vec![ch('a', 0)], "no capitals lock");
    // None: nothing switches; Caps Lock locks capitals again.
    decoder.set_switch(Switch::None);
    assert!(feed(&mut decoder, &ctrl_shift).is_empty() && feed(&mut decoder, &alt_shift).is_empty());
    assert_eq!(feed(&mut decoder, &[0x3A, 0xBA, 0x1E]), vec![ch('A', MOD_CAPS)]);
    assert_eq!(decoder.layout(), Layout::Us);
}

#[test]
fn ps2_modifiers_going_down_and_up() {
    // Shift down (its make code repeats while held: reported once), Shift+F4, Shift up; right Ctrl; left Alt.
    let m = |key: u16, mods: u8, pressed: bool| Event::Key(input_event(0, key, mods, pressed, 0));
    let mut decoder = Ps2::new();
    let events: Vec<Event> = [0x2A, 0x2A, 0x2A, 0x3E, 0xAA, 0xE0, 0x1D, 0xE0, 0x9D, 0x38, 0xB8].iter().filter_map(|&b| decoder.feed(b)).collect();
    assert_eq!(events, vec![m(KEY_SHIFT, MOD_SHIFT, true), Event::Key(with_byte(event(KEY_F1 + 3, 0, MOD_SHIFT), 0x3E)), m(KEY_SHIFT, 0, false),
                            m(KEY_CTRL, MOD_CTRL, true), m(KEY_CTRL, 0, false), m(KEY_ALT, MOD_ALT, true), m(KEY_ALT, 0, false)]);
    // They are no key presses and carry no legacy byte (READ_KEY never sees them).
    for event in &events {
        if let Event::Key(word) = *event { if keys::is_modifier(event_key(word)) { assert!(Key::from_event(word).is_none() && event_byte(word) == 0); } }
    }
    // Ctrl+Shift switching the layout: Shift's release goes into the switch; `modifiers()` says Ctrl is still held.
    let mut decoder = Ps2::new();
    let events: Vec<Event> = [0x1D, 0x2A, 0xAA].iter().filter_map(|&b| decoder.feed(b)).collect();
    assert_eq!(events, vec![m(KEY_CTRL, MOD_CTRL, true), m(KEY_SHIFT, MOD_CTRL | MOD_SHIFT, true), Event::Layout(Layout::Ru)]);
    assert_eq!(decoder.modifiers(), m(KEY_SHIFT, MOD_CTRL, false));
    assert_eq!(decoder.feed(0x9D), Some(m(KEY_CTRL, 0, false)));
}

#[test]
fn ps2_keypad_follows_num_lock() {
    let (events, _) = ps2(&[0x48, 0x45, 0x48, 0x53]);
    assert_eq!(events, vec![key(KEY_UP, 0), ch('8', 0), ch('.', 0)]);
}

fn vt(input: &[u8]) -> Vec<Event> {
    let mut decoder = Vt::new();
    let mut events = Vec::new();
    for (i, &b) in input.iter().enumerate() { decoder.feed(b, i as u64, &mut |e| events.push(plain(e))); }
    decoder.poll(1_000, &mut |e| events.push(plain(e)));
    events
}

#[test]
fn legacy_bytes_and_event_words() {
    // A key press carries the scan code (PS/2) or the UART byte that completed it, for READ_KEY (common/abi.rs).
    let mut decoder = Ps2::new();
    let Some(Event::Key(word)) = decoder.feed(0x1E) else { panic!() };
    assert_eq!((event_byte(word), event_key(word), event_char(word), event_pressed(word)), (0x1E, KEY_CHAR, 'a' as u32, true));
    let mut events = Vec::new();
    let mut vt = Vt::new();
    for (i, &b) in b"\x1b[A\r".iter().enumerate() { vt.feed(b, i as u64, &mut |e| events.push(e)); }
    let words: Vec<usize> = events.iter().map(|e| match e { Event::Key(w) => *w, _ => 0 }).collect();
    assert_eq!(words.iter().map(|&w| (event_byte(w), event_key(w))).collect::<Vec<_>>(), vec![(b'A', KEY_UP), (b'\r', KEY_ENTER)]);
    let key = Key(words[1]);
    assert_eq!((key.code(), key.char(), key.byte()), (keys::Code::Enter, Some('\n'), b'\r'));
    assert_eq!(Key::from_event(input_event(b'x', 0, 0, true, 0)), None, "an undecoded event is not a key");
    assert_eq!(Key::from_event(input_event(0, KEY_UP, 0, false, 0)), None, "a release is not a key");
}

#[test]
fn vt_text_utf8_and_line_endings() {
    assert_eq!(vt("aЖ€\r\n\n".as_bytes()), vec![ch('a', 0), ch('Ж', 0), ch('€', 0), Event::Key(event(KEY_ENTER, 10, 0)), Event::Key(event(KEY_ENTER, 10, 0))]);
    assert_eq!(vt(b"\x7f\x08\t\x03\x1a"), vec![Event::Key(event(KEY_BACKSPACE, 8, 0)), Event::Key(event(KEY_BACKSPACE, 8, 0)), Event::Key(event(KEY_TAB, 9, 0)), ch('c', MOD_CTRL), Event::Attention]);
    // A broken UTF-8 sequence is dropped, the next character survives.
    assert_eq!(vt(&[0xD0, b'x']), vec![ch('x', 0)]);
}

#[test]
fn vt_sequences_with_modifiers() {
    assert_eq!(vt(b"\x1b[A\x1b[B\x1b[C\x1b[D\x1b[H\x1b[F\x1bOP\x1bOS\x1b[15~\x1b[24~\x1b[2~\x1b[3~\x1b[5~\x1b[6~\x1b[1~\x1b[4~"),
        vec![key(KEY_UP, 0), key(KEY_DOWN, 0), key(KEY_RIGHT, 0), key(KEY_LEFT, 0), key(KEY_HOME, 0), key(KEY_END, 0), key(KEY_F1, 0), key(KEY_F1 + 3, 0),
             key(KEY_F1 + 4, 0), key((KEY_F1 + 11), 0), key(KEY_INSERT, 0), key(KEY_DELETE, 0), key(KEY_PAGE_UP, 0), key(KEY_PAGE_DOWN, 0), key(KEY_HOME, 0), key(KEY_END, 0)]);
    assert_eq!(vt(b"\x1b[1;2C\x1b[1;5D\x1b[1;3A\x1b[3;5~\x1b[1;2P\x1b[Z\x1b[[A"),
        vec![key(KEY_RIGHT, MOD_SHIFT), key(KEY_LEFT, MOD_CTRL), key(KEY_UP, MOD_ALT), key(KEY_DELETE, MOD_CTRL), key(KEY_F1, MOD_SHIFT),
             Event::Key(event(KEY_TAB, 9, MOD_SHIFT)), key(KEY_F1, 0)]);
}

#[test]
fn vt_lone_escape_after_timeout_or_before_another_byte() {
    let esc = Event::Key(event(KEY_ESC, 0x1B, 0));
    assert_eq!(vt(b"\x1b"), vec![esc]);
    assert_eq!(vt(b"\x1b\n"), vec![esc, Event::Key(event(KEY_ENTER, 10, 0))]);
    assert_eq!(vt(b"\x1b\x1b[A"), vec![esc, key(KEY_UP, 0)]);
    let mut decoder = Vt::new();
    let mut events = Vec::new();
    decoder.feed(0x1B, 100, &mut |e| events.push(e));
    decoder.poll(120, &mut |e| events.push(e));
    assert!(events.is_empty() && decoder.pending(), "Esc waits for a possible sequence");
    decoder.poll(100 + keys::ESC_TIMEOUT_MS, &mut |e| events.push(e));
    assert_eq!(events, vec![esc]);
}

#[test]
fn vt_slow_sequence_is_not_cut_by_the_escape_timeout() {
    // Bytes 30 ms apart (a slow serial line): the sequence still decodes as one key.
    let mut decoder = Vt::new();
    let mut events = Vec::new();
    for (i, &b) in b"\x1b[3;5~".iter().enumerate() { let now = i as u64 * 30; decoder.poll(now, &mut |e| events.push(plain(e))); decoder.feed(b, now, &mut |e| events.push(plain(e))); }
    assert_eq!(events, vec![key(KEY_DELETE, MOD_CTRL)]);
}

#[test]
fn vt_utf8_after_an_old_sequence_is_not_dropped_by_poll() {
    let mut decoder = Vt::new();
    let mut events = Vec::new();
    decoder.feed(0x1B, 0, &mut |e| events.push(plain(e))); decoder.feed(b'[', 0, &mut |e| events.push(plain(e))); decoder.feed(b'A', 0, &mut |e| events.push(plain(e)));
    decoder.feed(0xD0, 5_000, &mut |e| events.push(plain(e)));
    decoder.poll(5_000, &mut |e| events.push(plain(e)));
    decoder.feed(0x96, 5_010, &mut |e| events.push(plain(e)));
    assert_eq!(events, vec![key(KEY_UP, 0), ch('Ж', 0)]);
}

#[test]
fn letter_commands_in_either_layout() {
    let key = |ch: char| Key(event(0, ch as u32, 0));
    assert_eq!(key('з').latin(), Some('p'));
    assert_eq!(key('З').latin(), Some('P'));
    assert_eq!(key('е').latin(), Some('t'));
    assert_eq!(key('Ы').latin(), Some('S'));
    assert_eq!(key('q').latin(), Some('q'));
    assert_eq!(key('+').latin(), Some('+'));
    assert_eq!(Key(event(0, 'з' as u32, MOD_CTRL)).latin(), None);
    assert_eq!(Key(event(KEY_ENTER, '\n' as u32, 0)).latin(), None);
}
