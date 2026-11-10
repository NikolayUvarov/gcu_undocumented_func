//! keystore's passphrase typed at the console (308-APP-0130, `keystore.wit` 1.6): the boot's wait for it, and
//! `passphrase`, which sets or changes it. The lines the shell shows, the steps of what is typed (each with no echo, in a
//! `secret::Line` wiped once sent), a new one's checks and the waits keystore keeps after wrong ones. No system calls:
//! tests/shell_host.rs.

use crate::secret::{Line, Step};
use alloc::format;
use alloc::string::String;

/// A passphrase's length in bytes (`keystore.wit` 1.6; passlock's PASSPHRASE_MIN and PASSPHRASE_MAX).
pub const MIN: usize = 8;
pub const MAX: usize = 128;

/// Why the boot waits, in one line above the prompt.
pub const WAITS: &str = "THE BOOT WAITS FOR keystore'S PASSPHRASE: NO TPM SEALS THE DEVICE KEY HERE, THE PASSPHRASE DOES; THE STORE, ITS FILES, WI-FI, TLS AND SSH START ONCE IT IS TYPED.";
/// Esc at the boot's prompt.
pub const LATER: &str = "(LATER: THEY WAIT FOR IT; `passphrase` ASKS AGAIN)";
/// Esc while one is set or changed.
pub const KEPT: &str = "(NOTHING CHANGED)";
/// Before keystore derives the key from what was typed, which takes it a while.
pub const DERIVING: &str = "(keystore DERIVES ITS KEY FROM IT...)";
pub const USAGE: &str = "USAGE: PASSPHRASE (TYPED WITH NO ECHO AFTER IT, NEVER ON THE LINE)";
pub const NOT_HERE: &str = "PASSPHRASE: TYPED AT A CONSOLE OR THE SHELL'S WINDOW ALONE, NOT IN A SCRIPT, INTO A FILE OR FOR A PROGRAM";
pub const NO_CLIENT: &str = "PASSPHRASE: THE SHELL HOLDS NO keystore CLIENT FOR IT (NO keystore ON THIS MACHINE)";
pub const NO_KEY: &str = "PASSPHRASE: keystore HOLDS NO DEVICE KEY, SO NONE IS SEALED UNDER A PASSPHRASE (THE SYSTEM LOG SAYS WHY)";
pub const NO_ANSWER: &str = "PASSPHRASE: keystore DID NOT ANSWER";

/// What is typed next: the passphrase that opens the waiting key, or for `passphrase-set` the one set (empty when none
/// is), the new one, and the new one again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask { Unlock, Old, New, Again }

/// The prompt before each, typed with no echo.
pub fn prompt(ask: Ask) -> &'static str {
    match ask {
        Ask::Unlock => "PASSPHRASE (NO ECHO; ENTER ENDS IT, ESC: LATER): ",
        Ask::Old => "THE PASSPHRASE SET (NO ECHO; ENTER ALONE WHEN NONE IS SET YET, ESC: NOTHING CHANGES): ",
        Ask::New => "THE NEW PASSPHRASE, 8 TO 128 BYTES (NO ECHO): ",
        Ask::Again => "THE NEW PASSPHRASE AGAIN (NO ECHO): ",
    }
}

/// A passphrase keystore never seals under, refused before it is sent: nothing counted.
pub fn length(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < MIN { Some("(NOT TAKEN: SHORTER THAN 8 BYTES)") } else if bytes.len() > MAX { Some("(NOT TAKEN: LONGER THAN 128 BYTES)") } else { None }
}

/// Whether a line typed at the prompt names `passphrase` with words after it, which may be one: kept out of the history.
pub fn on_line(line: &str) -> bool {
    let mut words = line.split_whitespace();
    words.next().is_some_and(|w| w.eq_ignore_ascii_case("passphrase")) && words.next().is_some()
}

/// What the shell does once a key is taken.
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
    /// More keys.
    Going,
    /// The next prompt, after a line saying why when the one typed was not taken.
    Ask(Ask, Option<&'static str>),
    /// `unlock` with `line`.
    Unlock,
    /// `passphrase-set` with `old` and `new`.
    Set,
    /// Esc or Ctrl+C: nothing sent, every line wiped.
    Cancelled,
}

/// The lines typed: the one being typed, the one set and the new one. Each is wiped when dropped.
pub struct Typing { pub ask: Ask, pub line: Line, pub old: Line, pub new: Line }

impl Typing {
    pub fn new(ask: Ask) -> Self { Self { ask, line: Line::new(), old: Line::new(), new: Line::new() } }

    /// A key, as `secret::Line::key` takes it.
    pub fn key(&mut self, ch: Option<char>, escape: bool, enter: bool, backspace: bool) -> Next {
        match self.line.key(ch, escape, enter, backspace) {
            Step::Going => Next::Going,
            Step::Refused => { self.wipe(); Next::Cancelled }
            Step::Done => self.typed(),
        }
    }

