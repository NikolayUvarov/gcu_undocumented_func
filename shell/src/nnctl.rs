//! The system calls around `nnclients` (262-APP-0112, docs/nn/runtime.md section 1): the badged copy minted and lent
//! for a start, revoked when its program has ended, and `nnd`'s answers to the retirements.

use crate::nnclients::{Badges, Held, CLIENTS, NO_BADGE};
use crate::nnrows;
use alloc::string::String;
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

/// `nn [status]`: `nnd`'s quota, memory, workers, models, sessions and badges, and the shell's badges; `nn models`;
/// `nn load <path> <id>`, `nn unload <id>`, `nn run <id> <file>` (the file's bytes, which the shell read first, in
/// `file`), `nn bench <id>` (262-APP-0112, docs/nn/runtime.md section 7), on the shell's own client (badge 0).
pub fn command(out: &mut crate::console::Console, badges: &Badges, args: &[u8], file: Option<Result<alloc::vec::Vec<u8>, String>>) {
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
        _ => {
            let words: alloc::vec::Vec<&str> = args.split_whitespace().collect();
            let id = words.get(if words.first() == Some(&"load") { 2 } else { 1 }).and_then(|w| nnrows::id(w));
            match (words.as_slice(), id) {
                (["load", path, _], Some(id)) => load(out, path, &id),
                (["unload", _], Some(id)) => unload(out, &id),
                (["run", _, _], Some(id)) => match file {
                    Some(Ok(bytes)) => run(out, &id, &bytes),
                    Some(Err(why)) => { let _ = writeln!(out, "ERROR: NN RUN: {}", why); }
                    None => { let _ = writeln!(out, "{}", USAGE); }
                },
                (["bench", _], Some(id)) => bench(out, &id),
                _ => { let _ = writeln!(out, "{}", USAGE); }
            }
        }
    }
}

const USAGE: &str = "USAGE: NN [STATUS] | NN MODELS | NN LOAD <PATH> <ID> | NN UNLOAD <ID> | NN RUN <ID> <FILE OF ROWS> | NN BENCH <ID> (ID: 64 HEX DIGITS)";
// How long a load may take (the parser reads and checks the package), and a call besides, in ms.
const LOAD_MS: u32 = 60_000;
const CALL_MS: u32 = 5000;
// A job's longest wait for its rows, in ms: it is cancelled after.
const JOB_MS: u64 = 60_000;
// A session's memory beyond its output rows, for the model's workspace, which the shell cannot know.
const WORKSPACE: u64 = 1 << 20;
// How long `nn bench` submits rows, in ms.
const BENCH_MS: u64 = 2000;

fn said(error: nn::Error) -> &'static str {
    match error {
        nn::Error::NotFound => "NOT FOUND (NO SUCH FILE, MODEL OR SESSION)", nn::Error::Digest => "THE FILE OR THE MODEL ID IS NOT AS GIVEN",
        nn::Error::Malformed => "MALFORMED (THE PACKAGE, OR AN INPUT ROW)", nn::Error::Unsupported => "A FAMILY, PROFILE OR OPERATOR NND DOES NOT RUN",
        nn::Error::TooLarge => "PAST A BOUND OF THE PROFILE OR OF NND", nn::Error::NoMemory => "NND'S QUOTA IS SPENT",
        nn::Error::NoWorkers => "NND HAS NO WORKERS: A LINE NND <MIB> IN DATA/MEMORY.CONF GIVES THEM", nn::Error::Owner => "ANOTHER CLIENT'S",
        nn::Error::Budget => "PAST THE SESSION'S BUDGET", nn::Error::Busy => "BUSY (THE MODEL HAS SESSIONS, OR TOO MANY JOBS WAIT)",
        nn::Error::Pending => "NOT ENDED YET", nn::Error::Cancelled => "CANCELLED", nn::Error::Deadline => "PAST ITS DEADLINE",
        nn::Error::Numeric => "A VALUE INSIDE THE MODEL WAS NOT FINITE", nn::Error::Poisoned => "THE SESSION HAD ENDED", nn::Error::Failed => "THE WORKER ENDED WITHOUT AN ANSWER",
    }
}

