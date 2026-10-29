// nnd's clients on NRL's pinned model (262-APP-0112, docs/nn/runtime.md sections 1, 4, 5 and 8): badges, budgets,
// jobs and retirement, the jobs run in place of a worker.
use nnpkg::package::{self, Package};
use nnpkg::service::{execute, Budget, Error, Service, State, SHELL};
use nrl_runtime::{Checkpoint, Control};
use std::path::PathBuf;

const MODEL_ID: &str = "1b23aaf4baa75fa146ee8618ea1f7e0e64699c1ea2ee7cf27eeb8bc83f0c1bee";
const MIB: u64 = 1 << 20;

fn fixture() -> PathBuf {
    std::env::var_os("NNPKG_FIXTURE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../nrl/tests/fixtures/native-conformance-v0"))
}
fn read(name: &str) -> Vec<u8> { std::fs::read(fixture().join(name)).unwrap() }
fn id() -> [u8; 32] { let mut id = [0; 32]; for i in 0..32 { id[i] = u8::from_str_radix(&MODEL_ID[2 * i..2 * i + 2], 16).unwrap(); } id }
fn model() -> Package {
    let objects: Vec<(String, Vec<u8>)> = (0..7).map(|i| { let n = package::object_file(i); let b = read(&format!("model/{n}")); (n, b) }).collect();
    package::load(&read("model/manifest.json"), &id(), |name| objects.iter().find(|(n, _)| n == name).map(|(_, b)| b.as_slice())).unwrap()
}
fn budget() -> Budget { Budget { rows: 256, work: 256 * 631, jobs: 16, memory: 77 * 8 + 512 * 8, deadline_ms: 0 } }
fn f64s(bytes: &[u8]) -> Vec<f64> { bytes.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect() }
fn le(values: &[f64]) -> Vec<u8> { values.iter().flat_map(|v| v.to_le_bytes()).collect() }

// The clock of the tests: a tick a checkpoint; it cancels at `cancel_at`.
struct Clock { tick: u64, cancel_at: Option<u64> }
impl Control for Clock {
    fn checkpoint(&mut self) -> Checkpoint { self.tick += 1; Checkpoint { tick: self.tick, cancelled: self.cancel_at == Some(self.tick) } }
}

// Every queued job run, as nnd's workers would.
fn run(service: &mut Service, clock: &mut Clock) {
    while let Some(r) = service.start() {
        let input = service.jobs.iter().find(|j| j.id == r.job).unwrap().input.clone();
        let outcome = execute(&service.model(&r.model).unwrap().package, &r, &input, clock);
        service.finish(r.job, outcome);
    }
}
fn clock() -> Clock { Clock { tick: 0, cancel_at: None } }

fn loaded(badges: &[u16]) -> Service {
    let mut s = Service::new(64 * MIB, 4);
    for &b in badges { assert!(s.may_load(b, &id()).unwrap().is_none()); s.insert(b, model()).unwrap(); }
    s
}

#[test]
fn outputs_match_the_reference_and_charges_are_nrls() {
    let mut s = loaded(&[1]);
    let session = s.open(1, &id(), budget(), 0).unwrap();
    let job = s.submit(1, session, 3, &read("input.f64le")).unwrap();
    assert_eq!(s.take(1, job), Err(Error::Pending));
    run(&mut s, &mut clock());
    let out = s.take(1, job).unwrap();
    for (a, r) in out.iter().zip(f64s(&read("expected.f64le"))) { assert!((a - r).abs() <= 1e-12 + 1e-10 * r.abs()); }
    let u = s.usage(1, session, 5_000_000).unwrap();
    assert_eq!((u.rows, u.work, u.jobs, u.elapsed_ms, u.ended), (3, 1893, 1, 5, false));
    assert_eq!(s.take(1, job), Err(Error::NotFound), "an output is taken once");
}

#[test]
fn a_session_answers_only_its_badge() {
    let mut s = loaded(&[1, 2]);
    let first = s.open(1, &id(), budget(), 0).unwrap();
    let job = s.submit(1, first, 3, &read("input.f64le")).unwrap();
    assert_eq!(s.submit(2, first, 3, &read("input.f64le")), Err(Error::Owner));
    assert_eq!(s.usage(2, first, 0), Err(Error::Owner));
    assert_eq!(s.cancel(2, job), Err(Error::Owner));
    assert_eq!(s.take(2, job), Err(Error::Owner));
    assert_eq!(s.close(2, first), Err(Error::Owner));
    let second = s.open(2, &id(), budget(), 0).unwrap();
    let other = s.submit(2, second, 3, &read("input.f64le")).unwrap();
    run(&mut s, &mut clock());
    assert!(s.take(1, job).is_ok() && s.take(2, other).is_ok());
    assert_eq!(s.retire(1, 2), Err(Error::Owner), "only the shell retires");
    assert_eq!(s.retire_all(2), Err(Error::Owner));
}

