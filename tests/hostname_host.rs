// Host tests of libmind's machine name (000-KRN-0180, with 000-APP-0123's rule): the label, its capitals, the file's
// line, and the default with its reason.
extern crate alloc;
#[path = "../libmind/src/hostname.rs"]
mod hostname;
use hostname::{label, read, Source, DEFAULT, MAX};

#[test]
fn a_label_is_1_to_63_of_letters_digits_and_inner_hyphens() {
    assert_eq!(label("dexp").as_deref(), Ok("dexp"));
    assert_eq!(label("a").as_deref(), Ok("a"));
    assert_eq!(label("lab-2").as_deref(), Ok("lab-2"));
    let longest = "a".repeat(MAX);
    assert_eq!(label(&longest).as_deref(), Ok(longest.as_str()));
    assert_eq!(label(""), Err("NO NAME"));
    assert_eq!(label(&"a".repeat(MAX + 1)), Err("LONGER THAN 63 CHARACTERS"));
    for bad in ["my.mac", "under_score", "space d", "имя", "a/b"] {
        assert_eq!(label(bad), Err("A CHARACTER OTHER THAN a-z 0-9 -"), "{bad}");
    }
    assert_eq!(label("-dexp"), Err("A HYPHEN FIRST OR LAST"));
    assert_eq!(label("dexp-"), Err("A HYPHEN FIRST OR LAST"));
}

#[test]
fn capitals_are_taken_as_small_letters_when_set() {
    assert_eq!(label("DEXP-Lab").as_deref(), Ok("dexp-lab"));
}

#[test]
fn the_file_holds_one_label_and_a_line_end() {
    assert_eq!(read(Some(b"dexp\n")), (String::from("dexp"), Source::File));
    assert_eq!(read(Some(b"dexp\r\n")), (String::from("dexp"), Source::File));
    assert_eq!(read(Some(b"dexp")), (String::from("dexp"), Source::File));
}

#[test]
fn a_missing_or_invalid_file_gives_mindcore_and_why() {
    assert_eq!(read(None), (String::from(DEFAULT), Source::Missing));
    assert_eq!(read(Some(b"")), (String::from(DEFAULT), Source::Invalid("NO NAME")));
    assert_eq!(read(Some(b"dexp\nsecond\n")), (String::from(DEFAULT), Source::Invalid("A CHARACTER OTHER THAN a-z 0-9 -")));
    // The shell writes small letters: a file with capitals was not written by it, and holds no name.
    assert_eq!(read(Some(b"DEXP\n")), (String::from(DEFAULT), Source::Invalid("CAPITAL LETTERS")));
    assert_eq!(read(Some(&[0xff, 0xfe])), (String::from(DEFAULT), Source::Invalid("NOT UTF-8")));
}
