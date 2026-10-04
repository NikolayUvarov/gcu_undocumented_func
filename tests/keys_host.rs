//! Host tests of the key decoders (libmind/src/keys.rs): PS/2 set 1 and VT100/xterm input.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/keys.rs"]
mod keys;
use abi::*;
use keys::{event, Event, Key, Layout, Ps2, Vt};

fn ps2(bytes: &[u8]) -> (Vec<Event>, Ps2) {
    let mut decoder = Ps2::new();
    let events = bytes.iter().filter_map(|&b| decoder.feed(b)).collect();
    (events, decoder)
}
fn ch(c: char, mods: u32) -> Event { Event::Key(event(0, c as u32, mods)) }
fn key(code: u32, mods: u32) -> Event { Event::Key(event(code, 0, mods)) }

#[test]
fn ps2_letters_shift_caps_and_release() {
    // a, A (Shift), release codes produce nothing, Caps Lock inverts letters only.
    let (events, _) = ps2(&[0x1E, 0x9E, 0x2A, 0x1E, 0x9E, 0xAA, 0x3A, 0xBA, 0x1E, 0x02]);
    assert_eq!(events, vec![ch('a', 0), ch('A', KEY_MOD_SHIFT), ch('A', 0), ch('1', 0)]);
}

#[test]
fn ps2_extended_navigation_keys_and_fake_shifts() {
    let (events, _) = ps2(&[0xE0, 0x48, 0xE0, 0xC8, 0xE0, 0x50, 0xE0, 0x4B, 0xE0, 0x4D, 0xE0, 0x47, 0xE0, 0x4F, 0xE0, 0x49, 0xE0, 0x51, 0xE0, 0x52, 0xE0, 0x53, 0xE0, 0x2A, 0xE0, 0x1C]);
    assert_eq!(events, vec![key(KEY_UP, 0), key(KEY_DOWN, 0), key(KEY_LEFT, 0), key(KEY_RIGHT, 0), key(KEY_HOME, 0), key(KEY_END, 0),
        key(KEY_PGUP, 0), key(KEY_PGDN, 0), key(KEY_INSERT, 0), key(KEY_DELETE, 0), Event::Key(event(KEY_ENTER, '\n' as u32, 0))]);
}

#[test]
fn ps2_function_keys_specials_and_modifiers() {
    let (events, _) = ps2(&[0x3B, 0x44, 0x57, 0x58, 0x01, 0x0E, 0x0F, 0x1C, 0x1D, 0xE0, 0x4D, 0x9D, 0x38, 0x3D, 0xB8, 0xE0, 0x1D, 0x2E, 0xE0, 0x9D]);
    assert_eq!(events, vec![key(KEY_F1, 0), key(KEY_F1 + 9, 0), key(KEY_F11, 0), key(KEY_F12, 0),
        Event::Key(event(KEY_ESC, 0x1B, 0)), Event::Key(event(KEY_BACKSPACE, 8, 0)), Event::Key(event(KEY_TAB, 9, 0)), Event::Key(event(KEY_ENTER, 10, 0)),
        key(KEY_RIGHT, KEY_MOD_CTRL), key(KEY_F1 + 2, KEY_MOD_ALT), ch('c', KEY_MOD_CTRL)]);
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
    assert_eq!(events, vec![Event::Layout(Layout::Ru), ch('й', 0), ch('Й', KEY_MOD_SHIFT), ch('ё', 0), ch('б', 0), ch(',', KEY_MOD_SHIFT)]);
    assert_eq!(decoder.layout(), Layout::Ru);
    let (events, decoder) = ps2(&[0x1D, 0x2A, 0xE0, 0x4D, 0xAA, 0x9D]);
    assert_eq!(events, vec![key(KEY_RIGHT, KEY_MOD_CTRL | KEY_MOD_SHIFT)]);
    assert_eq!(decoder.layout(), Layout::Us);
    // Alt+Shift switches too; shortcuts stay positional: Ctrl+С is Ctrl+c.
    let (events, _) = ps2(&[0x38, 0x2A, 0xB8, 0xAA, 0x1D, 0x2E, 0x9D, 0x2E, 0x3A, 0x10]);
    assert_eq!(events, vec![Event::Layout(Layout::Ru), ch('c', KEY_MOD_CTRL), ch('с', 0), ch('Й', 0)]);
}

