//! The system calls around `passphrase` (308-APP-0130, `keystore.wit` 1.6): keystore's client badged
//! BADGE_KEY_PASSPHRASE in SLOT_PASSPHRASE, the shell's alone (308-KRN-0205). The boot's wait for the passphrase asked
//! on console 1 in place of the first prompt; what is typed for it, or for `passphrase`, never shown, sent and wiped.

use crate::flowctl::say;
use crate::passphrase::{self, Ask, Next, Refusal, Tries, Typing};
use crate::{is_window, Shell};
use alloc::string::String;
use core::fmt::Write;
use mind::abi::{CAP_KIND_ENDPOINT, SLOT_PASSPHRASE};
use mind::idl::keystore::{self, Error, KeyState};
use mind::idl::wire;
use mind::input::{Code, Key};
use mind::ipc::Endpoint;

const KEYSTORE: Endpoint = Endpoint(SLOT_PASSPHRASE);
// key-state is answered at once; unlock and passphrase-set after Argon2id's derivation, seconds on a slow machine.
const STATE_MS: u32 = 2000;
const DERIVE_MS: u32 = 60_000;
// Where keystore did not answer at the shell's start, the boot's wait is looked for once a second, for a minute at most.
const LOOK_MS: u64 = 1000;
const LOOK_FOR_MS: u64 = 60_000;

/// The passphrase typed on a session: for what, since the boot or by `passphrase`, and when its prompt shows (after
/// keystore's wait).
pub struct Passing { pub session: usize, typing: Typing, boot: bool, due: Option<u64> }

/// The shell's part: what is typed, keystore's wrong ones in a row, and when the boot's wait is looked for next.
pub struct State { pub passing: Option<Passing>, tries: Tries, look: Option<u64>, look_until: u64 }

impl State {
    pub fn new(now: u64) -> Self { Self { passing: None, tries: Tries::default(), look: None, look_until: now + LOOK_FOR_MS } }
}

/// At the shell's start, before its first prompt: true when the boot waits for the passphrase and it is asked for in
/// that prompt's place. keystore starts before the shell, as init asked it about the services it prepared.
pub fn start(shell: &mut Shell) -> bool {
    if !held() { return false; }
    match state() {
        Some(KeyState::Waiting) => { asked(shell); true }
        Some(_) => false,
        None => { shell.pass.look = Some(0); false }
    }
}

// The boot's prompt on console 1, the boot's log no more followed there, which would push it away.
fn asked(shell: &mut Shell) {
    mind::println!("[SHELL] THE BOOT WAITS FOR keystore'S PASSPHRASE: ASKED ON CONSOLE 1");
    shell.log_next = None;
    begin(shell, 0, Ask::Unlock, true, Some(passphrase::WAITS));
}

fn held() -> bool { mind::dev::cap_info(SLOT_PASSPHRASE).0 == CAP_KIND_ENDPOINT }

fn state() -> Option<KeyState> { wire::with_timeout(STATE_MS, || keystore::key_state(KEYSTORE)).ok()?.ok() }

fn refusal(error: Error) -> Refusal {
    match error { Error::Denied => Refusal::Denied, Error::Budget => Refusal::Budget, Error::Invalid => Refusal::Invalid, Error::NoKey => Refusal::NoKey, _ => Refusal::Other }
}

/// Whether the passphrase is typed on `session`.
pub fn waits_on(shell: &Shell, session: usize) -> bool { shell.pass.passing.as_ref().is_some_and(|p| p.session == session) }

/// Whether the boot's passphrase is still asked: the start script waits for it, as what it starts may need the key.
pub fn boot_asks(shell: &Shell) -> bool { shell.pass.look.is_some() || shell.pass.passing.as_ref().is_some_and(|p| p.boot) }

/// Each pass of the shell's loop: the boot's wait looked for, and a prompt shown once keystore's wait has passed.
pub fn tend(shell: &mut Shell, now: u64) {
    if let Some(due) = shell.pass.passing.as_ref().and_then(|p| p.due) {
        if now >= due { let session = shell.pass.passing.as_ref().map_or(0, |p| p.session); show(shell, session, None); }
    }
    let Some(look) = shell.pass.look.filter(|&look| now >= look) else { return };
    if !held() { shell.pass.look = None; return; }
    match state() {
        Some(KeyState::Waiting) => { shell.pass.look = None; if shell.pass.passing.is_none() { asked(shell); } }
        Some(_) => shell.pass.look = None,
        None if now >= shell.pass.look_until => { shell.pass.look = None; mind::println!("[SHELL] keystore DID NOT SAY WHETHER THE BOOT WAITS FOR ITS PASSPHRASE"); }
        None => shell.pass.look = Some(look.max(now) + LOOK_MS),
    }
}