// One call's answer, or the line saying why there is none.
fn call<T>(what: &str, ms: u32, f: impl FnOnce() -> mind::sys::Result<Result<T, nn::Error>>) -> Result<T, String> {
    match wire::with_timeout(ms, f) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(alloc::format!("ERROR: NN {}: {}", what, said(error))),
        Err(_) => Err(alloc::format!("ERROR: NN {}: NND DID NOT ANSWER", what)),
    }
}

fn load(out: &mut crate::console::Console, path: &str, id: &[u8; 32]) {
    use core::fmt::Write;
    match call("LOAD", LOAD_MS, || nn::load(Endpoint(SLOT_NN), path, id)) {
        Ok(m) => { let _ = writeln!(out, "LOADED {} {}: {} IN, {} OUT, {} ROWS A JOB, {} KIB", m.name.as_str(), nnrows::short(m.id.as_slice()), m.inputs, m.outputs, m.rows_max, m.bytes / 1024); }
        Err(line) => { let _ = writeln!(out, "{}", line); }
    }
}

fn unload(out: &mut crate::console::Console, id: &[u8; 32]) {
    use core::fmt::Write;
    match call("UNLOAD", CALL_MS, || nn::unload(Endpoint(SLOT_NN), id)) {
        Ok(()) => { let _ = writeln!(out, "UNLOADED {}: THE SHELL'S HOLD DROPPED", nnrows::short(id)); }
        Err(line) => { let _ = writeln!(out, "{}", line); }
    }
}

// The loaded model with this ID.
fn model(id: &[u8; 32]) -> Result<nn::Model, String> {
    let mut start = 0;
    loop {
        let page = wire::with_timeout(CALL_MS, || nn::models(Endpoint(SLOT_NN), start)).map_err(|_| String::from("ERROR: NN: NND DID NOT ANSWER"))?;
        if let Some(m) = page.as_slice().iter().find(|m| m.id.as_slice() == id) { return Ok(m.clone()); }
        if page.as_slice().len() < 8 { return Err(alloc::format!("ERROR: NN: NO MODEL {} IS LOADED (NN LOAD <PATH> <ID> FIRST)", nnrows::short(id))); }
        start += 8;
    }
}

// A session for `rows` rows in jobs of `per`, its output and a workspace reserved.
fn open(m: &nn::Model, rows: u64, per: usize) -> Result<u32, String> {
    let jobs = rows.div_ceil(per as u64).min(u32::MAX as u64) as u32;
    let budget = nn::Budget { rows, work: u64::MAX >> 1, jobs, memory: per as u64 * m.outputs as u64 * 8 + WORKSPACE, deadline_ms: 0 };
    call("OPEN", CALL_MS, || nn::open(Endpoint(SLOT_NN), &m.id.as_slice().try_into().unwrap_or([0; 32]), &budget))
}

// One job of `input` rows, waited for: its output's length in `output`.
fn job(session: u32, rows: usize, input: &[u8], output: &mut [u8]) -> Result<usize, String> {
    let job = call("SUBMIT", CALL_MS, || nn::submit(Endpoint(SLOT_NN), session, rows as u32, input))?;
    let since = mind::time::uptime_ms() as u64;
    loop {
        match wire::with_timeout(1000 + CALL_MS, || nn::wait(Endpoint(SLOT_NN), job, 1000, output)) {
            Ok(Err(nn::Error::Pending)) if (mind::time::uptime_ms() as u64) < since + JOB_MS => continue,
            Ok(Err(nn::Error::Pending)) => {
                let _ = wire::with_timeout(CALL_MS, || nn::cancel(Endpoint(SLOT_NN), job));
                return Err(alloc::format!("ERROR: NN: THE JOB DID NOT END IN {} S: CANCELLED", JOB_MS / 1000));
            }
            Ok(Ok(len)) => return Ok(len),
            Ok(Err(error)) => return Err(alloc::format!("ERROR: NN WAIT: {}", said(error))),
            Err(_) => return Err(String::from("ERROR: NN WAIT: NND DID NOT ANSWER")),
        }
    }
}

