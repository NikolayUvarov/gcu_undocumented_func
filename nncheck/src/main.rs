#![no_std]
#![no_main]
// nncheck: the test program of the model service nnd (262-APP-0112, docs/nn/runtime.md section 8). Without a mode it
// checks the client the shell lends for REQUEST_NN: that there is one, its badge (one of this start's, never 0) and
// rights (no right to badge), nnd's status through it, and that a retirement from it is refused (`owner`: only the
// shell's badge 0 retires); `hold` then keeps the client that many seconds, for the shell's `nn status` to show.
// The modes run NRL's pinned model through nnd (nrl/specs/native-conformance-v0.md), its inputs built in:
// - `load PATH ID`: one load and its answer, the model unloaded after;
// - `run PATH ID`: outputs within NRL's tolerance and their charges, the budget's refusals, a cancellation and a
//   deadline partway through a job, a numeric failure and a new session, and nnd's memory back after the unload;
// - `share PATH ID SECONDS`: a session and a finished job held that long, for another client;
// - `other SESSION JOB ID`: that client's calls on them, each answer printed.
// One line each, `NNCHECK <what> <result>`; exit 0 when every check holds.
use mind::abi::{BootInfo, CAP_BADGE, CAP_KIND_ENDPOINT, SLOT_NN};
use mind::idl::nn::{self, Budget, Error};
use mind::idl::wire;
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_NN);

const NND: Endpoint = Endpoint(SLOT_NN);
const CALL_MS: u32 = 5000; // a call's longest wait for nnd's answer
const INPUT: &[u8] = include_bytes!("../../nrl/tests/fixtures/native-conformance-v0/input.f64le");
const EXPECTED: &[u8] = include_bytes!("../../nrl/tests/fixtures/native-conformance-v0/expected.f64le");
const LONG: &[u8] = include_bytes!("../../nrl/tests/fixtures/native-conformance-v0/long-input.f64le");
const NUMERIC: &[u8] = include_bytes!("../../nrl/tests/fixtures/native-conformance-v0/numeric-input.f64le");
const ROW_IN: usize = 24; // 3 values a row
const ROW_OUT: usize = 16; // 2 values a row
const WORK_ROW: u64 = 631; // the model's work units a row (NRL's pinned accounting)
const WORKSPACE: u64 = 77 * 8; // its workspace: 47 row values and 30 scratch values
const MEMORY: u64 = 64 * 1024; // a session's memory, room for 256 rows' output

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("nncheck — checks the model service's client the shell lent this start, and runs NRL's pinned model through nnd.\nUsage: nncheck [hold SECONDS] | nncheck load PATH ID | nncheck run PATH ID | nncheck share PATH ID SECONDS | nncheck other SESSION JOB ID");
    let (kind, badge, rights) = mind::dev::cap_info(SLOT_NN);
    if kind != CAP_KIND_ENDPOINT { mind::println!("NNCHECK CLIENT NONE"); mind::process::exit_with(1); }
    let mut ok = badge != 0 && rights & CAP_BADGE as usize == 0;
    mind::println!("NNCHECK CLIENT BADGE={} RIGHTS={} {}", badge, rights, if ok { "OK" } else { "WRONG" });
    let mut words = mind::process::args_str().split_whitespace();
    let mode = words.next();
    let mut id = [0u8; 32];
    match mode {
        Some("load" | "run" | "share") => {
            let path = words.next().unwrap_or("");
            if !hex(words.next().unwrap_or(""), &mut id) { usage(); }
            ok &= match mode {
                Some("load") => load_once(path, &id),
                Some("run") => run(path, &id),
                _ => share(path, &id, words.next().and_then(|w| w.parse().ok()).unwrap_or(60)),
            };
        }
        Some("other") => {
            let (Some(session), Some(job)) = (words.next().and_then(|w| w.parse().ok()), words.next().and_then(|w| w.parse().ok())) else { usage() };
            if !hex(words.next().unwrap_or(""), &mut id) { usage(); }
            other(session, job, &id);
        }
        Some("hold") | None => ok &= client(badge as u16, words.next().and_then(|w| w.parse().ok()).filter(|_| mode.is_some())),
        Some(_) => usage(),
    }
    mind::println!("NNCHECK {}", if ok { "DONE" } else { "FAILED" });
    mind::process::exit_with(if ok { 0 } else { 1 });
}

