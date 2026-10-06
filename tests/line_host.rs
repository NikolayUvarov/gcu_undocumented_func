//! Host tests of libmind/src/line.rs (issue 209): `println!` hands the log one write per line, so lines of tasks that
//! print at once on several CPUs stay whole on the console (the kernel writes one LOG call under its lock).
#[path = "../libmind/src/line.rs"]
mod line;

use core::fmt::Write;
use line::{Line, LINE_MAX};

fn writes(f: impl FnOnce(&mut Line<&mut dyn FnMut(&[u8])>)) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut sink = |bytes: &[u8]| out.push(bytes.to_vec());
        let mut line = Line::new(&mut sink as &mut dyn FnMut(&[u8]));
        f(&mut line);
    }
    out
}

#[test]
fn a_formatted_line_is_one_write() {
    // init's "[INIT] STARTED {}" was two writes, and another service's line could land between them.
    let out = writes(|l| { let _ = write!(l, "[INIT] STARTED {} PID={}", "logd", 2); l.push(b"\n"); });
    assert_eq!(out, vec![b"[INIT] STARTED logd PID=2\n".to_vec()]);
}

#[test]
fn interleaved_writers_keep_their_lines_whole() {
    // Two tasks printing piece by piece at once: each line reaches the shared console in one write.
    let mut console: Vec<u8> = Vec::new();
    let mut a = writes(|l| { for piece in ["[INIT] ", "STARTED ", "logd"] { let _ = l.write_str(piece); } l.push(b"\n"); });
    let mut b = writes(|l| { for piece in ["[LOGD] ", "READY"] { let _ = l.write_str(piece); } l.push(b"\n"); });
    while !a.is_empty() || !b.is_empty() {
        if !a.is_empty() { console.extend(a.remove(0)); }
        if !b.is_empty() { console.extend(b.remove(0)); }
    }
    assert_eq!(String::from_utf8(console).unwrap(), "[INIT] STARTED logd\n[LOGD] READY\n");
}

#[test]
fn a_long_line_goes_in_pieces_of_line_max() {
    let text = "x".repeat(LINE_MAX * 2 + 10);
    let out = writes(|l| { let _ = l.write_str(&text); l.push(b"\n"); });
    assert_eq!(out.iter().map(Vec::len).collect::<Vec<_>>(), vec![LINE_MAX, LINE_MAX, 11]);
}

#[test]
fn nothing_is_written_for_nothing() {
    assert!(writes(|_| {}).is_empty());
}
