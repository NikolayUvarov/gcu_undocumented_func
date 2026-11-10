//! Host tests of `recovery` (308-APP-0122): the key's written form and its file, the words without a screen, and the
//! screen's steps against a stand-in store: the key shown once, typed back to keep it (a typo told before the store is
//! asked, another key refused, the older keys kept when it closes), and a closed store opened with a key.
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
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[path = "../libmind/src/blockstore.rs"]
mod blockstore;
#[path = "../recovery/src/keyfile.rs"]
mod keyfile;
#[path = "../recovery/src/view.rs"]
mod view;
use keyfile::*;
use keys::Key;
use view::*;

fn key(code: u16) -> Key { Key(keys::event(code, 0, 0)) }
fn char_key(c: char) -> Key { Key(keys::event(0, c as u32, 0)) }
fn typed(r: &mut Recovery, s: &mut dyn Store, text: &str) { for c in text.chars() { r.key(char_key(c), s); } }

fn screen(r: &Recovery) -> Vec<String> {
    let mut cells = vec![tui::Cell::BLANK; 100 * 30];
    let mut grid = tui::Grid::new(&mut cells, 100, 30);
    r.draw(&mut grid, &tui::DARK);
    (0..30).map(|y| (0..100).map(|x| grid.get(x, y).ch).collect::<String>()).collect()
}

fn text(key: &[u8; 16]) -> String { blockstore::recovery_text(key).iter().map(|&b| b as char).collect() }

/// A stand-in of the store's recovery calls: the keys it kept, the one it made and not yet kept, open or closed.
struct Stand { closed: bool, kept: Vec<String>, made: Option<String>, next: u8, asked: Vec<String> }

impl Stand {
    fn new(closed: bool) -> Self { Stand { closed, kept: vec![text(&[7; 16])], made: None, next: 1, asked: Vec::new() } }
}

impl Store for Stand {
    fn records(&mut self) -> Result<u32, Refused> {
        self.asked.push(String::from("records"));
        if self.closed { Err(Refused::Store(KEY)) } else { Ok((self.kept.len() + self.made.is_some() as usize) as u32) }
    }
    fn make(&mut self) -> Result<String, Refused> {
        self.asked.push(String::from("make"));
        let key = text(&[self.next; 16]);
        self.next += 1;
        self.made = Some(key.clone());
        Ok(key)
    }
    fn keep(&mut self, key: &str) -> Result<u32, Refused> {
        self.asked.push(format!("keep {}", key));
        if self.made.as_deref() != Some(key) { return Err(Refused::Store(KEY)); }
        let erased = self.kept.len() as u32;
        self.kept = vec![self.made.take().unwrap()];
        Ok(erased)
    }
    fn open(&mut self, key: &str) -> Result<(), Refused> {
        self.asked.push(format!("open {}", key));
        if !self.kept.iter().any(|k| k == key) { return Err(Refused::Store(KEY)); }
        self.closed = false;
        Ok(())
    }
}

#[test]
fn the_key_file_holds_its_form_and_the_key() {
    let key = text(&[1; 16]);
    assert_eq!(key.len(), 39);
    assert_eq!(key.matches('-').count(), 7, "8 groups of 4");
    let file = render(&key);
    assert_eq!(file, format!("MIND CORE RECOVERY KEY 1\n{}\n", key));
    assert_eq!(parse(&file), Ok(key.clone()));
    // Case, dashes and spaces aside; the written form comes back.
    assert_eq!(parse(&format!("{}\r\n  {}  \r\n", HEADER, key.to_lowercase().replace('-', " "))), Ok(key.clone()));
    assert_eq!(parse(&format!("MIND CORE RECOVERY KEY 2\n{}\n", key)), Err(String::from("the file is of another form (MIND CORE RECOVERY KEY 2): this recovery reads MIND CORE RECOVERY KEY 1")));
    assert!(parse(&key).unwrap_err().starts_with("not a recovery key file"));
    assert_eq!(parse(HEADER), Err(String::from("no key on the file's second line")));
    let mut wrong = key.clone().into_bytes();
    wrong[0] = if wrong[0] == b'A' { b'B' } else { b'A' };
    assert_eq!(parse(&format!("{}\n{}\n", HEADER, String::from_utf8(wrong).unwrap())), Err(format!("the file's key: {}", typo(blockstore::Typo::Check))));
    assert_eq!(canonical("ABC"), Err(blockstore::Typo::Length));
    assert_eq!(canonical(&key.replacen(&key[..1], "U", 1)), Err(blockstore::Typo::Character));
}

#[test]
fn the_words_without_a_screen() {
    assert_eq!(command(""), Ok(None));
    assert_eq!(command("new --key-file data/rk.txt"), Ok(Some(Command::New(String::from("data/rk.txt")))));
    assert_eq!(command("open --key-file data/rk.txt"), Ok(Some(Command::Open(String::from("data/rk.txt")))));
    for bad in ["new", "open data/rk.txt", "new --key-file", "make --key-file x", "new --key-file a b"] {
        assert_eq!(command(bad), Err(USAGE), "{}", bad);
    }
}