fn usage() -> ! { mind::println!("USAGE: NNCHECK [HOLD SECONDS] | LOAD PATH ID | RUN PATH ID | SHARE PATH ID SECONDS | OTHER SESSION JOB ID"); mind::process::exit_with(2) }

// 64 hex digits into `id`.
fn hex(text: &str, id: &mut [u8; 32]) -> bool {
    if text.len() != 64 { return false; }
    for (i, out) in id.iter_mut().enumerate() {
        let Ok(byte) = u8::from_str_radix(&text[2 * i..2 * i + 2], 16) else { return false };
        *out = byte;
    }
    true
}

// The lent client's checks: nnd's status, a retirement refused; then held `hold` seconds.
fn client(badge: u16, hold: Option<u64>) -> bool {
    let mut ok = true;
    match wire::with_timeout(2000, || nn::status(NND)) {
        Ok(s) => mind::println!("NNCHECK STATUS QUOTA={} USED={} WORKERS={}/{} MODELS={} SESSIONS={} BADGES={}", s.quota, s.used, s.workers, s.workers_max, s.models, s.sessions, s.badges),
        Err(error) => { ok = false; mind::println!("NNCHECK STATUS {:?}", error); }
    }
    for (what, answer) in [("RETIRE", wire::with_timeout(2000, || nn::retire(NND, badge))), ("RETIRE-ALL", wire::with_timeout(2000, || nn::retire_all(NND)))] {
        let refused = answer == Ok(Err(Error::Owner));
        ok &= refused;
        mind::println!("NNCHECK {} {:?} {}", what, answer, if refused { "REFUSED" } else { "WRONG" });
    }
    if let Some(seconds) = hold {
        mind::println!("NNCHECK HOLD {} S", seconds);
        let _ = mind::time::sleep(seconds as usize * 1000);
    }
    ok
}

// ---- the calls, each answer or the transport's failure as `Failed` ----

fn flat<T>(answer: mind::sys::Result<Result<T, Error>>) -> Result<T, Error> { answer.unwrap_or(Err(Error::Failed)) }
fn load(path: &str, id: &[u8; 32]) -> Result<nn::Model, Error> { flat(wire::with_timeout(60_000, || nn::load(NND, path, id))) }
fn unload(id: &[u8; 32]) -> Result<(), Error> { flat(wire::with_timeout(CALL_MS, || nn::unload(NND, id))) }
fn open(id: &[u8; 32], budget: Budget) -> Result<u32, Error> { flat(wire::with_timeout(CALL_MS, || nn::open(NND, id, &budget))) }
fn submit(session: u32, input: &[u8]) -> Result<u32, Error> { flat(wire::with_timeout(CALL_MS, || nn::submit(NND, session, (input.len() / ROW_IN) as u32, input))) }
fn wait(job: u32, wait_ms: u32, out: &mut [u8]) -> Result<usize, Error> { flat(wire::with_timeout(wait_ms + CALL_MS, || nn::wait(NND, job, wait_ms, out))) }
fn cancel(job: u32) -> Result<(), Error> { flat(wire::with_timeout(CALL_MS, || nn::cancel(NND, job))) }
fn used(session: u32) -> Result<nn::Usage, Error> { flat(wire::with_timeout(CALL_MS, || nn::usage(NND, session))) }
fn close(session: u32) -> Result<(), Error> { flat(wire::with_timeout(CALL_MS, || nn::close(NND, session))) }
fn status() -> Option<nn::Status> { wire::with_timeout(CALL_MS, || nn::status(NND)).ok() }

fn budget(rows: u64, work: u64, jobs: u32, memory: u64, deadline_ms: u64) -> Budget { Budget { rows, work, jobs, memory, deadline_ms } }
fn ample() -> Budget { budget(1 << 20, 1 << 40, 1000, MEMORY, 0) }

fn check(ok: &mut bool, what: &str, holds: bool) { *ok &= holds; mind::println!("NNCHECK {} {}", what, if holds { "OK" } else { "WRONG" }); }