/// `passphrase` typed on the active session: the boot's wait answered, or one set or changed.
pub fn command(shell: &mut Shell, args: &[u8]) {
    let fail = |shell: &mut Shell, why: &str| { let _ = writeln!(shell.term, "ERROR: {}", why); };
    if !args.iter().all(u8::is_ascii_whitespace) { return fail(shell, passphrase::USAGE); }
    if shell.script.is_some() || shell.redirecting || shell.term.capture.is_some() { return fail(shell, passphrase::NOT_HERE); }
    if !held() { return fail(shell, passphrase::NO_CLIENT); }
    if let Some(at) = shell.pass.passing.as_ref().map(|p| p.session) { return fail(shell, &alloc::format!("PASSPHRASE: ONE IS TYPED ON {} NOW", where_(at))); }
    match state() {
        Some(KeyState::Waiting) => begin(shell, shell.active, Ask::Unlock, false, None),
        Some(KeyState::Held) => begin(shell, shell.active, Ask::Old, false, None),
        Some(KeyState::None) => fail(shell, passphrase::NO_KEY),
        None => fail(shell, passphrase::NO_ANSWER),
    }
}

// The typing on `session` begun, its prompt shown at once unless keystore waits (a line above it when given).
fn begin(shell: &mut Shell, session: usize, ask: Ask, boot: bool, above: Option<&str>) {
    let left = if ask == Ask::Unlock { shell.pass.tries.left(mind::time::uptime_ms() as u64) } else { 0 };
    shell.pass.passing = Some(Passing { session, typing: Typing::new(ask), boot, due: None });
    let back = shell.active;
    if shell.activate(session) {
        say(shell, |term| { if term.position().col != 0 { term.print_char(b'\n'); } if let Some(line) = above { let _ = writeln!(term, "{}", line); } });
        shell.activate(back);
    }
    if left > 0 { wait(shell, left, None) } else { show(shell, session, None) }
}

// The prompt of what is typed next, after `why` when given.
fn show(shell: &mut Shell, session: usize, why: Option<&str>) {
    let Some(passing) = shell.pass.passing.as_mut() else { return };
    passing.due = None;
    let text = passphrase::prompt(passing.typing.ask);
    let back = shell.active;
    if shell.activate(session) {
        say(shell, |term| {
            if let Some(why) = why { let _ = writeln!(term, "{}", why); }
            if term.position().col != 0 { term.print_char(b'\n'); }
            let _ = write!(term, "{}", text);
        });
        shell.term.render(None);
        shell.activate(back);
    }
}

// keystore's wait before the next prompt, `why` said at once.
fn wait(shell: &mut Shell, ms: u64, why: Option<&str>) {
    let Some(passing) = shell.pass.passing.as_mut() else { return };
    passing.due = Some(mind::time::uptime_ms() as u64 + ms);
    let session = passing.session;
    if let Some(why) = why { say_on(shell, session, why); }
}

fn say_on(shell: &mut Shell, session: usize, line: &str) {
    let back = shell.active;
    if shell.activate(session) {
        say(shell, |term| { if term.position().col != 0 { term.print_char(b'\n'); } let _ = writeln!(term, "{}", line); });
        shell.term.render(None);
        shell.activate(back);
    }
}

// The typing ended: `line` said, the lines wiped, the shell's prompt back.
fn end(shell: &mut Shell, line: &str) {
    let Some(mut passing) = shell.pass.passing.take() else { return };
    passing.typing.wipe();
    let back = shell.active;
    if shell.activate(passing.session) {
        say(shell, |term| { if term.position().col != 0 { term.print_char(b'\n'); } let _ = writeln!(term, "{}", line); });
        shell.prompt();
        shell.term.render(None);
        shell.activate(back);
    }
}