#[test]
fn a_key_is_shown_once_and_kept_when_typed_back() {
    let mut store = Stand::new(false);
    let mut r = Recovery::new();
    r.refresh(&mut store);
    assert_eq!(r.state, State::Open { records: 1 });
    assert!(screen(&r).iter().any(|l| l.contains("The store is open. Recovery records on its medium: 1")));
    r.key(key(abi::KEY_ENTER), &mut store);
    let shown = store.made.clone().unwrap();
    assert!(matches!(r.mode, Mode::Shown { .. }));
    assert!(screen(&r).iter().any(|l| l.contains(&shown)), "the key on the screen");
    assert_eq!(r.status(), "KEY SHOWN; THE STORE OPEN, 1 RECOVERY RECORD(S)");
    assert!(!r.status().contains(&shown[..4]), "never in the log");
    r.key(key(abi::KEY_ENTER), &mut store);
    assert!(!screen(&r).iter().any(|l| l.contains(&shown)), "shown once: not while it is typed back");
    // A typo is told before the store is asked.
    typed(&mut r, &mut store, "1234");
    assert!(screen(&r).iter().any(|l| l.contains("••••")), "not echoed");
    r.key(key(abi::KEY_ENTER), &mut store);
    assert!(!store.asked.iter().any(|a| a.starts_with("keep")));
    let Mode::Confirm { problem, .. } = &r.mode else { panic!() };
    assert_eq!(problem.as_deref(), Some("Not taken: not 32 characters, the dashes and spaces aside. Type it again"));
    // Another valid key is not the one shown: refused without asking the store.
    typed(&mut r, &mut store, &text(&[9; 16]));
    r.key(key(abi::KEY_ENTER), &mut store);
    assert!(!store.asked.iter().any(|a| a.starts_with("keep")));
    let Mode::Confirm { problem, .. } = &r.mode else { panic!() };
    assert_eq!(problem.as_deref(), Some("This is not the key shown: type it again (Esc: the older keys stay)"));
    // The key typed back in lower case and with spaces is the one shown, and kept.
    typed(&mut r, &mut store, &shown.to_lowercase().replace('-', " "));
    r.key(key(abi::KEY_ENTER), &mut store);
    assert_eq!(store.asked.iter().filter(|a| a.starts_with("keep")).cloned().collect::<Vec<_>>(), [format!("keep {}", shown)]);
    assert!(matches!(r.mode, Mode::Menu));
    assert_eq!(r.notice, "Kept: this key opens the medium on any machine; 1 older recovery record(s) erased");
    assert_eq!(store.kept, [shown]);
    assert_eq!(r.state, State::Open { records: 1 });
}

#[test]
fn closing_before_it_is_typed_back_keeps_the_older_keys() {
    for (at_confirm, keys_pressed) in [(false, vec![abi::KEY_ESC]), (true, vec![abi::KEY_ENTER, abi::KEY_ESC])] {
        let mut store = Stand::new(false);
        let mut r = Recovery::new();
        r.refresh(&mut store);
        r.key(char_key('m'), &mut store);
        for k in keys_pressed { r.key(key(k), &mut store); }
        assert!(matches!(r.mode, Mode::Menu), "{}", at_confirm);
        assert_eq!(r.notice, "Not kept: the older recovery keys still open the medium");
        assert_eq!(store.kept, [text(&[7; 16])], "the older key kept");
        assert!(!store.asked.iter().any(|a| a.starts_with("keep")));
    }
}

#[test]
fn a_closed_store_opens_with_a_key() {
    let mut store = Stand::new(true);
    let mut r = Recovery::new();
    r.refresh(&mut store);
    assert_eq!(r.state, State::Closed);
    assert_eq!(r.selected, 1, "open is what a closed store offers");
    assert!(screen(&r).iter().any(|l| l.contains("The store is closed: no keeper of this machine keeps its volume key")));
    r.key(char_key('m'), &mut store);
    assert_eq!(r.notice, "The store is closed: open it with a recovery key first");
    assert!(!store.asked.contains(&String::from("make")));
    r.key(key(abi::KEY_ENTER), &mut store);
    assert!(matches!(r.mode, Mode::Opening { .. }));
    // A key that opens no record: nothing changes.
    typed(&mut r, &mut store, &text(&[3; 16]));
    r.key(key(abi::KEY_ENTER), &mut store);
    let Mode::Opening { problem, .. } = &r.mode else { panic!() };
    assert_eq!(problem.as_deref(), Some("Not opened: the key opens no record of this medium, or no keeper kept it: nothing changed"));
    assert!(store.closed);
    typed(&mut r, &mut store, &text(&[7; 16]));
    r.key(key(abi::KEY_ENTER), &mut store);
    assert!(!store.closed && matches!(r.mode, Mode::Menu));
    assert_eq!(r.notice, "Opened: the store mounts, and this machine's keystore keeps its volume key from now on");
    assert_eq!(r.state, State::Open { records: 1 });
}

#[test]
fn refusals_say_what_they_mean() {
    assert_eq!(refusal("make", Refused::Store(INVALID)), "this store keeps its volume key in its superblock (a RAM disk, or no keeper): a recovery key would open nothing more");
    assert_eq!(refusal("records", Refused::NoClient), "this program holds no recovery client: the shell lends it when you allow it, each time (REQUEST_RECOVER)");
    assert_eq!(refusal("open", Refused::Store(INVALID)), "the store is open already, or keeps its key in its superblock");
    assert_eq!(refusal("keep", Refused::Store(DEVICE)), "the medium did not answer");
    assert_eq!(refusal("keep", Refused::Store(99)), "the store refused (error 99)");
}
