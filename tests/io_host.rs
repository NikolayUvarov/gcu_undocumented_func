//! Host tests of `mind::io::Lines` (000-KRN-0094): lines cut anywhere by the reads, `\r\n`, a last line without its
//! end, an empty input, a line longer than LINE_MAX in pieces, and a read's error passed on.
#[path = "../libmind/src/io.rs"]
mod io;
use io::{Lines, LINE_MAX};

// Every line of `input`, read `step` bytes at a time.
fn lines(input: &[u8], step: usize) -> Vec<Vec<u8>> {
    let (mut lines, mut at, mut out) = (Lines::new(), 0, Vec::new());
    let mut read = |buffer: &mut [u8]| -> Result<usize, ()> {
        let n = step.min(buffer.len()).min(input.len() - at);
        buffer[..n].copy_from_slice(&input[at..at + n]);
        at += n;
        Ok(n)
    };
    while let Some(line) = lines.next_from(&mut read).unwrap() { out.push(line.to_vec()); }
    out
}

#[test]
fn lines_whatever_the_reads() {
    let input = b"one\ntwo\r\n\nthree";
    for step in [1, 2, 3, 5, 7, 4096] {
        assert_eq!(lines(input, step), vec![b"one".to_vec(), b"two".to_vec(), b"".to_vec(), b"three".to_vec()], "step {}", step);
    }
}

#[test]
fn an_empty_input_has_no_line() {
    assert!(lines(b"", 10).is_empty());
    assert_eq!(lines(b"\n", 10), vec![b"".to_vec()]);
}

#[test]
fn a_long_line_comes_in_pieces() {
    let mut input = vec![b'x'; LINE_MAX + 10];
    input.extend_from_slice(b"\nend\n");
    let got = lines(&input, 1000);
    assert_eq!(got.len(), 3);
    assert_eq!((got[0].len(), got[1].len(), got[2].as_slice()), (LINE_MAX, 10, &b"end"[..]));
}

#[test]
fn a_read_error_is_passed_on() {
    let mut lines = Lines::new();
    assert_eq!(lines.next_from(|_: &mut [u8]| Err::<usize, &str>("gone")).err(), Some("gone"));
}
