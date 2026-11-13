//! Host tests of the clipboard's state (clipboard/src/state.rs, 000-APP-0032, idl/clipboard.wit 1.0): a text set in parts,
//! in order and within 65536 bytes; its generations; parts read back where a character ends; the program in front alone
//! answered, a manager always, a client with neither badge never.
#![allow(dead_code)]
extern crate alloc;
#[path = "../clipboard/src/state.rs"]
mod state;
use state::{Clipboard, Refusal, Who, MAX, PART};

const SHELL: u64 = 5;
const EDIT: u64 = 40;
const OTHER: u64 = 41;

fn set_all(c: &mut Clipboard, who: Who, pid: u64, text: &str) -> Result<u32, Refusal> {
    let mut at = 0;
    loop {
        let mut end = (at + PART).min(text.len());
        while !text.is_char_boundary(end) { end -= 1; }
        let last = end == text.len();
        let generation = c.set_part(who, pid, at, &text.as_bytes()[at..end], last)?;
        if last { return Ok(generation); }
        at = end;
    }
}

fn get_all(c: &Clipboard, who: Who, pid: u64) -> Result<(u32, String), Refusal> {
    let (generation, length, first) = c.get_part(who, pid, 0, 0)?;
    let mut text = first.to_vec();
    while text.len() < length {
        let (_, _, part) = c.get_part(who, pid, generation, text.len())?;
        assert!(!part.is_empty() && part.len() <= PART);
        text.extend_from_slice(part);
    }
    Ok((generation, String::from_utf8(text).unwrap()))
}

#[test]
fn the_program_in_front_alone_copies_and_pastes() {
    let mut c = Clipboard::new();
    assert_eq!(c.generation_for(Who::Program), Ok(0));
    assert_eq!(c.generation_for(Who::Other), Err(Refusal::Refused));
    // No program is in front yet: a program is behind; a manager is always answered.
    assert_eq!(c.set_part(Who::Program, EDIT, 0, b"x", true), Err(Refusal::Behind));
    assert_eq!(set_all(&mut c, Who::Manager, SHELL, "from the shell's line"), Ok(1));
    assert_eq!(c.set_front(Who::Program, EDIT), Err(Refusal::Refused), "only a manager names the program in front");
    assert_eq!(c.set_front(Who::Manager, EDIT), Ok(()));
    assert_eq!(get_all(&c, Who::Program, EDIT), Ok((1, String::from("from the shell's line"))));
    assert_eq!(c.get_part(Who::Program, OTHER, 0, 0).map(|_| ()), Err(Refusal::Behind), "a program behind cannot read it");
    assert_eq!(c.get_part(Who::Other, EDIT, 0, 0).map(|_| ()), Err(Refusal::Refused), "nor one with neither badge");
    assert_eq!(set_all(&mut c, Who::Program, EDIT, "copied in edit"), Ok(2));
    assert_eq!(get_all(&c, Who::Manager, SHELL), Ok((2, String::from("copied in edit"))));
    // Once another is in front, edit is behind.
    assert_eq!(c.set_front(Who::Manager, OTHER), Ok(()));
    assert_eq!(c.set_part(Who::Program, EDIT, 0, b"late", true), Err(Refusal::Behind));
    assert_eq!(c.set_front(Who::Manager, 0), Ok(()));
    assert_eq!(c.get_part(Who::Program, OTHER, 0, 0).map(|_| ()), Err(Refusal::Behind), "none in front: no program is answered");
}