#[test]
fn budgets_refuse_the_whole_request_first() {
    let mut s = loaded(&[1]);
    let input = read("input.f64le");
    let session = s.open(1, &id(), Budget { work: 1892, ..budget() }, 0).unwrap();
    assert_eq!(s.submit(1, session, 3, &input), Err(Error::Budget));
    let u = s.usage(1, session, 0).unwrap();
    assert_eq!((u.rows, u.work, u.jobs, u.ended), (0, 0, 0, false));
    let session = s.open(1, &id(), Budget { rows: 3, work: 1893, ..budget() }, 0).unwrap();
    let mut c = clock();
    let job = s.submit(1, session, 2, &input[..48]).unwrap();
    run(&mut s, &mut c);
    s.take(1, job).unwrap();
    assert_eq!(s.submit(1, session, 2, &input[..48]), Err(Error::Budget));
    let job = s.submit(1, session, 1, &input[..24]).unwrap();
    run(&mut s, &mut c);
    s.take(1, job).unwrap();
    assert_eq!(s.submit(1, session, 1, &input[..24]), Err(Error::Budget));
    let u = s.usage(1, session, 0).unwrap();
    assert_eq!((u.rows, u.work, u.jobs), (3, 1893, 2));
    // A malformed or non-finite row is refused before it runs; the session stays usable.
    assert_eq!(s.submit(1, session, 2, &input[..24]), Err(Error::Malformed));
    let fresh = s.open(1, &id(), budget(), 0).unwrap();
    assert_eq!(s.submit(1, fresh, 1, &le(&[f64::NAN, 0.0, 0.0])), Err(Error::Malformed));
    assert_eq!(s.submit(1, fresh, 257, &le(&vec![0.2; 257 * 3])), Err(Error::TooLarge));
    assert!(!s.usage(1, fresh, 0).unwrap().ended);
}

#[test]
fn a_numeric_failure_poisons_its_session_only() {
    let mut s = loaded(&[1]);
    let session = s.open(1, &id(), budget(), 0).unwrap();
    let job = s.submit(1, session, 1, &read("numeric-input.f64le")).unwrap();
    run(&mut s, &mut clock());
    assert_eq!(s.take(1, job), Err(Error::Numeric));
    let u = s.usage(1, session, 0).unwrap();
    assert_eq!((u.rows, u.work, u.jobs, u.ended), (1, 39, 1, true));
    assert_eq!(s.submit(1, session, 3, &read("input.f64le")), Err(Error::Poisoned));
    let fresh = s.open(1, &id(), budget(), 0).unwrap();
    let job = s.submit(1, fresh, 3, &read("input.f64le")).unwrap();
    run(&mut s, &mut clock());
    assert_eq!(s.take(1, job).unwrap().len(), 6);
}

#[test]
fn cancellation_and_the_deadline_end_the_job_and_the_session() {
    let mut s = loaded(&[1]);
    // Cancelled at the 20th checkpoint: two rows started and charged, the output cleared.
    let session = s.open(1, &id(), budget(), 0).unwrap();
    let job = s.submit(1, session, 3, &read("input.f64le")).unwrap();
    run(&mut s, &mut Clock { tick: 0, cancel_at: Some(20) });
    assert_eq!(s.take(1, job), Err(Error::Cancelled));
    let u = s.usage(1, session, 0).unwrap();
    assert_eq!((u.rows, u.work, u.jobs, u.ended), (2, 646, 1, true));
    assert_eq!(s.submit(1, session, 3, &read("input.f64le")), Err(Error::Poisoned));
    // Cancelled before it ran: nothing charged, the session ended.
    let session = s.open(1, &id(), budget(), 0).unwrap();
    let job = s.submit(1, session, 3, &read("input.f64le")).unwrap();
    s.cancel(1, job).unwrap();
    assert_eq!(s.take(1, job), Err(Error::Cancelled));
    assert_eq!(s.usage(1, session, 0).unwrap().rows, 0);
    s.close(1, session).unwrap();
    // A deadline of 20 ms from an open at 1 s: the clock of nanoseconds passes it at once.
    let session = s.open(1, &id(), Budget { deadline_ms: 20, ..budget() }, 1_000_000_000).unwrap();
    assert_eq!(s.sessions.iter().find(|x| x.id == session).unwrap().deadline, 1_020_000_000);
    let job = s.submit(1, session, 3, &read("input.f64le")).unwrap();
    run(&mut s, &mut Clock { tick: 1_020_000_000, cancel_at: None });
    assert_eq!(s.take(1, job), Err(Error::Deadline));
    // No deadline is the tick u64::MAX; a deadline that would overflow is refused.
    let session = s.open(1, &id(), budget(), u64::MAX - 1).unwrap();
    assert_eq!(s.sessions.iter().find(|x| x.id == session).unwrap().deadline, u64::MAX);
    assert_eq!(s.open(1, &id(), Budget { deadline_ms: u64::MAX, ..budget() }, 0), Err(Error::TooLarge));
}

