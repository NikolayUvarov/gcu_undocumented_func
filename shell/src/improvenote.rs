//! MIND Hub's requests that wait for the user, said above the shell's prompt (100003-APP-0153): `improve` keeps them in
//! data/improve/waiting; the shell says them above its first prompt, and above a later one on the console shown when
//! they change and something waits, once no program is in front there and nothing is asked. It reads the file while a
//! program runs too, and `change` tells it of a write it missed (a set gone and back between two looks).

use crate::flowctl::say;
use crate::{passctl, Shell};
use core::fmt::Write;

// The shell reads the file and says its line; the rest of it is improve's and wm's.
#[allow(dead_code)]
#[path = "../../improve/app/src/waiting.rs"]
mod waiting;

/// How often the file is read again, and how soon again while a change waits to be said.
const EVERY_MS: u64 = 5_000;
const WAIT_MS: u64 = 500;

/// What the file said when last read, whether a change of it waits to be said, and when the file is read next.
pub struct Seen { last: Option<waiting::Waiting>, unsaid: bool, due: u64 }

impl Seen { pub const fn new() -> Seen { Seen { last: None, unsaid: false, due: 0 } } }

// The file now; None when it is missing or not of its format.
fn read() -> Option<waiting::Waiting> {
    let file = mind::fs::File::open(waiting::FILE).ok()?;
    let mut buffer = [0u8; waiting::BYTES];
    let n = file.read_at(0, &mut buffer).ok()?;
    waiting::read(core::str::from_utf8(&buffer[..n]).ok()?)
}

/// Above the first prompt: what waited when `improve` last looked.
pub fn first(shell: &mut Shell) {
    let w = read().unwrap_or_default();
    if let Some(line) = waiting::notice(&w.ids) { let _ = writeln!(shell.term, "{}", line); mind::println!("[SHELL] {}", line); }
    shell.improve.last = Some(w);
    shell.improve.due = mind::time::uptime_ms() as u64 + EVERY_MS;
}

/// Each EVERY_MS: the file read, whatever runs, and a change with something waiting said on the console shown, above
/// its prompt, once nothing is in the way.
pub fn tend(shell: &mut Shell, now: u64) {
    if now < shell.improve.due { return; }
    shell.improve.due = now + EVERY_MS;
    if let Some(w) = read() {
        if shell.improve.last.as_ref() != Some(&w) {
            mind::println!("[SHELL] IMPROVE: data/improve/waiting CHANGE {}: {} REQUESTS WAIT", w.change, w.ids.len());
            shell.improve.unsaid = !w.ids.is_empty();
            shell.improve.last = Some(w);
        }
    }
    if !shell.improve.unsaid { return; }
    let back = shell.active;
    shell.activate(shell.shown);
    // Not over a program in front, a script, a question or a secret line: looked at again soon.
    let busy = shell.focused.is_some() || shell.script.is_some() || shell.asking.is_some() || shell.secret.is_some() || passctl::boot_asks(shell);
    if busy {
        shell.improve.due = now + WAIT_MS;
    } else if let Some(line) = shell.improve.last.as_ref().and_then(|w| waiting::notice(&w.ids)) {
        shell.improve.unsaid = false;
        mind::println!("[SHELL] {}", line);
        say(shell, |term| { if term.position().col != 0 { term.print_char(b'\n'); } let _ = writeln!(term, "{}", line); });
        shell.prompt();
        shell.term.render(None);
    }
    shell.activate(back);
}