// A session's charges as `rows/work/jobs`, and whether it ended.
fn charged(session: u32) -> (u64, u64, u32, bool) { used(session).map_or((u64::MAX, u64::MAX, u32::MAX, false), |u| (u.rows, u.work, u.jobs, u.ended)) }

// Output rows against the reference rows, each row `i` against reference row `i % 3`: NRL's tolerance.
fn within(out: &[u8]) -> bool {
    let value = |bytes: &[u8], i: usize| f64::from_le_bytes(bytes[8 * i..8 * i + 8].try_into().unwrap());
    out.len() % ROW_OUT == 0 && (0..out.len() / 8).all(|i| {
        let (a, r) = (value(out, i), value(EXPECTED, i % 6));
        a.is_finite() && (a - r).abs() <= 1e-12 + 1e-10 * r.abs()
    })
}

// ---- the modes ----

fn load_once(path: &str, id: &[u8; 32]) -> bool {
    match load(path, id) {
        Ok(m) => {
            mind::println!("NNCHECK LOAD {} Ok IN={} OUT={} ROWS={} BYTES={}", path, m.inputs, m.outputs, m.rows_max, m.bytes);
            mind::println!("NNCHECK UNLOAD {:?}", unload(id));
        }
        Err(error) => mind::println!("NNCHECK LOAD {} Err({:?})", path, error),
    }
    true
}

