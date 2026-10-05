//! Host tests of how `say` cuts its text into rows for its screen (say/src/wrap.rs, issue u010): by characters, so
//! Cyrillic takes one column a letter; at spaces; the lines of the text kept; a word longer than a row cut in it.
#[path = "../say/src/wrap.rs"]
mod wrap;

use wrap::{cut, rows};

fn all(text: &str, columns: usize) -> Vec<&str> { rows(text, columns).collect() }

#[test]
fn the_greeting_fits_a_wide_screen_whole() {
    let greeting = "Привет. Я разум корабля. Система готова к работе. Hello world.";
    assert_eq!(all(greeting, 122), vec![greeting], "62 characters, 109 bytes: one row of a 1024-pixel screen");
}

#[test]
fn rows_are_cut_at_spaces_by_characters() {
    let greeting = "Привет. Я разум корабля. Система готова к работе. Hello world.";
    let cut = all(greeting, 20);
    assert_eq!(cut, vec!["Привет. Я разум", "корабля. Система", "готова к работе.", "Hello world."]);
    assert!(cut.iter().all(|row| row.chars().count() <= 20));
    assert_eq!(cut.join(" "), greeting, "nothing is lost");
}

#[test]
fn a_word_longer_than_a_row_is_cut_in_it() {
    assert_eq!(all("Достопримечательность рядом", 8), vec!["Достопри", "мечатель", "ность", "рядом"]);
    assert_eq!(cut("abc def", 3), ("abc", "def"), "a space just after the row");
    assert_eq!(cut("abc", 3), ("abc", ""));
    assert_eq!(cut("   abcdef", 4), ("   a", "bcdef"), "no row of spaces alone");
}

#[test]
fn lines_of_the_text_are_kept() {
    assert_eq!(all("Первая строка\n\nтретья\r\n", 40), vec!["Первая строка", "", "третья"]);
    assert_eq!(all("", 40), Vec::<&str>::new());
    assert_eq!(all("one two", 0), vec!["o", "n", "e", "t", "w", "o"], "at least a column");
}