#[test]
fn a_text_goes_in_parts_in_order_within_64_kib() {
    let mut c = Clipboard::new();
    c.set_front(Who::Manager, EDIT).unwrap();
    // 65536 bytes of Cyrillic and ASCII: parts end where a character ends, and the text reads back whole.
    let text: String = "Жук ".repeat(MAX / 7).chars().chain(core::iter::repeat('.')).take_while({ let mut n = 0; move |c| { n += c.len_utf8(); n <= MAX } }).collect();
    assert!(text.len() <= MAX && text.len() > MAX - 4);
    assert_eq!(set_all(&mut c, Who::Program, EDIT, &text), Ok(1));
    assert_eq!(get_all(&c, Who::Program, EDIT).unwrap().1, text);
    // A part not where the last ended, or past 65536 bytes in all, drops the caller's text; the clipboard keeps its own.
    assert_eq!(c.set_part(Who::Program, EDIT, 0, b"abc", false), Ok(0));
    assert_eq!(c.set_part(Who::Program, EDIT, 5, b"def", true), Err(Refusal::Invalid));
    assert_eq!(c.set_part(Who::Program, EDIT, 3, b"def", true), Err(Refusal::Invalid), "dropped: it starts again at 0");
    assert_eq!(c.generation(), 1);
    assert_eq!(c.set_part(Who::Program, EDIT, 0, &[b'a'; PART], false), Ok(0));
    let mut at = PART;
    while at + PART <= MAX { assert_eq!(c.set_part(Who::Program, EDIT, at, &[b'a'; PART], false), Ok(0)); at += PART; }
    assert_eq!(c.set_part(Who::Program, EDIT, at, &[b'a'; PART], true), Err(Refusal::Invalid), "past 65536 bytes");
    assert_eq!((c.generation(), c.length()), (1, text.len()));
    // A read from the middle of a character, or past the end, is refused.
    assert_eq!(c.get_part(Who::Program, EDIT, 0, 1).map(|_| ()), Err(Refusal::Invalid), "Ж is two bytes");
    assert_eq!(c.get_part(Who::Program, EDIT, 0, text.len() + 1).map(|_| ()), Err(Refusal::Invalid));
}

#[test]
fn a_text_set_meanwhile_is_told_and_two_callers_keep_their_own_parts() {
    let mut c = Clipboard::new();
    c.set_front(Who::Manager, EDIT).unwrap();
    set_all(&mut c, Who::Program, EDIT, &"a".repeat(PART + 10)).unwrap();
    let (generation, _, _) = c.get_part(Who::Program, EDIT, 0, 0).unwrap();
    set_all(&mut c, Who::Manager, SHELL, "new").unwrap();
    assert_eq!(c.get_part(Who::Program, EDIT, generation, PART).map(|_| ()), Err(Refusal::Changed));
    // A manager's text in parts and a program's at once: each continues its own.
    assert_eq!(c.set_part(Who::Program, EDIT, 0, b"edit ", false), Ok(0));
    assert_eq!(c.set_part(Who::Manager, SHELL, 3, b"x", false), Err(Refusal::Invalid), "not the shell's text");
    assert_eq!(c.set_part(Who::Program, EDIT, 5, b"text", true), Ok(3));
    assert_eq!(get_all(&c, Who::Manager, SHELL).unwrap().1, "edit text");
    // A part at 0 from another caller starts its own, and the first one's later part is refused.
    assert_eq!(c.set_part(Who::Program, EDIT, 0, b"one ", false), Ok(0));
    assert_eq!(c.set_part(Who::Manager, SHELL, 0, b"two", true), Ok(4));
    assert_eq!(c.set_part(Who::Program, EDIT, 4, b"more", true), Err(Refusal::Invalid));
    assert_eq!(get_all(&c, Who::Manager, SHELL).unwrap().1, "two");
}

#[test]
fn a_manager_alone_has_a_programs_client_minted() {
    // clipboard.wit 1.1: a window manager's copy comes on the wire without the right to badge (000-KRN-0073), so the
    // service mints its programs' copies, for a manager alone.
    let c = Clipboard::new();
    assert_eq!(c.program_client(Who::Manager), Ok(()));
    assert_eq!(c.program_client(Who::Program), Err(Refusal::Refused));
    assert_eq!(c.program_client(Who::Other), Err(Refusal::Refused));
}