fn run(path: &str, id: &[u8; 32]) -> bool {
    let mut ok = true;
    let mut out = [0u8; 256 * ROW_OUT];
    let Some(before) = status() else { mind::println!("NNCHECK STATUS NO ANSWER"); return false };
    let model = load(path, id);
    match &model {
        Ok(m) => check(&mut ok, "LOAD IN=3 OUT=2 ROWS=256", m.inputs == 3 && m.outputs == 2 && m.rows_max == 256),
        Err(error) => { mind::println!("NNCHECK LOAD Err({:?}) WRONG", error); return false; }
    }
    let loaded = status().map_or(0, |s| s.used);
    mind::println!("NNCHECK MEMORY USED={} BEFORE, {} LOADED", before.used, loaded);

    // Three ordinary rows: the reference outputs, charged 3 rows, 1893 units, 1 job; a refused row charges nothing.
    let Ok(s) = open(id, ample()) else { mind::println!("NNCHECK OPEN WRONG"); return false };
    let start = mind::time::monotonic_ns();
    let got = submit(s, INPUT).and_then(|j| wait(j, CALL_MS, &mut out));
    // Three rows take little of it: most is the start of the job's worker (a worker per job, section 5).
    mind::println!("NNCHECK 3 ROWS TOOK {} US, MOSTLY THE WORKER'S START", (mind::time::monotonic_ns() - start) / 1000);
    check(&mut ok, "OUTPUTS 3 ROWS WITHIN 1e-12 + 1e-10*|r| (DEADLINE 0: NONE)", got == Ok(48) && within(&out[..48]));
    check(&mut ok, "CHARGED 3/1893/1", charged(s) == (3, 1893, 1, false));
    let mut nan = [0u8; ROW_IN];
    nan.copy_from_slice(&INPUT[..ROW_IN]);
    nan[8..16].copy_from_slice(&f64::NAN.to_le_bytes());
    check(&mut ok, "NONFINITE INPUT REFUSED (MALFORMED), NOTHING CHARGED", submit(s, &nan) == Err(Error::Malformed) && charged(s) == (3, 1893, 1, false));
    let mut wide = [0u8; 257 * ROW_IN];
    wide[..LONG.len()].copy_from_slice(LONG);
    wide[LONG.len()..].copy_from_slice(&INPUT[..ROW_IN]);
    check(&mut ok, "257 ROWS REFUSED (TOO-LARGE)", submit(s, &wide) == Err(Error::TooLarge));
    // 256 rows: each row's output, and how long the job takes, which times the cancellation and the deadline below.
    let start = mind::time::monotonic_ns();
    let got = submit(s, LONG).and_then(|j| wait(j, 60_000, &mut out));
    let took_us = (mind::time::monotonic_ns() - start) / 1000;
    check(&mut ok, "OUTPUTS 256 ROWS WITHIN THE TOLERANCE", got == Ok(4096) && within(&out));
    mind::println!("NNCHECK 256 ROWS TOOK {} US", took_us);
    let _ = close(s);

    // The whole request against what is left: 3 rows of 1893 units on a budget of 1892 refused, 1 row then runs.
    if let Ok(s) = open(id, budget(1 << 20, 3 * WORK_ROW - 1, 1000, MEMORY, 0)) {
        check(&mut ok, "WORK BUDGET 1892: 3 ROWS REFUSED (BUDGET)", submit(s, INPUT) == Err(Error::Budget) && charged(s) == (0, 0, 0, false));
        check(&mut ok, "WORK BUDGET 1892: 1 ROW RUNS", submit(s, &INPUT[..ROW_IN]).and_then(|j| wait(j, CALL_MS, &mut out)) == Ok(ROW_OUT) && charged(s) == (1, WORK_ROW, 1, false));
        let _ = close(s);
    } else { check(&mut ok, "OPEN FOR THE WORK BUDGET", false); }
    // Cumulative rows: 2 run, the next 2 refused, 1 runs, the next refused.
    if let Ok(s) = open(id, budget(3, 1 << 40, 1000, MEMORY, 0)) {
        let two = submit(s, &INPUT[..2 * ROW_IN]).and_then(|j| wait(j, CALL_MS, &mut out)) == Ok(2 * ROW_OUT);
        let refused = submit(s, &INPUT[..2 * ROW_IN]) == Err(Error::Budget);
        let one = submit(s, &INPUT[2 * ROW_IN..]).and_then(|j| wait(j, CALL_MS, &mut out)) == Ok(ROW_OUT);
        let last = submit(s, &INPUT[..ROW_IN]) == Err(Error::Budget);
        check(&mut ok, "ROW BUDGET 3: 2 RUN, 2 REFUSED, 1 RUNS, 1 REFUSED; CHARGED 3/1893/2", two && refused && one && last && charged(s) == (3, 1893, 2, false));
        let _ = close(s);
    } else { check(&mut ok, "OPEN FOR THE ROW BUDGET", false); }
    // Two jobs at most: the third refused.
    if let Ok(s) = open(id, budget(1 << 20, 1 << 40, 2, MEMORY, 0)) {
        let runs = (0..2).all(|_| submit(s, &INPUT[..ROW_IN]).and_then(|j| wait(j, CALL_MS, &mut out)) == Ok(ROW_OUT));
        check(&mut ok, "JOB BUDGET 2: THE THIRD REFUSED; CHARGED 2/1262/2", runs && submit(s, &INPUT[..ROW_IN]) == Err(Error::Budget) && charged(s) == (2, 2 * WORK_ROW, 2, false));
        let _ = close(s);
    } else { check(&mut ok, "OPEN FOR THE JOB BUDGET", false); }
    // Memory: the workspace and one output row at least; one byte less is refused.
    let least = WORKSPACE + ROW_OUT as u64;
    check(&mut ok, "MEMORY ONE BYTE SHORT OF THE WORKSPACE AND A ROW REFUSED (BUDGET)", open(id, budget(1, 1 << 40, 1, least - 1, 0)) == Err(Error::Budget));
    if let Ok(s) = open(id, budget(1, 1 << 40, 1, least, 0)) {
        check(&mut ok, "MEMORY OF THE WORKSPACE AND A ROW: 1 ROW RUNS", submit(s, &INPUT[..ROW_IN]).and_then(|j| wait(j, CALL_MS, &mut out)) == Ok(ROW_OUT));
        let _ = close(s);
    } else { check(&mut ok, "MEMORY OF THE WORKSPACE AND A ROW OPENS", false); }

    // Cancellation partway: a 256-row job cancelled after a wait, on a fresh session each try, 16 at most. The wait
    // starts once nnd has started the worker; after a job that finished first the next wait is shorter, after one
    // cancelled before its first row it is longer. Only a partway cancellation passes.
    let mut partway = false;
    let mut delay_ms: u32 = 40;
    for attempt in 0..16 {
        let Ok(s) = open(id, ample()) else { break };
        let Ok(job) = submit(s, LONG) else { let _ = close(s); break };
        let early = wait(job, delay_ms, &mut out);
        let cancelled = early == Err(Error::Pending) && cancel(job) == Ok(());
        let end = if early == Err(Error::Pending) { wait(job, 60_000, &mut out) } else { early };
        let (rows, work, jobs, ended) = charged(s);
        let poisoned = submit(s, &INPUT[..ROW_IN]) == Err(Error::Poisoned);
        let outcome = match end { Err(Error::Cancelled) if rows > 0 && rows < 256 => "CANCELLED PARTWAY", Err(Error::Cancelled) => "CANCELLED BEFORE ITS FIRST ROW", Ok(_) => "COMPLETED BEFORE THE CANCEL", _ => "WRONG" };
        mind::println!("NNCHECK CANCEL TRY {} AFTER {} MS: {} {:?}, CHARGED {}/{}/{}, ENDED={} POISONED={}", attempt + 1, delay_ms, outcome, end.map(|_| ()), rows, work, jobs, ended, poisoned);
        let _ = close(s);
        if cancelled && end == Err(Error::Cancelled) && rows > 0 && rows < 256 && jobs == 1 && ended && poisoned { partway = true; break; }
        delay_ms = if end.is_ok() { delay_ms * 2 / 3 } else { delay_ms + 15 };
    }
    check(&mut ok, "CANCEL PARTWAY: CANCELLED, ITS ROWS CHARGED, THE SESSION ENDED", partway);

    // A deadline partway through, on a fresh session each try, 64 at most. It counts from the session's opening, and
    // the worker starts after the submit: under TCG that start took from about 30 to 550 ms, the rows about 70 ms.
    // So each try's deadline follows the last one's end: one that ended before its first row ended about when its
    // rows would have begun, so the next deadline comes a little after that; after one that finished first, a little
    // before its end. Only a deadline partway passes. Then one passed before the job starts.
    let mut partway = false;
    let mut deadline_ms = (took_us * 3 / 4000).max(1);
    for attempt in 0..64 {
        let opened = mind::time::monotonic_ns();
        let Ok(s) = open(id, budget(1 << 20, 1 << 40, 1000, MEMORY, deadline_ms)) else { break };
        let end = submit(s, LONG).and_then(|j| wait(j, 60_000, &mut out));
        let elapsed_ms = (mind::time::monotonic_ns() - opened) / 1_000_000; // on this program's clock: open to the job's end
        let (rows, work, jobs, ended) = charged(s);
        let poisoned = submit(s, &INPUT[..ROW_IN]) == Err(Error::Poisoned);
        let outcome = match end { Err(Error::Deadline) if rows > 0 && rows < 256 => "ENDED PARTWAY", Err(Error::Deadline) => "ENDED BEFORE ITS FIRST ROW", Ok(_) => "COMPLETED BEFORE THE DEADLINE", _ => "WRONG" };
        mind::println!("NNCHECK DEADLINE TRY {} OF {} MS: {} {:?} AFTER {} MS, CHARGED {}/{}/{}, ENDED={} POISONED={}", attempt + 1, deadline_ms, outcome, end.map(|_| ()), elapsed_ms, rows, work, jobs, ended, poisoned);
        let _ = close(s);
        if end == Err(Error::Deadline) && rows > 0 && rows < 256 && jobs == 1 && ended && poisoned { partway = true; break; }
        deadline_ms = if end.is_ok() { elapsed_ms.saturating_sub(35).max(1) } else { elapsed_ms + 20 };
    }
    check(&mut ok, "DEADLINE PARTWAY: DEADLINE, ITS ROWS CHARGED, THE SESSION ENDED", partway);
    if let Ok(s) = open(id, budget(1 << 20, 1 << 40, 1000, MEMORY, 1)) {
        let _ = mind::time::sleep(20);
        let end = submit(s, INPUT).and_then(|j| wait(j, CALL_MS, &mut out));
        let (rows, work, _, ended) = charged(s);
        check(&mut ok, "DEADLINE PASSED BEFORE THE JOB: DEADLINE, NO ROW CHARGED", end == Err(Error::Deadline) && rows == 0 && work == 0 && ended);
        let _ = close(s);
    } else { check(&mut ok, "OPEN WITH A 1 MS DEADLINE", false); }

    // A finite row that overflows inside: numeric, charged 1 row and 39 units, the session ended; a new one runs.
    if let Ok(s) = open(id, ample()) {
        let end = submit(s, NUMERIC).and_then(|j| wait(j, CALL_MS, &mut out));
        let (rows, work, jobs, ended) = charged(s);
        mind::println!("NNCHECK NUMERIC: {:?}, CHARGED {}/{}/{}, ENDED={}", end.map(|_| ()), rows, work, jobs, ended);
        check(&mut ok, "NUMERIC: CHARGED 1/39/1, THE SESSION ENDED, THE NEXT JOB POISONED", end == Err(Error::Numeric) && (rows, work, jobs, ended) == (1, 39, 1, true) && submit(s, INPUT) == Err(Error::Poisoned) && charged(s) == (1, 39, 1, true));
        let _ = close(s);
    } else { check(&mut ok, "OPEN FOR THE NUMERIC ROW", false); }
    if let Ok(s) = open(id, ample()) {
        check(&mut ok, "A NEW SESSION AFTER THE FAILURE: 3 ROWS, CHARGED 3/1893/1", submit(s, INPUT).and_then(|j| wait(j, CALL_MS, &mut out)) == Ok(48) && within(&out[..48]) && charged(s) == (3, 1893, 1, false));
        let _ = close(s);
    } else { check(&mut ok, "OPEN AFTER THE FAILURE", false); }

    // Everything closed and the model unloaded: nnd's memory as before the load.
    check(&mut ok, "UNLOAD", unload(id) == Ok(()));
    let after = status().map_or(u64::MAX, |s| s.used);
    mind::println!("NNCHECK MEMORY USED={} AFTER", after);
    check(&mut ok, "MEMORY BACK TO BEFORE THE LOAD", after == before.used && loaded > before.used);
    ok
}

