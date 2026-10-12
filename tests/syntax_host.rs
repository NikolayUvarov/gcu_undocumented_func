//! Host tests of syntax highlighting (libmind/src/tui/syntax.rs, docs/tools §5 T4): each language on lines worked
//! out by hand (one letter per byte: k keyword, t type, s string, n number, c comment, m meta, . text), states carried
//! over lines, languages by file name, and no panic on arbitrary bytes.
#![allow(dead_code)]
extern crate alloc;
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/keys.rs"]
mod keys;
#[path = "../libmind/src/util.rs"]
mod util;
#[path = "../libmind/src/tui/mod.rs"]
mod tui;

use tui::syntax::{for_path, style, Close, Kind, State};

// The marks of each line of `lines` in the language of `file`, and the state the last line leaves.
fn marks(file: &str, lines: &[&str]) -> (Vec<String>, State) {
    let language = for_path(file).unwrap();
    let mut state = State::Normal;
    let mut out = Vec::new();
    for line in lines {
        let mut kinds = vec![Kind::Text; line.len()];
        let next = language.line(line.as_bytes(), state, &mut kinds);
        out.push(kinds.iter().map(|k| match k { Kind::Text => '.', Kind::Keyword => 'k', Kind::Type => 't', Kind::String => 's', Kind::Number => 'n', Kind::Comment => 'c', Kind::Meta => 'm' }).collect());
        state = next;
    }
    (out, state)
}

fn check(file: &str, pairs: &[(&str, &str)], last: State) {
    let lines: Vec<&str> = pairs.iter().map(|p| p.0).collect();
    let (got, state) = marks(file, &lines);
    for ((line, want), got) in pairs.iter().zip(&got) { assert_eq!(got, want, "{}: {}", file, line); }
    assert_eq!(state, last, "{}", file);
}

#[test]
fn rust() {
    check("main.rs", &[
        ("#[derive(Debug)]", "mmmmmmmmmmmmmmmm"),
        ("pub fn main() -> Result<u32, E> { let x = 0x1F + 2.5e-3; // done", "kkk.kk...........tttttt.ttt..t....kkk.....nnnn...nnnnnn..ccccccc"),
        ("    println!(\"a {} \\\" b\", 'x', '\\n', r#\"raw \"q\"#); x != y", "....mmmmmmmm.sssssssssss..sss..ssss..sssssssssss........."),
        ("/* one /* two */ still", "cccccccccccccccccccccc"),
        ("end */ fn f<'a>(s: &'a str) {}", "cccccc.kk...tt......tt.ttt...."),
        ("let s = \"multi", "kkk.....ssssss"),
        ("line\"; 0..10", "sssss..n..nn"),
        ("let r = r##\"open", "kkk.....ssssssss"),
    ], State::Quote(Close::Raw(2)));
    // A nested comment needs as many ends as it had starts.
    let (_, state) = marks("a.rs", &["/* a /* b */"]);
    assert_eq!(state, State::Comment(1));
}

#[test]
fn python_c_and_shells() {
    check("x.py", &[
        ("@decorator", "mmmmmmmmmm"),
        ("def f(a: int) -> str:  # comment", "kkk......ttt.....ttt...ccccccccc"),
        ("    s = '''doc", "........ssssss"),
        ("  still'''; t = 'it''s' + \"q\"", "ssssssssss......sssssss...sss"),
        ("    return None if 3.5 else True", "....kkkkkk.kkkk.kk.nnn.kkkk.kkkk"),
        ("x = \"unclosed", "....sssssssss"),
    ], State::Normal);
    check("x.c", &[
        ("#include <stdio.h> // io", "mmmmmmmmmmmmmmmmmmmccccc"),
        ("static const char *s = \"a\\\"b\"; int c = 'x';", "kkkkkk.kkkkk.tttt......ssssss..ttt.....sss."),
        ("/* block */ unsigned long n = 10UL;", "ccccccccccc.tttttttt.tttt.....nnnn."),
    ], State::Normal);
    check("s.sh", &[
        ("#!/bin/sh", "ccccccccc"),
        ("for f in $HOME/*.txt; do echo \"${f}\" # note", "kkk...kk.ttttt........kk......ssssss.cccccc"),
        ("x=a#b", "....."),
    ], State::Normal);
    check("t.msh", &[
        ("requires: files", "kkkkkkkk......."),
        ("let total = 0 # sum", "kkk.........n.ccccc"),
        ("for f in files(\"ram:\")? { print(\"{f}\") }", "kkk...kk.......ssssss...........sssss..."),
    ], State::Normal);
}

