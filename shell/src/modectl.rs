//! `mode`'s calls (750-EFF-0013): init's modes through the shell's client badged `BADGE_MODE` in `SLOT_MODE` (`init.wit`
//! 1.5); the command line and the text are `modes.rs`'.
use crate::modes::{self, Asked, Which};
use core::fmt::Write;
use mind::idl::init::{self, Error, Mode};

fn error(error: Error) -> &'static str {
    match error {
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
        if on {
            let mut text = alloc::string::String::new();
            modes::question(&mut text, which);
            if !crate::msh::ask(shell, &text) { let _ = writeln!(shell.term, "MODE: NOT CHANGED"); return; }
        }
        let mode = match which { Which::Development => Mode::Development, Which::Maintenance => Mode::Maintenance };
        match init::set_mode(client, mode, on, if reason.is_empty() { "operator" } else { reason }) {
            Ok(Ok(())) => { let _ = writeln!(shell.term, "MODE {} {}", modes::name(which), if on { "ON" } else { "OFF" }); }
            Ok(Err(e)) => { let _ = writeln!(shell.term, "MODE: {}", error(e)); return; }
            Err(_) => { let _ = writeln!(shell.term, "MODE: INIT DOES NOT ANSWER"); return; }
        }
    }
    match init::mode(client) {
        Ok(Ok(state)) => modes::line(&mut shell.term, state.development, state.maintenance, state.by.as_str(), state.reason.as_str()),
        Ok(Err(e)) => { let _ = writeln!(shell.term, "MODE: {}", error(e)); }
        Err(_) => { let _ = writeln!(shell.term, "MODE: INIT DOES NOT ANSWER"); }
    }
}

// The modes above the first prompt, and in the system log, while one is on. No timeout: a call given up revokes its lent
// buffer under init's late answer (the fault of LNK's report, 2026-10-22), and init answers `mode` at once.
pub fn notice(shell: &mut crate::Shell) {
    if mind::dev::cap_info(mind::abi::SLOT_MODE).0 != mind::abi::CAP_KIND_ENDPOINT { return; }
    let Ok(Ok(state)) = init::mode(mind::ipc::Endpoint(mind::abi::SLOT_MODE)) else { return };
    let mut text = alloc::string::String::new();
    if modes::notice(&mut text, state.development, state.maintenance, state.by.as_str()) {
        let _ = write!(shell.term, "{}", text);
        mind::print!("[SHELL] {}", text);
    }
}