#[test]
fn a_retired_badge_leaves_nothing_for_its_next_holder() {
    let mut s = loaded(&[SHELL, 7]);
    let session = s.open(7, &id(), budget(), 0).unwrap();
    let queued = s.submit(7, session, 3, &read("input.f64le")).unwrap();
    assert_eq!(s.badges(), 2);
    s.retire(SHELL, 7).unwrap();
    // The badge's next holder: the old session and job are not found; nor is its hold.
    assert_eq!(s.usage(7, session, 0), Err(Error::NotFound));
    assert_eq!(s.take(7, queued), Err(Error::NotFound));
    assert_eq!(s.unload(7, &id()), Err(Error::NotFound));
    assert_eq!(s.badges(), 1);
    // A job running at the retirement: dropped when its worker answers, never seen by the new holder.
    let session = s.open(7, &id(), budget(), 0).unwrap();
    let job = s.submit(7, session, 3, &read("input.f64le")).unwrap();
    let r = s.start().unwrap();
    s.retire(SHELL, 7).unwrap();
    assert_eq!(s.take(7, job), Err(Error::NotFound));
    assert!(s.jobs.iter().any(|j| j.id == job && j.gone && j.cancel));
    let input = s.jobs.iter().find(|j| j.id == r.job).unwrap().input.clone();
    let outcome = execute(&s.model(&id()).unwrap().package, &r, &input, &mut clock());
    s.finish(r.job, outcome);
    assert!(s.jobs.is_empty() && s.sessions.is_empty());
    s.retire_all(SHELL).unwrap();
    assert!(s.models.is_empty() && s.used == 0, "every hold dropped, the memory back");
}

#[test]
fn memory_comes_back_and_each_badge_is_bounded() {
    let mut s = loaded(&[1, 2]);
    let held = s.used;
    assert_eq!(held, model().held_bytes(), "one copy for two holders");
    let session = s.open(1, &id(), budget(), 0).unwrap();
    let job = s.submit(1, session, 3, &read("input.f64le")).unwrap();
    run(&mut s, &mut clock());
    s.take(1, job).unwrap();
    s.close(1, session).unwrap();
    assert_eq!(s.used, held);
    s.unload(1, &id()).unwrap();
    assert_eq!(s.used, held, "badge 2 still holds it");
    s.unload(2, &id()).unwrap();
    assert_eq!(s.used, 0);
    // A badge has a quarter of the quota; the shell all of it.
    let mut s = Service::new(4 * model().held_bytes() + 8 * MIB, 4);
    s.insert(3, model()).unwrap();
    assert_eq!(s.open(3, &id(), Budget { memory: 4 * MIB, ..budget() }, 0), Err(Error::NoMemory));
    assert!(s.open(SHELL, &id(), Budget { memory: 4 * MIB, ..budget() }, 0).is_ok());
    // Four sessions and four jobs a badge.
    let mut s = loaded(&[1]);
    let sessions: Vec<u32> = (0..4).map(|_| s.open(1, &id(), budget(), 0).unwrap()).collect();
    assert_eq!(s.open(1, &id(), budget(), 0), Err(Error::Busy));
    for _ in 0..4 { s.submit(1, sessions[0], 1, &read("input.f64le")[..24]).unwrap(); }
    assert_eq!(s.submit(1, sessions[1], 1, &read("input.f64le")[..24]), Err(Error::Busy));
    // No workers: nothing parsed or run.
    let s = Service::new(64 * MIB, 0);
    assert_eq!(s.may_load(1, &id()).err(), Some(Error::NoWorkers));
}

#[test]
fn a_clock_that_went_back_between_jobs_poisons_the_session() {
    let mut s = loaded(&[1]);
    let session = s.open(1, &id(), budget(), 0).unwrap();
    let job = s.submit(1, session, 1, &read("input.f64le")[..24]).unwrap();
    run(&mut s, &mut Clock { tick: 1000, cancel_at: None });
    s.take(1, job).unwrap();
    let job = s.submit(1, session, 1, &read("input.f64le")[..24]).unwrap();
    run(&mut s, &mut clock());
    assert_eq!(s.take(1, job), Err(Error::Failed));
    assert!(s.usage(1, session, 0).unwrap().ended);
}

#[test]
fn a_worker_that_ends_without_answering_poisons_its_session() {
    let mut s = loaded(&[1]);
    let session = s.open(1, &id(), budget(), 0).unwrap();
    let job = s.submit(1, session, 3, &read("input.f64le")).unwrap();
    let r = s.start().unwrap();
    assert!(s.jobs.iter().any(|j| j.state == State::Running));
    s.lost(r.job);
    assert_eq!(s.take(1, job), Err(Error::Failed));
    assert!(s.usage(1, session, 0).unwrap().ended);
    assert_eq!(s.workers, 0);
}