#[test]
fn ps2_keypad_follows_num_lock() {
    let (events, _) = ps2(&[0x48, 0x45, 0x48, 0x53]);
    assert_eq!(events, vec![key(KEY_UP, 0), ch('8', 0), ch('.', 0)]);
}

fn vt(input: &[u8]) -> Vec<Event> {
    let mut decoder = Vt::new();
    let mut events = Vec::new();
    for (i, &b) in input.iter().enumerate() { decoder.feed(b, i as u64, &mut |e| events.push(e)); }
    decoder.poll(1_000, &mut |e| events.push(e));
    events
}

#[test]
fn vt_text_utf8_and_line_endings() {
    assert_eq!(vt("aЖ€\r\n\n".as_bytes()), vec![ch('a', 0), ch('Ж', 0), ch('€', 0), Event::Key(event(KEY_ENTER, 10, 0)), Event::Key(event(KEY_ENTER, 10, 0))]);
    assert_eq!(vt(b"\x7f\x08\t\x03\x1a"), vec![Event::Key(event(KEY_BACKSPACE, 8, 0)), Event::Key(event(KEY_BACKSPACE, 8, 0)), Event::Key(event(KEY_TAB, 9, 0)), ch('c', KEY_MOD_CTRL), Event::Attention]);
    // A broken UTF-8 sequence is dropped, the next character survives.
    assert_eq!(vt(&[0xD0, b'x']), vec![ch('x', 0)]);
}

#[test]
fn vt_sequences_with_modifiers() {
    assert_eq!(vt(b"\x1b[A\x1b[B\x1b[C\x1b[D\x1b[H\x1b[F\x1bOP\x1bOS\x1b[15~\x1b[24~\x1b[2~\x1b[3~\x1b[5~\x1b[6~\x1b[1~\x1b[4~"),
        vec![key(KEY_UP, 0), key(KEY_DOWN, 0), key(KEY_RIGHT, 0), key(KEY_LEFT, 0), key(KEY_HOME, 0), key(KEY_END, 0), key(KEY_F1, 0), key(KEY_F1 + 3, 0),
             key(KEY_F1 + 4, 0), key(KEY_F12, 0), key(KEY_INSERT, 0), key(KEY_DELETE, 0), key(KEY_PGUP, 0), key(KEY_PGDN, 0), key(KEY_HOME, 0), key(KEY_END, 0)]);
    assert_eq!(vt(b"\x1b[1;2C\x1b[1;5D\x1b[1;3A\x1b[3;5~\x1b[1;2P\x1b[Z\x1b[[A"),
        vec![key(KEY_RIGHT, KEY_MOD_SHIFT), key(KEY_LEFT, KEY_MOD_CTRL), key(KEY_UP, KEY_MOD_ALT), key(KEY_DELETE, KEY_MOD_CTRL), key(KEY_F1, KEY_MOD_SHIFT),
             Event::Key(event(KEY_TAB, 9, KEY_MOD_SHIFT)), key(KEY_F1, 0)]);
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
    for (i, &b) in b"\x1b[3;5~".iter().enumerate() { let now = i as u64 * 30; decoder.poll(now, &mut |e| events.push(e)); decoder.feed(b, now, &mut |e| events.push(e)); }
    assert_eq!(events, vec![key(KEY_DELETE, KEY_MOD_CTRL)]);
}

#[test]
fn vt_utf8_after_an_old_sequence_is_not_dropped_by_poll() {
    let mut decoder = Vt::new();
    let mut events = Vec::new();
    decoder.feed(0x1B, 0, &mut |e| events.push(e)); decoder.feed(b'[', 0, &mut |e| events.push(e)); decoder.feed(b'A', 0, &mut |e| events.push(e));
    decoder.feed(0xD0, 5_000, &mut |e| events.push(e));
    decoder.poll(5_000, &mut |e| events.push(e));
    decoder.feed(0x96, 5_010, &mut |e| events.push(e));
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
    assert_eq!(Key(event(0, 'з' as u32, KEY_MOD_CTRL)).latin(), None);
    assert_eq!(Key(event(KEY_ENTER, '\n' as u32, 0)).latin(), None);
}