/// A key on the session the passphrase is typed on, never shown: true, as the key is taken.
pub fn key(shell: &mut Shell, key: Key) -> bool {
    let Some(passing) = shell.pass.passing.as_mut() else { return false };
    // Until its prompt shows, keystore takes no try: the keys go nowhere, but Esc.
    if passing.due.is_some() && !key.is_escape() && !key.is_ctrl('c') { return true; }
    let ch = if key.is_ctrl('c') { Some('\u{3}') } else { key.text() };
    let (session, boot) = (passing.session, passing.boot);
    match passing.typing.key(ch, key.is_escape(), key.code() == Code::Enter, key.code() == Code::Backspace) {
        Next::Going => {}
        Next::Ask(_, why) => show(shell, session, why),
        Next::Cancelled => {
            mind::println!("[SHELL] keystore'S PASSPHRASE NOT TYPED: {}", if boot { "LATER" } else { "NOTHING CHANGED" });
            end(shell, if boot { passphrase::LATER } else { passphrase::KEPT });
        }
        Next::Unlock => unlock(shell, session),
        Next::Set => set(shell, session),
    }
    true
}

// What was typed sent to `unlock`, then wiped.
fn unlock(shell: &mut Shell, session: usize) {
    say_on(shell, session, passphrase::DERIVING);
    let Some(passing) = shell.pass.passing.as_mut() else { return };
    let answer = match core::str::from_utf8(passing.typing.line.bytes()) {
        Ok(text) => wire::with_timeout(DERIVE_MS, || keystore::unlock(KEYSTORE, text)).map(|r| r.map_err(refusal)).unwrap_or(Err(Refusal::Other)),
        Err(_) => Err(Refusal::Denied),
    };
    passing.typing.wipe();
    let now = mind::time::uptime_ms() as u64;
    match answer {
        Ok(()) => {
            shell.pass.tries.opened();
            mind::println!("[SHELL] keystore'S PASSPHRASE TYPED ON {}: THE DEVICE KEY OPENED", where_(session));
            end(shell, passphrase::unlocked(Ok(())));
        }
        Err(Refusal::Denied) | Err(Refusal::Budget) => {
            let line = if answer == Err(Refusal::Denied) { shell.pass.tries.denied(now) } else { shell.pass.tries.budget(now) };
            mind::println!("[SHELL] keystore'S PASSPHRASE REFUSED: {}", line.trim_matches(|c| c == '(' || c == ')'));
            let left = shell.pass.tries.left(now);
            if left > 0 { wait(shell, left, Some(&line)) } else { show(shell, session, Some(&line)) }
        }
        Err(refused) => {
            mind::println!("[SHELL] keystore'S PASSPHRASE NOT TAKEN: {:?}", refused);
            end(shell, passphrase::unlocked(Err(refused)));
        }
    }
}

// The one set and the new one sent to `passphrase-set`, then wiped.
fn set(shell: &mut Shell, session: usize) {
    say_on(shell, session, passphrase::DERIVING);
    let Some(passing) = shell.pass.passing.as_mut() else { return };
    let changes = passing.typing.changes();
    let answer = match (core::str::from_utf8(passing.typing.old.bytes()), core::str::from_utf8(passing.typing.new.bytes())) {
        (Ok(old), Ok(new)) => wire::with_timeout(DERIVE_MS, || keystore::passphrase_set(KEYSTORE, old, new)).map(|r| r.map_err(refusal)).unwrap_or(Err(Refusal::Other)),
        _ => Err(Refusal::Denied),
    };
    passing.typing.wipe();
    let now = mind::time::uptime_ms() as u64;
    let mut line = String::from(passphrase::set(answer, changes));
    match answer {
        Ok(()) => { shell.pass.tries.opened(); mind::println!("[SHELL] keystore'S PASSPHRASE {} ON {}", if changes { "CHANGED" } else { "SET" }, where_(session)); }
        // A wrong one set counts with the boot's (keystore.wit 1.6); an empty one where one is set does not.
        Err(Refusal::Denied) if changes => { let tries = shell.pass.tries.denied(now); line.push(' '); line.push_str(&tries); mind::println!("[SHELL] keystore'S PASSPHRASE NOT CHANGED: {}", tries.trim_matches(|c| c == '(' || c == ')')); }
        Err(Refusal::Budget) => { line = shell.pass.tries.budget(now); mind::println!("[SHELL] keystore'S PASSPHRASE NOT CHANGED: TOO SOON AFTER A WRONG ONE"); }
        Err(refused) => mind::println!("[SHELL] keystore'S PASSPHRASE NOT CHANGED: {:?}", refused),
    }
    end(shell, &line);
}

fn where_(session: usize) -> String {
    if is_window(session) { String::from("THE SHELL'S WINDOW") } else { alloc::format!("CONSOLE {}", session + 1) }
}
