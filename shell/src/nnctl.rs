//! The system calls around `nnclients` (262-APP-0112, docs/nn/runtime.md section 1): the badged copy minted and lent
//! for a start, revoked when its program has ended, and `nnd`'s answers to the retirements.

use crate::nnclients::{Badges, Held, CLIENTS, NO_BADGE};
use mind::abi::{CAP_GRANT, CAP_WRITE, SLOT_NN};
use mind::idl::{nn, wire};
use mind::ipc::Endpoint;
use mind::sys::Error;

// How long the shell waits for `nnd`'s answer to a retirement, in ms: it asks again later rather than hold its loop.
const WAIT_MS: u32 = 50;

/// Whether the machine runs `nnd`: `init` gave the shell its client (262-KRN-0178).
pub fn present() -> bool { mind::dev::cap_info(SLOT_NN).0 != 0 }

/// What is due: `retire-all` once after the shell's start, then the ended programs' copies revoked and their badges
/// retired. False when `nnd` did not answer, so the shell asks later.
pub fn tend(badges: &mut Badges) -> bool {
    if !present() { return true; }
    if !badges.ready {
        if wire::with_timeout(WAIT_MS, || nn::retire_all(Endpoint(SLOT_NN))) != Ok(Ok(())) { return false; }
        badges.ready = true;
        mind::println!("[SHELL] NND: THE BADGES OF AN EARLIER SHELL RETIRED");
    }
    // The revocation removes every copy made from it, a child's too (000-KRN-0104); then the shell's own goes.
    badges.ended(&mut mind::process::alive, &mut |_, _, copy| { let _ = mind::ipc::revoke(copy); let _ = mind::ipc::drop_cap(copy); });
    loop {
        let next = badges.owed().next();
        let Some(badge) = next else { return true };
        if wire::with_timeout(WAIT_MS, || nn::retire(Endpoint(SLOT_NN), badge)) != Ok(Ok(())) { return false; }
        badges.retired(badge);
        mind::println!("[SHELL] NND: BADGE {} RETIRED", badge);
    }
}

/// A copy of the shell's client badged for one start, which the shell keeps: its badge and handle. None when `nnd` has
/// not answered `retire-all` yet: the program runs without it, as without another optional client.
pub fn mint(badges: &mut Badges) -> Result<Option<(u16, usize)>, Error> {
    tend(badges);
    if !badges.ready { return Ok(None); }
    let badge = badges.take().ok_or(Error::Other(NO_BADGE))?;
    match mind::ipc::mint_badged(SLOT_NN, CAP_WRITE | CAP_GRANT, badge) {
        Ok(copy) => Ok(Some((badge, copy))),
        Err(error) => { badges.owe(badge); Err(error) }
    }
}

/// The start failed after `mint`: the copy is revoked as at an end, and the badge waits for its retirement.
pub fn failed(badges: &mut Badges, badge: u16, copy: usize) {
    let _ = mind::ipc::revoke(copy);
    let _ = mind::ipc::drop_cap(copy);
    badges.owe(badge);
}

/// `nn [status]`: `nnd`'s quota, memory, workers, models, sessions and badges, and the shell's badges; `nn models`.
pub fn command(out: &mut crate::console::Console, badges: &Badges, args: &[u8]) {
    use core::fmt::Write;
    let args = core::str::from_utf8(args).unwrap_or("").trim();
    if !present() { let _ = writeln!(out, "NO MODEL SERVICE: INIT RUNS NND WHERE THE BOOT IMAGE HAS IT"); return; }
    match args {
        "" | "status" => {
            let Ok(s) = wire::with_timeout(1000, || nn::status(Endpoint(SLOT_NN))) else { let _ = writeln!(out, "NND DOES NOT ANSWER"); return; };
            let _ = writeln!(out, "NND: {} OF {} KIB USED, WORKERS {} OF {}, {} MODELS, {} SESSIONS, {} BADGES HOLDING SOMETHING",
                s.used / 1024, s.quota / 1024, s.workers, s.workers_max, s.models, s.sessions, s.badges);
            if s.workers_max == 0 { let _ = writeln!(out, "NO WORKERS: A LINE NND <MIB> IN DATA/MEMORY.CONF GIVES THEM; UNTIL THEN IT LOADS AND RUNS NOTHING"); }
            let (mut lent, mut owed) = (0, 0);
            for badge in 1..=CLIENTS as u16 {
                match badges.state(badge) {
                    Some(Held::Lent { pid, .. }) => { lent += 1; let _ = writeln!(out, "BADGE {}: PID {}", badge, pid); }
                    Some(Held::Owed) => { owed += 1; let _ = writeln!(out, "BADGE {}: ENDED, NOT RETIRED YET", badge); }
                    Some(Held::Starting) | None => {}
                }
            }
            let _ = writeln!(out, "SHELL: {} BADGES LENT, {} OWED A RETIREMENT, {} FREE{}", lent, owed, CLIENTS - lent - owed,
                if badges.ready { "" } else { " (NND HAS NOT ANSWERED RETIRE-ALL YET)" });
        }
        "models" => {
            let mut start = 0;
            loop {
                let Ok(page) = wire::with_timeout(1000, || nn::models(Endpoint(SLOT_NN), start)) else { let _ = writeln!(out, "NND DOES NOT ANSWER"); return; };
                for m in page.as_slice() {
                    let _ = write!(out, "{} ", m.name.as_str());
                    for byte in &m.id.as_slice()[..8] { let _ = write!(out, "{:02x}", byte); }
                    let _ = writeln!(out, "… {} IN, {} OUT, {} ROWS A JOB, {} KIB, {} SESSIONS{}{}", m.inputs, m.outputs, m.rows_max, m.bytes / 1024, m.sessions,
                        if m.licence.as_str().is_empty() { "" } else { ", " }, m.licence.as_str());
                }
                if start == 0 && page.as_slice().is_empty() { let _ = writeln!(out, "NO MODEL LOADED"); }
                if page.as_slice().len() < 8 { return; }
                start += 8;
            }
        }
        _ => { let _ = writeln!(out, "USAGE: NN [STATUS] | NN MODELS"); }
    }
}