#[test]
fn interfaces_settings_and_documents() {
    check("v.wit", &[
        ("/// docs", "cccccccc"),
        ("interface rtc {", "kkkkkkkkk......"),
        ("    now: func() -> result<option<u32>, error>;", ".........kkkk......tttttt.tttttt.ttt.........."),
    ], State::Normal);
    check("Cargo.toml", &[
        ("[package]", "mmmmmmmmm"),
        ("name = \"mind\" # x", "tttt...ssssss.ccc"),
        ("version = 2", "ttttttt...n"),
        ("enabled = true", "ttttttt...kkkk"),
    ], State::Normal);
    check("e.ini", &[("; comment", "ccccccccc"), ("[section]", "mmmmmmmmm"), ("key = on", "ttt...kk")], State::Normal);
    check("d.json", &[("{\"key\": \"value\", \"n\": -1.5e3, \"ok\": true, \"z\": null}", ".ttttt..sssssss..ttt..nnnnnn..tttt..kkkk..ttt..kkkk.")], State::Normal);
    check("README.md", &[
        ("# Title", "kkkkkkk"),
        ("Some `code` and [a link](http://x).", ".....ssssss.............tttttttttt."),
        ("- item", "m....."),
        ("1. first", "mm......"),
        ("> quote", "ccccccc"),
        ("```rust", "mmmmmmm"),
        ("fn main() {}", "ssssssssssss"),
        ("```", "mmm"),
        ("after", "....."),
        ("~~~", "mmm"),
    ], State::Fence);
}

#[test]
fn languages_by_name() {
    assert_eq!(for_path("ram:src/MAIN.RS").map(|l| l.name), Some("Rust"));
    assert_eq!(for_path("data/sum.msh").map(|l| l.name), Some("msh"));
    assert_eq!(for_path("idl/vfs.wit").map(|l| l.name), Some("WIT"));
    assert_eq!(for_path("include/x.h").map(|l| l.name), Some("C"));
    assert!(for_path("Makefile").is_none() && for_path("notes.txt").is_none() && for_path("ram:.rs/x").is_none() && for_path("").is_none());
    // Colours keep the panel's background; plain text is the panel's own style.
    for theme in [&tui::CLASSIC, &tui::DARK] {
        assert_eq!(style(theme, Kind::Text), theme.panel);
        for kind in [Kind::Keyword, Kind::Type, Kind::String, Kind::Number, Kind::Comment, Kind::Meta] {
            let s = style(theme, kind);
            assert_eq!(s.bg, theme.panel.bg);
            assert_ne!(s.fg, theme.panel.fg);
        }
    }
}

#[test]
fn any_bytes_in_any_state() {
    // xorshift over bytes that start and end things, Cyrillic and invalid UTF-8: no panic, one kind per byte.
    let alphabet: Vec<u8> = b"#/*\"'`r\\[]{}()!$@.:=;-+e0x9 aZ_\t".iter().copied().chain("ёж".bytes()).chain([0xFF, 0xC3]).collect();
    let states = [State::Normal, State::Comment(3), State::Quote(Close::Double), State::Quote(Close::Raw(1)), State::Quote(Close::Triple(b'"')), State::Fence];
    let mut x = 0x9E37_79B9u32;
    for file in ["a.rs", "a.c", "a.py", "a.sh", "a.msh", "a.wit", "a.toml", "a.ini", "a.json", "a.md"] {
        let language = for_path(file).unwrap();
        for round in 0..400 {
            let len = (round % 40) as usize;
            let line: Vec<u8> = (0..len).map(|_| { x ^= x << 13; x ^= x >> 17; x ^= x << 5; alphabet[x as usize % alphabet.len()] }).collect();
            // A longer slice: only the line's part is written.
            let mut kinds = vec![Kind::Meta; line.len() + 3];
            language.line(&line, states[round % states.len()], &mut kinds);
            assert!(kinds[line.len()..].iter().all(|&k| k == Kind::Meta), "{} {:?}", file, line);
        }
    }
}
