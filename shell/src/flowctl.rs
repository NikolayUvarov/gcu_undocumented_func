//! The system calls around `flowask` (000-APP-0120, `netpolicy.wit` 1.3): the broker's questions looked for in the
//! shell's loop, the oldest put to the user where its program was started, and the key's answer given back.

use crate::flowask::{self, Answer};
use crate::{is_window, Shell};
use alloc::string::String;
use core::fmt::Write;
use mind::abi::SLOT_NETPOLICY;
use mind::control;
use mind::idl::{netpolicy, wire};
use mind::input::Key;
use mind::ipc::Endpoint;

const BROKER: Endpoint = Endpoint(SLOT_NETPOLICY);
// How long a call to the broker may take from the shell's loop, in ms.
const CALL_MS: u32 = 200;

/// The question shown: its number, its program, the session it is on, when it showed, and the program the shell took
/// the keys from.
pub struct Asking { pub id: u32, pid: u64, pub session: usize, since: u64, held: Option<u64> }

/// Whether the question shown waits for a key on `session`.
pub fn waits_on(shell: &Shell, session: usize) -> bool { shell.asking.as_ref().is_some_and(|a| a.session == session) }

/// Each `POLL_MS`: a question the broker dropped meanwhile is taken down; with none shown, the oldest is put to the user.
pub fn tend(shell: &mut Shell, now: u64) {
    if now < shell.flow_due || mind::dev::cap_info(SLOT_NETPOLICY).0 == 0 { return; }
    let list = match wire::with_timeout(CALL_MS, || netpolicy::questions(BROKER)) {
        Ok(Ok(list)) => list,
        _ => { shell.flow_due = now + flowask::QUIET_MS; return; }
    };
    shell.flow_due = now + flowask::POLL_MS;
    let questions = list.as_slice();
    if let Some(id) = shell.asking.as_ref().map(|a| a.id) {
        // Still asked and its program alive: it waits. Else the broker dropped it, or will within a second.
        if questions.iter().any(|q| q.id == id) && !gone(shell) { return; }
        finish(shell, "(GONE: ITS PROGRAM ENDED, OR NO ANSWER CAME WITHIN 60 S AND THE BROKER REFUSED IT)");
        return;
    }
    // The oldest whose program still runs: the broker drops an ended one's within a second.
    let Some(q) = questions.iter().find(|q| mind::process::alive(q.pid)) else { return };
    // Where its program was started; a program the shell did not start, beside a window manager the shell started,
    // in the shell's window there, which the user sees; else on the console shown.
    let wm = shell.managers.iter().any(|&pid| mind::process::alive(pid));
    let session = match shell.owner(q.pid).filter(|&s| shell.is_open(s)) {
        Some(session) => session,
        None if wm => shell.open_window(false).map_or(shell.shown, |(session, _)| session),
        None => shell.shown,
    };
    let back = shell.active;
    shell.activate(session);
    let text = flowask::question(q.program.as_str(), q.pid, q.host.as_str(), q.port, q.why.as_str());
    say(shell, |term| { if term.position().col != 0 { term.print_char(b'\n'); } let _ = write!(term, "{}", text); });
    shell.term.render(None);
    // A program in front of the console shown has the keys: the shell takes them for the answer and gives them back.
    let held = shell.focused.filter(|_| session == shell.shown && !is_window(session));
    if held.is_some() { let _ = control::focus(shell.own, true); }
    shell.asking = Some(Asking { id: q.id, pid: q.pid, session, since: mind::time::uptime_ms() as u64, held });
    shell.activate(back);
    mind::println!("[SHELL] NETWORK QUESTION {}: {} (PID {}) ASKS FOR {}:{}", q.id, flowask::shown(q.program.as_str(), 16), q.pid, flowask::shown(q.host.as_str(), 253), q.port);
    if session != 0 { shell.note(format_args!("A NETWORK QUESTION WAITS ON {}", where_(session))); }
}

/// A key on the session the question is on: N, O, S, A, or Esc and Enter for no, answer it, and any other is held
/// back while it shows; true. False when its program has ended meanwhile: the question is taken down and the key is
/// the shell's as before.
pub fn key(shell: &mut Shell, key: Key) -> bool { respond(shell, key.char(), key.is_escape(), key.code() == mind::input::Code::Enter) }

