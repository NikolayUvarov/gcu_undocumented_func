//! MIND Hub's requests that wait for the user, said above the shell's prompt (100003-APP-0153): `improve` keeps them in
//! data/improve/waiting; the shell says them above its first prompt, and above a later one on the console shown when
//! they change and something waits, once no program is in front there and nothing is asked.

use crate::flowctl::say;
use crate::{passctl, Shell};
use alloc::vec::Vec;
use core::fmt::Write;

#[path = "../../improve/app/src/waiting.rs"]
mod waiting;

/// How often the file is read again.
const EVERY_MS: u64 = 5_000;

/// The numbers last said, or read at the start, and when the file is read next.
pub struct Seen { ids: Option<Vec<u64>>, due: u64 }

impl Seen { pub const fn new() -> Seen { Seen { ids: None, due: 0 } } }

// The file's numbers now; None when it is missing or not of its format.
fn read() -> Option<Vec<u64>> {
    let file = mind::fs::File::open(waiting::FILE).ok()?;
    let mut buffer = [0u8; waiting::BYTES];
    let n = file.read_at(0, &mut buffer).ok()?;
    waiting::read(core::str::from_utf8(&buffer[..n]).ok()?)
}

/// Above the first prompt: what waited when `improve` last looked.
pub fn first(shell: &mut Shell) {
    let ids = read().unwrap_or_default();
    if let Some(line) = waiting::notice(&ids) { let _ = writeln!(shell.term, "{}", line); mind::println!("[SHELL] {}", line); }
    shell.improve.ids = Some(ids);
    shell.improve.due = mind::time::uptime_ms() as u64 + EVERY_MS;
}

/// Each EVERY_MS: a change said on the console shown, above its prompt.
pub fn tend(shell: &mut Shell, now: u64) {
    if now < shell.improve.due { return; }
    shell.improve.due = now + EVERY_MS;
    let Some(ids) = read() else { return };
    if shell.improve.ids.as_ref() == Some(&ids) { return; }
    let back = shell.active;
    shell.activate(shell.shown);
    // Not over a program in front, a script, a question or a secret line: the next look says it.
    let busy = shell.focused.is_some() || shell.script.is_some() || shell.asking.is_some() || shell.secret.is_some() || passctl::boot_asks(shell);
    if !busy {
        let line = waiting::notice(&ids);
        shell.improve.ids = Some(ids);
        if let Some(line) = line {
            mind::println!("[SHELL] {}", line);
            say(shell, |term| { if term.position().col != 0 { term.print_char(b'\n'); } let _ = writeln!(term, "{}", line); });
            shell.prompt();
            shell.term.render(None);
        }
    }
    shell.activate(back);
}