// `nn run <id> <file>`: the file's rows through the model, each output row as a line, then what the session used.
fn run(out: &mut crate::console::Console, id: &[u8; 32], bytes: &[u8]) {
    use core::fmt::Write;
    let m = match model(id) { Ok(m) => m, Err(line) => { let _ = writeln!(out, "{}", line); return; } };
    let Ok(text) = core::str::from_utf8(bytes) else { let _ = writeln!(out, "ERROR: NN RUN: THE FILE IS NOT TEXT"); return; };
    let values = match nnrows::rows(text, m.inputs as usize) { Ok(values) => values, Err(why) => { let _ = writeln!(out, "ERROR: NN RUN: {}", why); return; } };
    let (width, rows) = (m.inputs.max(1) as usize, values.len() / m.inputs.max(1) as usize);
    let per = nnrows::per_job(m.rows_max, m.inputs, m.outputs);
    if rows == 0 { let _ = writeln!(out, "ERROR: NN RUN: THE FILE HAS NO ROWS"); return; }
    if per == 0 { let _ = writeln!(out, "ERROR: NN RUN: NOT ONE ROW OF THIS MODEL FITS A CALL"); return; }
    let session = match open(&m, rows as u64, per) { Ok(s) => s, Err(line) => { let _ = writeln!(out, "{}", line); return; } };
    let since = mind::time::uptime_ms() as u64;
    let mut output = alloc::vec![0u8; nnrows::CALL_BYTES];
    for chunk in values.chunks(per * width) {
        match job(session, chunk.len() / width, &nnrows::encode(chunk), &mut output) {
            Ok(len) => for row in output[..len].chunks(m.outputs.max(1) as usize * 8) { let _ = writeln!(out, "{}", nnrows::row_text(row)); },
            Err(line) => { let _ = writeln!(out, "{}", line); break; }
        }
    }
    let took = mind::time::uptime_ms() as u64 - since;
    if let Ok(u) = call("USAGE", CALL_MS, || nn::usage(Endpoint(SLOT_NN), session)) {
        let _ = writeln!(out, "{} ROWS THROUGH {} IN {} MS: CHARGED {} ROWS, {} WORK, {} JOBS", rows, m.name.as_str(), took, u.rows, u.work, u.jobs);
    }
    let _ = call("CLOSE", CALL_MS, || nn::close(Endpoint(SLOT_NN), session));
}

// `nn bench <id>`: rows of zeros, a job of the most a call carries after another, for BENCH_MS; the rate and the
// session's memory, for this machine alone (MC-12.1).
fn bench(out: &mut crate::console::Console, id: &[u8; 32]) {
    use core::fmt::Write;
    let m = match model(id) { Ok(m) => m, Err(line) => { let _ = writeln!(out, "{}", line); return; } };
    let per = nnrows::per_job(m.rows_max, m.inputs, m.outputs);
    if per == 0 { let _ = writeln!(out, "ERROR: NN BENCH: NOT ONE ROW OF THIS MODEL FITS A CALL"); return; }
    let session = match open(&m, u32::MAX as u64, per) { Ok(s) => s, Err(line) => { let _ = writeln!(out, "{}", line); return; } };
    let input = nnrows::encode(&alloc::vec![0.0; per * m.inputs as usize]);
    let mut output = alloc::vec![0u8; nnrows::CALL_BYTES];
    let (since, mut rows, mut jobs) = (mind::time::uptime_ms() as u64, 0u64, 0u32);
    let mut failed = None;
    while (mind::time::uptime_ms() as u64) < since + BENCH_MS {
        match job(session, per, &input, &mut output) { Ok(_) => { rows += per as u64; jobs += 1; } Err(line) => { failed = Some(line); break; } }
    }
    let took = mind::time::uptime_ms() as u64 - since;
    let memory = call("USAGE", CALL_MS, || nn::usage(Endpoint(SLOT_NN), session)).map(|u| u.memory).unwrap_or(0);
    let _ = call("CLOSE", CALL_MS, || nn::close(Endpoint(SLOT_NN), session));
    if let Some(line) = failed { let _ = writeln!(out, "{}", line); }
    let workers = wire::with_timeout(CALL_MS, || nn::status(Endpoint(SLOT_NN))).map(|s| s.workers_max).unwrap_or(0);
    let _ = writeln!(out, "BENCH {}: {} ROWS IN {} JOBS OF {} IN {} MS: {} ROWS A SECOND; THE SESSION HELD {} KIB; THIS MACHINE ONLY, NND WITH {} WORKERS AT MOST",
        m.name.as_str(), rows, jobs, per, took, nnrows::rate(rows, took), memory / 1024, workers);
}
