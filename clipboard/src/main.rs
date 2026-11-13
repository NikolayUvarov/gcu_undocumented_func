#![no_std]
#![no_main]
// clipboard (000-APP-0032, idl/clipboard.wit 1.1): one text the user copied, in this service's memory only. The badge of
// a client says who calls: a manager (the shell, the window manager it started) or a program a manager lent a copy;
// a program is answered only while a manager names it in front. A manager's program-client is minted here from the
// service's endpoint, as a copy on the wire carries no right to badge. Each text set and read is logged, never the text.
extern crate alloc;

use clipboard::state::{Clipboard, Refusal, Who, PART};
use mind::abi::{BootInfo, CAP_GRANT, CAP_WRITE, SLOT_SERVICE};
use mind::clipboard::{BADGE_MANAGER, BADGE_PROGRAM};
use mind::idl::{clipboard as api, wire};
use mind::ipc::Endpoint;

const RECEIVED: usize = 9;

fn who(badge: u16) -> Who {
    match badge { BADGE_MANAGER => Who::Manager, BADGE_PROGRAM => Who::Program, _ => Who::Other }
}

fn error(refusal: Refusal) -> api::Error {
    match refusal { Refusal::Behind => api::Error::Behind, Refusal::Refused => api::Error::Refused, Refusal::Invalid => api::Error::Invalid, Refusal::Changed => api::Error::Changed }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut clipboard = Clipboard::new();
    mind::println!("[CLIPBOARD] READY: ONE TEXT IN MEMORY, UP TO {} BYTES, IN PARTS OF {}", clipboard::state::MAX, PART);
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        if !request.is_call { continue; }
        let (caller, pid) = (who(request.badge), request.sender);
        let _ = match api::decode(&request, RECEIVED) {
            Err(reason) => wire::reject(reason),
            Ok((api::Request::SetPart { at, text, last }, call)) => {
                let set = clipboard.set_part(caller, pid, at as usize, text.as_str().as_bytes(), last);
                match set {
                    Ok(generation) if last => mind::println!("[CLIPBOARD] SET BY PID {}: {} BYTES, GENERATION {}", pid, clipboard.length(), generation),
                    Err(refusal) => mind::println!("[CLIPBOARD] SET REFUSED TO PID {}: {:?}", pid, refusal),
                    Ok(_) => {}
                }
                api::reply_set_part(call, set.map_err(error))
            }
            Ok((api::Request::GetPart { generation, at }, call)) => match clipboard.get_part(caller, pid, generation, at as usize) {
                Ok((generation, length, part)) => {
                    if at == 0 { mind::println!("[CLIPBOARD] READ BY PID {}: {} BYTES, GENERATION {}", pid, length, generation); }
                    let text = core::str::from_utf8(part).ok().and_then(|s| mind::idl::codec::Text::<4000>::new(s));
                    match text {
                        Some(text) => api::reply_get_part(call, Ok(&api::Part { generation, length: length as u32, text })),
                        None => api::reply_get_part(call, Err(api::Error::Invalid)),
                    }
                }
                Err(refusal) => { mind::println!("[CLIPBOARD] READ REFUSED TO PID {}: {:?}", pid, refusal); api::reply_get_part(call, Err(error(refusal))) }
            },
            Ok((api::Request::Generation, call)) => api::reply_generation(call, clipboard.generation_for(caller).map_err(error)),
            Ok((api::Request::ProgramClient, call)) => {
                // Moved to the manager, which lends it: the service keeps no copy.
                let minted = clipboard.program_client(caller).map_err(error)
                    .and_then(|()| mind::ipc::mint_badged(SLOT_SERVICE, CAP_WRITE | CAP_GRANT, BADGE_PROGRAM).map_err(|_| api::Error::Unavailable));
                match minted {
                    Ok(_) => mind::println!("[CLIPBOARD] A PROGRAM'S CLIENT MINTED FOR PID {}", pid),
                    Err(api::Error::Unavailable) => mind::println!("[CLIPBOARD] NO PROGRAM'S CLIENT FOR PID {}: THIS SERVICE MAY NOT BADGE ITS ENDPOINT", pid),
                    Err(e) => mind::println!("[CLIPBOARD] PROGRAM'S CLIENT REFUSED TO PID {}: {:?}", pid, e),
                }
                api::reply_program_client(call, minted)
            }
            // Not logged: the shell names a program in front at each command typed, and the boot's log would fill with it
            // (KRN's remark of 2026-11-13); a read or a set refused names the caller.
            Ok((api::Request::Front { pid: front }, call)) => api::reply_front(call, clipboard.set_front(caller, front).map_err(error)),
        };
    }
}
