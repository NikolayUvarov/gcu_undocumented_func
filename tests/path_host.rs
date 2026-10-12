//! Host tests of `mind::path` (libmind/src/path.rs, 000-APP-0066): paths from a current directory, and Tab's
//! completion of a name.
extern crate alloc;
#[path = "../libmind/src/path.rs"]
mod path;
use path::*;

#[test]
fn a_path_is_taken_from_the_current_directory() {
    for (cwd, typed, full) in [
        ("", "diag", "diag"), ("diag", "quick.msh", "diag/quick.msh"), ("diag", "..", ""), ("diag", "../data/x", "data/x"),
        ("diag", "/docs", "docs"), ("diag", "ram:notes", "ram:notes"), ("ram:notes", "a/./b//", "ram:notes/a/b"),
        ("ram:notes", "..", "ram:"), ("RAM:x", ".", "ram:x"), ("log:boot", "/", ""), ("diag", "", "diag"),
        ("data/a/b", "../../c", "data/c"), ("diag", "LOG:", "log:"),
    ] {
        assert_eq!(resolve(cwd, typed), full, "{:?} from {:?}", typed, cwd);
    }
    // Above a root: as typed, for the file service to refuse; never cut at the root (paths cannot climb out).
    for (cwd, typed) in [("", ".."), ("ram:", ".."), ("", "ram:../kernel.elf"), ("diag", "../../kernel.elf"), ("data/a", "/../x"), ("ram:x", "../../y")] {
        assert_eq!(resolve(cwd, typed), typed, "{:?} from {:?}", typed, cwd);
    }
    assert!(relative("quick.msh") && relative("a/b") && !relative("-f") && !relative("/x") && !relative("ram:x") && !relative(""));
}

#[test]
fn pwd_and_the_prompt_name_the_directory() {
    assert_eq!((shown(""), shown("diag"), shown("ram:"), shown("ram:notes")), ("/".into(), "/diag".into(), "ram:".into(), "ram:notes".into()));
    assert_eq!((prompt(""), prompt("diag"), prompt("ram:")), ("".into(), " diag".into(), " ram:".into()));
}

#[test]
fn tab_completes_a_name_or_lists_the_choices() {
    assert_eq!(split_word("diag/q"), ("diag/", "q"));
    assert_eq!(split_word("ram:"), ("ram:", ""));
    assert_eq!(split_word("ram:no"), ("ram:", "no"));
    assert_eq!(split_word("q"), ("", "q"));
    let names = [("quick.msh", false), ("full.msh", false), ("logs", true), ("LONG.TXT", false)];
    // One match: its name, and `/` after a directory or a space after a file.
    assert_eq!(complete("q", &names), Completion::Add("uick.msh ".into()));
    assert_eq!(complete("logs", &names), Completion::Add("/".into()));
    assert_eq!(complete("Q", &names), Completion::Add("uick.msh ".into()), "letter case aside");
    // Several: what they share, then (nothing more to add) the list.
    assert_eq!(complete("l", &names), Completion::Add("o".into()));
    assert_eq!(complete("lo", &names), Completion::List(vec!["logs/".into(), "LONG.TXT".into()]));
    assert_eq!(complete("", &[("b", false), ("a", true)]), Completion::List(vec!["a/".into(), "b".into()]));
    assert_eq!(complete("x", &names), Completion::None);
    // Commands: the shared start of check and chmod-like names.
    assert_eq!(complete("ch", &[("check", false), ("chat", false)]), Completion::List(vec!["chat".into(), "check".into()]));
    assert_eq!(complete("ки", &[("кирпич", false), ("кино", false)]), Completion::List(vec!["кино".into(), "кирпич".into()]), "on characters, not bytes");
    assert_eq!(complete("к", &[("кирпич", false), ("кино", false)]), Completion::Add("и".into()));
}
