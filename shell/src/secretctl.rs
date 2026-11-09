//! The system calls around `secret` (000-APP-0126, `shell.wit` 1.12): a console program's `secret-line` call held while
//! the user types the line on its console with no echo, then answered with the line's bytes, which are wiped at once.

use crate::secret::{self, Line, Step};
use crate::flowctl::say;
use crate::{is_window, Shell, COMMANDS_ASIDE, COMMANDS_RECEIVE, SESSIONS};
use core::fmt::Write;
use mind::abi::CAP_KIND_NONE;
use mind::idl::shell as commands;
use mind::idl::wire::Call;
use mind::input::{Code, Key};

/// The badge of the copy of the shell's client through which alone `secret-line` is answered: the value proposed to KRN
/// as `BADGE_SHELL_SECRET` (issues/requests-KRN.md); until that lands no copy carries it, and every call is refused.
pub const BADGE: u16 = 2;

/// The line being typed: the call held, its program, the session it is typed on, the slot its buffer came in, the bytes.
pub struct Typing { call: Option<Call>, pid: u64, pub session: usize, slot: usize, line: Line }

/// Whether a secret line is being typed on `session`.
pub fn waits_on(shell: &Shell, session: usize) -> bool { shell.secret.as_ref().is_some_and(|t| t.session == session) }

/// Where the next request of the shell's commands arrives: the slot the call held does not hold.
pub fn receive_slot(shell: &Shell) -> usize {
    if shell.secret.as_ref().is_some_and(|t| t.slot == COMMANDS_RECEIVE) { COMMANDS_ASIDE } else { COMMANDS_RECEIVE }
}

/// A `secret-line` call that came in `slot`: refused unless it came through the badged copy from the console program in
/// front of its session, busy while another line is typed; else the shell's prompt shown there and the call held.
pub fn ask(shell: &mut Shell, mut call: Call, sender: u64, badge: u16, slot: usize, words: &str) {
    let session = if badge == BADGE { console_session(shell, sender) } else { None };
    let Some(session) = session else {
        mind::println!("[SHELL] SECRET LINE FOR PID {} REFUSED: {}", sender, if badge == BADGE { "NOT THE CONSOLE PROGRAM IN FRONT" } else { "NOT ITS CLIENT" });
        let _ = commands::reply_secret_line(call, Err(commands::Error::Refused));
        return;
    };
    if shell.secret.is_some() { let _ = commands::reply_secret_line(call, Err(commands::Error::Busy)); return; }
    if call.defer().is_err() { let _ = commands::reply_secret_line(call, Err(commands::Error::Unavailable)); return; }
    let program = program_name(sender);
    let back = shell.active;
    shell.activate(session);
    // What the program printed before it asked comes first.
    shell.pump_console(sender);
    let text = secret::prompt(&program, sender, words);
    say(shell, |term| { if term.position().col != 0 { term.print_char(b'\n'); } let _ = write!(term, "{}", text); });
    shell.term.render(None);
    shell.activate(back);
    shell.secret = Some(Typing { call: Some(call), pid: sender, session, slot, line: Line::new() });
    mind::println!("[SHELL] SECRET LINE FOR {} (PID {}) ASKED ON {}", crate::flowask::shown(&program, 16), sender, where_(session));
}

/// A key on the session the line is typed on: never shown; Enter gives the line, Esc and Ctrl+C refuse it. True, or
/// false when its program had gone meanwhile and the key is the shell's as before.
pub fn key(shell: &mut Shell, key: Key) -> bool {
    if gone(shell).is_some() { tend(shell); return false; }
    let Some(typing) = shell.secret.as_mut() else { return false };
    let ch = if key.is_ctrl('c') { Some('\u{3}') } else { key.text() };
    match typing.line.key(ch, key.is_escape(), key.code() == Code::Enter, key.code() == Code::Backspace) {
        Step::Going => {}
        Step::Done => finish(shell, true, "GIVEN"),
        Step::Refused => finish(shell, false, "REFUSED"),
    }
    true
}

/// Each pass of the shell's loop: a line whose program ended, or gave up waiting, is taken down.
pub fn tend(shell: &mut Shell) {
    if let Some(why) = gone(shell) { finish(shell, false, why); }
}

// Why the line's caller no longer waits: its program ended, or it gave up and took its buffer back.
fn gone(shell: &Shell) -> Option<&'static str> {
    let typing = shell.secret.as_ref()?;
    if !mind::process::alive(typing.pid) { return Some("GONE: ITS PROGRAM ENDED"); }
    (mind::dev::cap_info(typing.slot).0 == CAP_KIND_NONE).then_some("GONE: ITS PROGRAM STOPPED WAITING")
}

// The line's end: its bytes to the caller when `give` and it still waits, else a refusal; the bytes wiped where they
// are kept, before the state goes (a move would leave a copy); a line on its session saying which, and the log's.
fn finish(shell: &mut Shell, give: bool, word: &str) {
    let Some(typing) = shell.secret.as_mut() else { return };
    let (pid, session) = (typing.pid, typing.session);
    let waits = mind::process::alive(pid) && mind::dev::cap_info(typing.slot).0 != CAP_KIND_NONE;
    if let Some(call) = typing.call.take() {
        let answer = if !waits { Err(commands::Error::Unavailable) } else if give { Ok(typing.line.bytes()) } else { Err(commands::Error::Declined) };
        let _ = commands::reply_secret_line(call, answer);
    }
    typing.line.wipe();
    shell.secret = None;
    let back = shell.active;
    if shell.activate(session) {
        say(shell, |term| { let _ = writeln!(term, "({})", word); });
        shell.term.render(None);
        shell.activate(back);
    }
    mind::println!("[SHELL] SECRET LINE FOR PID {} {}", pid, word);
}

// The open session whose console program is `pid`.
fn console_session(shell: &mut Shell, pid: u64) -> Option<usize> {
    if shell.console == Some(pid) { return Some(shell.active); }
    (0..SESSIONS).find(|&i| shell.parked[i].as_mut().is_some_and(|p| p.get().console == Some(pid)))
}

fn program_name(pid: u64) -> alloc::string::String {
    let (list, count) = crate::tasks();
    list[..count].iter().find(|t| t.pid == pid).map_or_else(|| alloc::string::String::from("?"), |t| alloc::string::String::from(crate::label(&t.name)))
}

fn where_(session: usize) -> alloc::string::String {
    if is_window(session) { alloc::string::String::from("THE SHELL'S WINDOW") } else { alloc::format!("CONSOLE {}", session + 1) }
}
