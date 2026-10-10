//! Host tests of the shell console's lines (shell/src/ring.rs): wrapping, the input line's positions, truncation, the
//! scrollback, and a console that follows its window's size (211-APP-0040).
#![allow(dead_code)]
#[path = "../shell/src/ring.rs"]
mod ring;
use ring::{Position, Ring, LINES};

fn ring(stride: usize, cols: usize, rows: usize) -> Ring<Vec<u32>> { Ring::new(vec![0; LINES * stride], stride, cols, rows) }

fn write(ring: &mut Ring<Vec<u32>>, text: &str) { for ch in text.chars() { ring.put(ch); } }

// Line `line` as text, `width` characters of it, without trailing spaces.
fn line(ring: &mut Ring<Vec<u32>>, line: u64, width: usize) -> String {
    ring.row(line)[..width].iter().map(|&c| char::from_u32(c).unwrap()).collect::<String>().trim_end().to_string()
}

// The lines on view, each `width` characters.
fn view(ring: &mut Ring<Vec<u32>>, width: usize) -> Vec<String> {
    let (top, bottom) = ring.view();
    (top..=bottom).map(|l| line(ring, l, width)).collect()
}

#[test]
fn lines_wrap_at_the_width_and_the_input_line_is_found() {
    let mut r = ring(10, 10, 4);
    write(&mut r, "MIND> ");
    let prompt = r.position();
    assert_eq!(prompt, Position { line: 0, col: 6 });
    write(&mut r, "abcdefgh");
    assert_eq!(view(&mut r, 10), ["MIND> abcd", "efgh"]);
    // The cursor after 4 characters sits at the end of the full line; after 5 on the next one.
    assert_eq!(r.offset(prompt, 4), Position { line: 0, col: 10 });
    assert_eq!(r.offset(prompt, 5), Position { line: 1, col: 1 });
    // The input line redrawn: everything after the prompt goes.
    r.truncate(prompt);
    assert_eq!(view(&mut r, 10), ["MIND>"]);
    assert_eq!(r.position(), prompt);
    // \r: what comes next overwrites the line; a step back clears a character.
    write(&mut r, "\nfirst\rFI\x08");
    assert_eq!(line(&mut r, 1, 10), "F rst");
}

#[test]
fn the_scrollback_keeps_its_lines_and_scrolls_by_pages() {
    let mut r = ring(8, 8, 5);
    for n in 0..LINES + 20 { write(&mut r, &format!("{}\n", n)); }
    // The newest lines on view, the last one being written; the oldest dropped.
    assert_eq!(view(&mut r, 8), [(LINES + 16).to_string(), (LINES + 17).to_string(), (LINES + 18).to_string(), (LINES + 19).to_string(), String::new()]);
    r.scroll(true);
    assert_eq!(r.back(), 4);
    for _ in 0..200 { r.scroll(true); }
    assert_eq!(r.back(), LINES - 5, "no further back than the oldest line kept");
    assert_eq!(view(&mut r, 8)[0], "21");
    assert!(r.unscroll() && !r.unscroll());
    r.clear();
    assert_eq!(view(&mut r, 8), [""]);
}

#[test]
fn a_console_in_a_window_follows_its_size() {
    // Room for 20 columns; the window starts 12 wide and 3 high.
    let mut r = ring(20, 12, 3);
    write(&mut r, "0123456789AB\nline two\nMIND> ");
    assert_eq!(view(&mut r, 12), ["0123456789AB", "line two", "MIND>"]);
    // Narrower: what was written keeps its characters; new text wraps at the new width.
    r.resize(6, 3);
    assert_eq!(view(&mut r, 6), ["012345", "line t", "MIND>"]);
    write(&mut r, "abcdef");
    assert_eq!(view(&mut r, 6), ["line t", "MIND>", "abcdef"]);
    // Wider and higher: the first line whole again, and more lines on view.
    r.resize(16, 6);
    assert_eq!(view(&mut r, 16), ["0123456789AB", "line two", "MIND>", "abcdef"]);
    write(&mut r, "ghijklmnopqrstuv");
    assert_eq!(view(&mut r, 16)[3..], ["abcdefghijklmnop", "qrstuv"]);
    // Never wider than its room; a new size shows the newest lines.
    r.scroll(true);
    r.resize(40, 2);
    assert_eq!((r.cols, r.rows, r.back()), (20, 2, 0));
}
