//! `update`'s calls (351-APP-0029): the updater through the shell's client in `SLOT_UPDATE` (ABI 12, 351-KRN-0116), which
//! no program gets; the command line and the text are `updates.rs`'.
use crate::updates::{self, Asked, View};
use core::fmt::Write;
use mind::idl::update::{self, State};
use mind::ipc::Endpoint;

fn view(state: &State) -> View<'_> {
    View { slot: state.slot, trial: state.trial, running: state.running, recorded: state.recorded, confirmed: state.confirmed, staged: state.staged, failure: state.failure.as_str() }
}

fn failed(shell: &mut crate::Shell, error: update::Error) { let _ = writeln!(shell.term, "UPDATE: {}", updates::error(error as usize)); }

fn silent(shell: &mut crate::Shell) { let _ = writeln!(shell.term, "UPDATE: THE UPDATER DOES NOT ANSWER"); }

// `update [status|check|fetch|apply [tries]|rollback]`: apply and rollback restart the machine after the operator agrees
// at this console; the others only read, or stage into the slot that did not boot.
pub fn command(shell: &mut crate::Shell, args: &[u8]) {
    let asked = match updates::parse(core::str::from_utf8(args).unwrap_or("")) { Ok(asked) => asked, Err(usage) => { let _ = writeln!(shell.term, "{}", usage); return } };
    if mind::dev::cap_info(mind::abi::SLOT_UPDATE).0 != mind::abi::CAP_KIND_ENDPOINT {
        let _ = writeln!(shell.term, "UPDATE: THIS SHELL HOLDS NO CLIENT OF THE UPDATER (SLOT_UPDATE): THIS BUILD HAS NO UPDATER");
        return;
    }
    let updater = Endpoint(mind::abi::SLOT_UPDATE);
    match asked {
        Asked::Status => {
            match update::status(updater) { Ok(state) => updates::state(&mut shell.term, &view(&state)), Err(_) => return silent(shell) }
            match update::metadata(updater) {
                Ok(held) => updates::channel(&mut shell.term, held.channel.as_str(), held.version, held.minimum, held.expires.as_str(), held.expired, held.checked),
                Err(_) => silent(shell),
            }
        }
        Asked::Check => match update::check(updater) {
            Ok(Ok(offer)) => updates::offer(&mut shell.term, offer.channel.as_str(), offer.version, offer.minimum, offer.expires.as_str(), offer.running),
            Ok(Err(error)) => failed(shell, error),
            Err(_) => silent(shell),
        },
        Asked::Fetch => {
            // The updater answers when every file is staged: the shell waits for it.
            let _ = writeln!(shell.term, "UPDATE: FETCHING INTO THE OTHER SLOT; THE SHELL WAITS FOR THE UPDATER");
            shell.term.render(None);
            match update::fetch(updater) {
                Ok(Ok(version)) => { let _ = writeln!(shell.term, "UPDATE: VERSION {} STAGED IN THE OTHER SLOT; UPDATE APPLY BOOTS IT ON TRIAL", version); }
                Ok(Err(error)) => failed(shell, error),
                Err(_) => silent(shell),
            }
        }
        Asked::Apply(_) | Asked::Rollback => {
            let Ok(state) = update::status(updater) else { return silent(shell) };
            let (shown, apply) = (view(&state), matches!(asked, Asked::Apply(_)));
            if let Some(code) = updates::refused(&shown, apply) { let _ = writeln!(shell.term, "UPDATE: {}", updates::error(code)); return; }
            let mut question = alloc::string::String::new();
            match asked { Asked::Apply(tries) => updates::apply_question(&mut question, &shown, tries), _ => updates::rollback_question(&mut question, &shown) }
            let into = updates::other(shown.slot);
            if !crate::msh::ask(shell, &question) { let _ = writeln!(shell.term, "UPDATE: NOT {}", if apply { "APPLIED" } else { "ROLLED BACK" }); return; }
            mind::println!("[SHELL] UPDATE {}: THE OPERATOR AGREED TO BOOT SLOT {} ON TRIAL", if apply { "APPLY" } else { "ROLLBACK" }, into);
            let done = match asked { Asked::Apply(tries) => update::apply(updater, tries), _ => update::rollback(updater) };
            match done {
                Ok(Ok(())) => { let _ = writeln!(shell.term, "UPDATE: THE MACHINE RESTARTS INTO SLOT {} ON TRIAL", into); }
                Ok(Err(error)) => failed(shell, error),
                Err(_) => silent(shell),
            }
        }
    }
}