    // A line ended with Enter.
    fn typed(&mut self) -> Next {
        match self.ask {
            Ask::Unlock => match length(self.line.bytes()) {
                Some(why) => { self.line.wipe(); Next::Ask(Ask::Unlock, Some(why)) }
                None => Next::Unlock,
            },
            Ask::Old => {
                // Empty when none is set; else one keystore may have sealed under.
                if let (false, Some(why)) = (self.line.bytes().is_empty(), length(self.line.bytes())) { self.line.wipe(); return Next::Ask(Ask::Old, Some(why)); }
                self.old.take(&mut self.line);
                self.ask = Ask::New;
                Next::Ask(Ask::New, None)
            }
            Ask::New => match length(self.line.bytes()) {
                Some(why) => { self.line.wipe(); Next::Ask(Ask::New, Some(why)) }
                None => { self.new.take(&mut self.line); self.ask = Ask::Again; Next::Ask(Ask::Again, None) }
            },
            Ask::Again => {
                let same = self.line.bytes() == self.new.bytes();
                self.line.wipe();
                if same { return Next::Set; }
                self.new.wipe();
                self.ask = Ask::New;
                Next::Ask(Ask::New, Some("(THE TWO DIFFER: THE NEW ONE AGAIN, TWICE)"))
            }
        }
    }

    /// Whether the one set was typed: a change, not a first passphrase.
    pub fn changes(&self) -> bool { !self.old.bytes().is_empty() }

    /// Every line zeroed.
    pub fn wipe(&mut self) { self.line.wipe(); self.old.wipe(); self.new.wipe(); }
}

/// keystore's refusals as the shell tells them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal { Denied, Budget, Invalid, NoKey, Other }

/// The wait in ms after `wrong` wrong passphrases in a row: none for the first two, then 1 s, doubling up to 60 s
/// (`keystore.wit` 1.6, as passlock::wait_ms counts it).
pub fn wait_ms(wrong: u32) -> u64 { if wrong <= 2 { 0 } else { (1000u64 << (wrong - 3).min(6)).min(60_000) } }

/// The wrong passphrases in a row keystore refused the shell, and when it takes the next try (uptime, ms).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tries { pub wrong: u32, pub next_ms: u64 }

impl Tries {
    /// keystore's `denied` at `now`: the line to say, the next try's time kept.
    pub fn denied(&mut self, now: u64) -> String {
        self.wrong = self.wrong.saturating_add(1);
        let wait = wait_ms(self.wrong);
        self.next_ms = now + wait;
        match wait {
            0 => format!("(WRONG: {} IN A ROW)", self.wrong),
            _ => format!("(WRONG: {} IN A ROW; keystore TAKES THE NEXT TRY IN {} S)", self.wrong, wait / 1000),
        }
    }
    /// keystore's `budget` at `now`: a try before its wait. keystore's count outlives the shell's, so a wait is kept
    /// here too when the shell counted fewer.
    pub fn budget(&mut self, now: u64) -> String {
        self.wrong = self.wrong.max(3);
        let wait = wait_ms(self.wrong).max(self.left(now));
        self.next_ms = now + wait;
        format!("(TOO SOON AFTER A WRONG ONE: keystore TAKES THE NEXT TRY IN {} S)", wait.div_ceil(1000))
    }
    pub fn opened(&mut self) { *self = Self::default(); }
    /// The ms before keystore takes the next try.
    pub fn left(&self, now: u64) -> u64 { self.next_ms.saturating_sub(now) }
}

/// What an `unlock` answered, said on the console (`denied` and `budget` are the tries' lines).
pub fn unlocked(answer: Result<(), Refusal>) -> &'static str {
    match answer {
        Ok(()) => "(OPENED: keystore HOLDS THE DEVICE KEY, AND THE SERVICES THAT WAITED FOR IT START)",
        Err(Refusal::Invalid) => "(NOTHING WAITS FOR A PASSPHRASE: keystore HOLDS THE DEVICE KEY, OR A TPM SEALS IT)",
        Err(Refusal::NoKey) => "(NOT OPENED: keystore'S SEALED FILE DOES NOT OPEN: THE SYSTEM LOG SAYS WHY)",
        Err(_) => "(NOT OPENED: keystore DID NOT ANSWER)",
    }
}

/// What a `passphrase-set` answered (`budget` is the tries' line), `changes` when the one set was typed.
pub fn set(answer: Result<(), Refusal>, changes: bool) -> &'static str {
    match (answer, changes) {
        (Ok(()), false) => "(SET: THE DEVICE KEY IS SEALED UNDER IT AND ITS UNENCRYPTED COPY REMOVED; EVERY BOOT WAITS FOR IT)",
        (Ok(()), true) => "(CHANGED: THE DEVICE KEY IS SEALED UNDER THE NEW ONE; THE OLD ONE OPENS IT NO MORE)",
        (Err(Refusal::Denied), false) => "(NOT SET: ONE IS SET ALREADY; TYPE IT AS THE ONE SET)",
        (Err(Refusal::Denied), true) => "(NOT CHANGED: THAT IS NOT THE ONE SET, OR NONE IS: THEN ENTER ALONE)",
        (Err(Refusal::Invalid), _) => "(NOT CHANGED: A TPM SEALS THE DEVICE KEY HERE)",
        (Err(Refusal::NoKey), _) => "(NOT CHANGED: keystore HOLDS NO DEVICE KEY, OR DID NOT WRITE THE SEALED FILE: THE SYSTEM LOG SAYS WHY)",
        (Err(_), _) => "(NOT CHANGED: keystore DID NOT ANSWER)",
    }
}