/// A byte of the serial line, in a script's wait (000-APP-0125): as `key`, a byte no key gives held back.
pub fn byte(shell: &mut Shell, byte: u8) -> bool {
    match flowask::byte_key(byte) { Some((ch, escape, enter)) => respond(shell, ch, escape, enter), None => !gone(shell) && shell.asking.is_some() }
}

// The key's character, or Esc or Enter, given to the question: `key`'s answer.
fn respond(shell: &mut Shell, ch: Option<char>, escape: bool, enter: bool) -> bool {
    if gone(shell) { finish(shell, "(GONE: ITS PROGRAM ENDED)"); return false; }
    let Some((id, since)) = shell.asking.as_ref().map(|a| (a.id, a.since)) else { return false };
    if (mind::time::uptime_ms() as u64) < since + flowask::GUARD_MS { return true; }
    let Some(answer) = flowask::answer(ch, escape, enter) else { return true };
    let choice = match answer { Answer::No => netpolicy::Choice::No, Answer::Once => netpolicy::Choice::Once, Answer::Session => netpolicy::Choice::Session, Answer::Always => netpolicy::Choice::Always };
    let line = match wire::with_timeout(CALL_MS * 5, || netpolicy::answer(BROKER, id, choice)) {
        Ok(Ok(())) => alloc::format!("{}", flowask::word(answer)),
        Ok(Err(netpolicy::Error::NotFound)) => alloc::format!("{}: THE QUESTION HAD GONE (ITS PROGRAM ENDED, OR 60 S PASSED)", flowask::word(answer)),
        Ok(Err(netpolicy::Error::NoNetwork)) => alloc::format!("{}: THE HOST COULD NOT BE RESOLVED, SO THE PROGRAM WAS REFUSED", flowask::word(answer)),
        Ok(Err(netpolicy::Error::Limit)) => alloc::format!("{}: ITS GRANT HOLDS 16 RULES ALREADY, SO IT WAS REFUSED", flowask::word(answer)),
        Ok(Err(netpolicy::Error::Unwritable | netpolicy::Error::Invalid)) if answer == Answer::Always =>
            String::from("ALWAYS: ALLOWED FOR THIS RUN, BUT THE LINE COULD NOT BE KEPT IN THE POLICY"),
        Ok(Err(error)) => alloc::format!("{}: THE BROKER REFUSED THE ANSWER ({:?})", flowask::word(answer), error),
        Err(_) => alloc::format!("{}: THE BROKER DID NOT ANSWER", flowask::word(answer)),
    };
    mind::println!("[SHELL] NETWORK QUESTION {} ANSWERED {}", id, flowask::word(answer));
    finish(shell, &line);
    true
}

// Whether the question shown is of a program that has ended: the broker drops it then, a second later at most.
fn gone(shell: &Shell) -> bool { shell.asking.as_ref().is_some_and(|a| !mind::process::alive(a.pid)) }

// The question's end on its session: the line, the keys back to the program that had them, the prompt.
fn finish(shell: &mut Shell, line: &str) {
    let Some(asking) = shell.asking.take() else { return };
    let back = shell.active;
    shell.activate(asking.session);
    say(shell, |term| { let _ = writeln!(term, "{}", line); });
    if let Some(pid) = asking.held.filter(|&pid| mind::process::alive(pid) && asking.session == shell.shown) { let _ = control::focus(pid, true); }
    if shell.focused.is_none() && shell.console.is_none() { shell.prompt(); }
    shell.term.render(None);
    shell.activate(back);
}

// Lines on the console itself, not into what a script running there keeps of its program's output (000-APP-0125):
// the question is the user's, not the program's.
pub(crate) fn say(shell: &mut Shell, print: impl FnOnce(&mut crate::console::Console)) {
    let kept = shell.term.capture.take();
    print(&mut shell.term);
    shell.term.capture = kept;
}

fn where_(session: usize) -> String {
    if is_window(session) { String::from("THE SHELL'S WINDOW") } else { alloc::format!("CONSOLE {} (CTRL+ALT+F{})", session + 1, session + 1) }
}
