//! `mode`'s calls (750-EFF-0013): init's modes through the shell's client badged `BADGE_MODE` in `SLOT_MODE` (`init.wit`
//! 1.5), and the owner's permissions `remote-update` and `link-remote-update` (1.11, 801-APP-0139); the command line and
//! the text are `modes.rs`'.
use crate::modes::{self, Asked, Which};
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};
use mind::idl::init::{self, Error, Mode};

// Whether the booted slot trusts the public test key (the launch record's `test_key`), for the notice.
static TEST_KEY: AtomicBool = AtomicBool::new(false);
pub fn launched(test_key: bool) { TEST_KEY.store(test_key, Ordering::Relaxed); }

fn error(error: Error, which: Option<Which>) -> &'static str {
    match error {
        Error::Denied if which == Some(Which::RemoteUpdate) => "INIT REFUSED IT: REMOTE-UPDATE COUNTS ONLY WITH THE DEVELOPMENT OR MAINTENANCE MODE ON",
        Error::Denied if which == Some(Which::LinkRemoteUpdate) => "INIT REFUSED IT: LINK-REMOTE-UPDATE COUNTS ONLY WHILE THE LINK'S CONSOLE IS ON",
        Error::Denied => "INIT REFUSED IT: THIS CLIENT MAY NOT CHANGE THE MODE",
        Error::Failed => "INIT COULD NOT WRITE DATA/MODE.CONF OR START THE SERVICES",
        Error::NoDevice => "NO BOOT VOLUME FOR DATA/MODE.CONF",
        Error::NotFound | Error::Running | Error::Stopped => "INIT ANSWERED AN ERROR",
    }
}

// `mode [development|maintenance on|off [reason]]`: a mode goes on after the operator agrees at this console, off at once;
// then the modes as init now has them.
pub fn command(shell: &mut crate::Shell, args: &[u8]) {
    let asked = match modes::parse(core::str::from_utf8(args).unwrap_or("")) { Ok(asked) => asked, Err(usage) => { let _ = writeln!(shell.term, "{}", usage); return } };
    if mind::dev::cap_info(mind::abi::SLOT_MODE).0 != mind::abi::CAP_KIND_ENDPOINT {
        let _ = writeln!(shell.term, "MODE: THIS SHELL HOLDS NO MODE CLIENT (SLOT_MODE, INIT.WIT 1.5)");
        return;
    }
    let client = mind::ipc::Endpoint(mind::abi::SLOT_MODE);
    if let Asked::Set { which, on, reason } = asked {
        let mut why = alloc::string::String::from(if reason.is_empty() { "operator" } else { reason });
        if shell.unasked {
            // The link's console may switch remote-update while init reports the owner's link-remote-update (751-KRN-0185);
            // no other mode from the link, its console's script either (800-APP-0108, MC-12.6).
            let leave = shell.console_link && matches!(init::mode(client), Ok(Ok(state)) if state.link_remote_update);
            if !modes::link_may(&asked, leave) {
                let _ = writeln!(shell.term, "MODE: NOT CHANGED: {}", if shell.console_link && which == Which::RemoteUpdate {
                    "THE OWNER'S LINK-REMOTE-UPDATE IS OFF, SO THE LINK'S CONSOLE MAY NOT SWITCH REMOTE-UPDATE" } else { "A LINE OF THE LINK MAY NOT CHANGE THE MODE" });
                return;
            }
            // The link named as the source in init's log line, within set-mode's 120 bytes.
            why = alloc::format!("the link's console{}{}", if reason.is_empty() { "" } else { ": " }, reason);
            let mut end = why.len().min(modes::REASON_MAX);
            while !why.is_char_boundary(end) { end -= 1; }
            why.truncate(end);
        } else if on {
            let mut text = alloc::string::String::new();
            modes::question(&mut text, which);
            if !crate::msh::ask(shell, &text) { let _ = writeln!(shell.term, "MODE: NOT CHANGED"); return; }
        }
        let mode = match which { Which::Development => Mode::Development, Which::Maintenance => Mode::Maintenance, Which::RemoteUpdate => Mode::RemoteUpdate, Which::LinkRemoteUpdate => Mode::LinkRemoteUpdate };
        match init::set_mode(client, mode, on, &why) {
            Ok(Ok(())) => { let _ = writeln!(shell.term, "MODE {} {}", modes::name(which), if on { "ON" } else { "OFF" }); }
            Ok(Err(e)) => { let _ = writeln!(shell.term, "MODE: {}", error(e, Some(which))); return; }
            Err(_) => { let _ = writeln!(shell.term, "MODE: INIT DOES NOT ANSWER"); return; }
        }
    }
    match init::mode(client) {
        Ok(Ok(state)) => {
            modes::line(&mut shell.term, state.development, state.maintenance, state.by.as_str(), state.reason.as_str());
            modes::permissions(&mut shell.term, state.remote_update, state.link_remote_update);
        }
        Ok(Err(e)) => { let _ = writeln!(shell.term, "MODE: {}", error(e, None)); }
        Err(_) => { let _ = writeln!(shell.term, "MODE: INIT DOES NOT ANSWER"); }
    }
    if crate::link_console() { let _ = writeln!(shell.term, "{}", LINK_CONSOLE); }
}

// The `link-console` mode (init.wit 1.8, 800-APP-0108): only data/mode.conf sets it, so `mode` shows it and never changes
// it; the link's console switches remote-update alone, with the owner's link-remote-update (801-APP-0139, 0141).
const LINK_CONSOLE: &str = "LINK CONSOLE: ON (DATA/MODE.CONF): THE LINK'S LINES ARE THE CONSOLE'S, BUT NO MODE: REMOTE-UPDATE ALONE, WITH THE OWNER'S LINK-REMOTE-UPDATE";

// The modes, and the link's console, above the first prompt and in the system log while one is on. No timeout: a call given up revokes its lent
// buffer under init's late answer (the fault of LNK's report, 2026-10-22), and init answers `mode` at once.
pub fn notice(shell: &mut crate::Shell) {
    if crate::link_console() {
        let _ = writeln!(shell.term, "{}", LINK_CONSOLE);
        mind::println!("[SHELL] {}", LINK_CONSOLE);
    }
    if mind::dev::cap_info(mind::abi::SLOT_MODE).0 != mind::abi::CAP_KIND_ENDPOINT { return; }
    let Ok(Ok(state)) = init::mode(mind::ipc::Endpoint(mind::abi::SLOT_MODE)) else { return };
    let mut text = alloc::string::String::new();
    let mode_on = modes::notice(&mut text, state.development, state.maintenance, state.by.as_str());
    if modes::remote_notice(&mut text, state.remote_update, state.link_remote_update, TEST_KEY.load(Ordering::Relaxed)) || mode_on {
        let _ = write!(shell.term, "{}", text);
        for line in text.lines() { mind::println!("[SHELL] {}", line); }
    }
}