fn share(path: &str, id: &[u8; 32], seconds: u64) -> bool {
    let Ok(_) = load(path, id) else { mind::println!("NNCHECK SHARE LOAD WRONG"); return false };
    let Ok(s) = open(id, ample()) else { mind::println!("NNCHECK SHARE OPEN WRONG"); return false };
    let Ok(job) = submit(s, INPUT) else { mind::println!("NNCHECK SHARE SUBMIT WRONG"); return false };
    // Its end left with nnd, for the other client to ask about: the session's charges say it ran, and `wait` would take it.
    let mut done = false;
    for _ in 0..250 {
        if charged(s).2 == 1 { done = true; break; }
        let _ = mind::time::sleep(20);
    }
    mind::println!("NNCHECK SHARE SESSION={} JOB={} {}", s, job, if done { "HELD" } else { "WRONG" });
    let _ = mind::time::sleep(seconds as usize * 1000);
    done
}

// Another client's calls on a session and a job it does not own, and on the model, which it does not hold.
fn other(session: u32, job: u32, id: &[u8; 32]) {
    let mut out = [0u8; 48];
    mind::println!("NNCHECK OTHER USAGE {:?}", used(session).map(|_| ()));
    mind::println!("NNCHECK OTHER SUBMIT {:?}", submit(session, INPUT).map(|_| ()));
    mind::println!("NNCHECK OTHER WAIT {:?}", wait(job, 0, &mut out).map(|_| ()));
    mind::println!("NNCHECK OTHER CANCEL {:?}", cancel(job));
    mind::println!("NNCHECK OTHER CLOSE {:?}", close(session));
    mind::println!("NNCHECK OTHER UNLOAD {:?}", unload(id));
    // A session of its own on the model another holds: a number other than the first's, and its job runs.
    let own = open(id, ample());
    mind::println!("NNCHECK OTHER OPEN {:?}", own.map(|s| s != session));
    if let Ok(s) = own {
        let ran = submit(s, INPUT).and_then(|j| wait(j, CALL_MS, &mut out));
        mind::println!("NNCHECK OTHER OWN JOB {:?} {}", ran, if ran == Ok(48) && within(&out) { "WITHIN THE TOLERANCE" } else { "WRONG" });
        let _ = close(s);
    }
}
