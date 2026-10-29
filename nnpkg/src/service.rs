//! nnd's clients and what they hold (262-APP-0112, docs/nn/runtime.md sections 1, 4 and 5): models held by badges,
//! sessions bound to a badge and a model with their budgets, jobs, and the retirement of a badge before its reuse. Pure
//! state: nnd reads the files and starts the workers; the host tests run the jobs in place.
use crate::package::Package;
use alloc::vec::Vec;
use nrl_runtime::{Checkpoint, Control, Status};

/// The shell's badge: only it retires badges, and it may use the whole quota.
pub const SHELL: u16 = 0;
pub const SESSIONS_PER_BADGE: usize = 4;
pub const JOBS_PER_BADGE: usize = 4;

/// `nn.wit`'s errors, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error { NotFound, Digest, Malformed, Unsupported, TooLarge, NoMemory, NoWorkers, Owner, Budget, Busy, Pending, Cancelled, Deadline, Numeric, Poisoned, Failed }

/// A session's budget as `nn.wit` gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget { pub rows: u64, pub work: u64, pub jobs: u32, pub memory: u64, pub deadline_ms: u64 }

/// A session's use as `nn.wit` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage { pub rows: u64, pub work: u64, pub jobs: u32, pub memory: u64, pub elapsed_ms: u64, pub ended: bool }

pub struct Model { pub package: Package, pub holders: Vec<u16>, pub sessions: u32 }

pub struct Session {
    pub id: u32,
    pub owner: u16,
    pub model: [u8; 32],
    pub budget: Budget,
    /// What `nrl-runtime` charged: rows, work units, requests.
    pub used: nrl_runtime::Usage,
    pub ended: bool,
    /// Closed by its owner while a job ran: removed when the job ends.
    pub closed: bool,
    pub opened_ns: u64,
    /// The runtime's deadline tick: `u64::MAX` for none.
    pub deadline: u64,
    pub output_values: usize,
    pub last_tick: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum State { Queued, Running, Done(Vec<f64>), Failed(Error) }

/// `gone`: its session ended while it ran; no client sees it again, and it is dropped when it ends.
pub struct Job { pub id: u32, pub session: u32, pub owner: u16, pub rows: usize, pub input: Vec<f64>, pub state: State, pub cancel: bool, pub gone: bool, pub held: u64 }

/// What a worker runs: the session's remaining budget, so a fresh runtime session charges as the session would.
pub struct Run { pub job: u32, pub model: [u8; 32], pub owner: u64, pub budget: nrl_runtime::Budget, pub output_values: usize }

/// What a worker answers.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome { pub result: nrl_runtime::Result<()>, pub status: Status, pub usage: nrl_runtime::Usage, pub output: Vec<f64>, pub first_tick: u64, pub last_tick: u64 }

pub struct Service {
    pub quota: u64,
    pub used: u64,
    pub workers_max: u32,
    pub workers: u32,
    pub models: Vec<Model>,
    pub sessions: Vec<Session>,
    pub jobs: Vec<Job>,
    next_session: u32,
    next_job: u32,
}

fn next(counter: &mut u32) -> u32 { *counter = counter.wrapping_add(1).max(1); *counter }

impl Service {
    pub fn new(quota: u64, workers_max: u32) -> Self {
        Self { quota, used: 0, workers_max, workers: 0, models: Vec::new(), sessions: Vec::new(), jobs: Vec::new(), next_session: 0, next_job: 0 }
    }

    /// What a badge holds of the quota: the models it holds, its sessions' reservations and its jobs' inputs.
    pub fn share(&self, badge: u16) -> u64 {
        let models: u64 = self.models.iter().filter(|m| m.holders.contains(&badge)).map(|m| m.package.held_bytes()).sum();
        let sessions: u64 = self.sessions.iter().filter(|s| s.owner == badge).map(|s| s.budget.memory).sum();
        let jobs: u64 = self.jobs.iter().filter(|j| j.owner == badge).map(|j| j.held).sum();
        models + sessions + jobs
    }

    // `bytes` more for `badge`: within the quota and, but for the shell, a quarter of it.
    fn room(&self, badge: u16, bytes: u64) -> Result<(), Error> {
        let total = self.used.checked_add(bytes).ok_or(Error::NoMemory)?;
        let mine = self.share(badge).checked_add(bytes).ok_or(Error::NoMemory)?;
        if total > self.quota || (badge != SHELL && mine > self.quota / 4) { return Err(Error::NoMemory); }
        Ok(())
    }

    pub fn model(&self, id: &[u8; 32]) -> Option<&Model> { self.models.iter().find(|m| m.package.id == *id) }

    /// Whether `badge` may load: a worker parses, so none means `no-workers`; a model it holds already is answered.
    pub fn may_load(&self, badge: u16, id: &[u8; 32]) -> Result<Option<&Package>, Error> {
        if self.workers_max == 0 { return Err(Error::NoWorkers); }
        Ok(self.model(id).filter(|m| m.holders.contains(&badge)).map(|m| &m.package))
    }

    /// A checked package loaded for `badge`: one copy for every holder of its ID.
    pub fn insert(&mut self, badge: u16, package: Package) -> Result<&Package, Error> {
        if let Some(i) = self.models.iter().position(|m| m.package.id == package.id) {
            if !self.models[i].holders.contains(&badge) {
                if badge != SHELL && self.share(badge) + self.models[i].package.held_bytes() > self.quota / 4 { return Err(Error::NoMemory); }
                self.models[i].holders.push(badge);
            }
            return Ok(&self.models[i].package);
        }
        self.room(badge, package.held_bytes())?;
        self.used += package.held_bytes();
        self.models.push(Model { package, holders: alloc::vec![badge], sessions: 0 });
        Ok(&self.models.last().unwrap().package)
    }

    // A model no badge holds and no session uses is freed.
    fn collect(&mut self) {
        let used = &mut self.used;
        self.models.retain(|m| { let keep = !m.holders.is_empty() || m.sessions > 0; if !keep { *used -= m.package.held_bytes(); } keep });
    }

    pub fn unload(&mut self, badge: u16, id: &[u8; 32]) -> Result<(), Error> {
        let model = self.models.iter_mut().find(|m| m.package.id == *id).ok_or(Error::NotFound)?;
        let at = model.holders.iter().position(|b| *b == badge).ok_or(Error::NotFound)?;
        model.holders.remove(at);
        self.collect();
        Ok(())
    }

    /// A session of `badge` on a loaded model, opened at `now_ns`.
    pub fn open(&mut self, badge: u16, id: &[u8; 32], budget: Budget, now_ns: u64) -> Result<u32, Error> {
        if self.workers_max == 0 { return Err(Error::NoWorkers); }
        let package = &self.model(id).ok_or(Error::NotFound)?.package;
        if self.sessions.iter().filter(|s| s.owner == badge).count() >= SESSIONS_PER_BADGE { return Err(Error::Busy); }
        let workspace = package.workspace_values as u64 * 8;
        let least = workspace + package.output_width as u64 * 8;
        if budget.rows == 0 || budget.work == 0 || budget.jobs == 0 || budget.memory < least { return Err(Error::Budget); }
        let output_values = usize::try_from((budget.memory - workspace) / 8).map_err(|_| Error::TooLarge)?.min(nrl_runtime::MAX_WORKSPACE);
        if package.workspace_values > nrl_runtime::MAX_WORKSPACE { return Err(Error::TooLarge); }
        // deadline-ms 0 is none: the tick u64::MAX, since the runtime's tick 0 would end a job at once.
        let deadline = match budget.deadline_ms {
            0 => u64::MAX,
            ms => ms.checked_mul(1_000_000).and_then(|ns| now_ns.checked_add(ns)).filter(|t| *t != u64::MAX).ok_or(Error::TooLarge)?,
        };
        self.room(badge, budget.memory)?;
        self.used += budget.memory;
        let session = next(&mut self.next_session);
        self.models.iter_mut().find(|m| m.package.id == *id).unwrap().sessions += 1;
        self.sessions.push(Session { id: session, owner: badge, model: *id, budget, used: nrl_runtime::Usage { rows: 0, work_units: 0, requests: 0 }, ended: false, closed: false, opened_ns: now_ns, deadline, output_values, last_tick: 0 });
        Ok(session)
    }

    // The session `id` if `badge` owns it.
    fn session_mut(&mut self, badge: u16, id: u32) -> Result<&mut Session, Error> {
        let s = self.sessions.iter_mut().find(|s| s.id == id && !s.closed).ok_or(Error::NotFound)?;
        if s.owner != badge { return Err(Error::Owner); }
        Ok(s)
    }

    /// A job of `rows` rows of little-endian f64 on `badge`'s session, checked as the runtime would check it, then
    /// queued: nothing is charged by a refusal.
    pub fn submit(&mut self, badge: u16, session: u32, rows: u32, input: &[u8]) -> Result<u32, Error> {
        let s = self.sessions.iter().find(|s| s.id == session && !s.closed).ok_or(Error::NotFound)?;
        if s.owner != badge { return Err(Error::Owner); }
        if s.ended { return Err(Error::Poisoned); }
        let package = &self.model(&s.model).ok_or(Error::NotFound)?.package;
        let rows = rows as usize;
        if rows == 0 || input.len() != rows * package.input_width * 8 { return Err(Error::Malformed); }
        if rows > package.rows_max() || rows.checked_mul(package.values_per_row).is_none_or(|n| n > package.activations_max()) { return Err(Error::TooLarge); }
        let work = package.work_per_row.checked_mul(rows as u64).ok_or(Error::TooLarge)?;
        if rows * package.output_width > s.output_values || s.used.rows + rows as u64 > s.budget.rows || s.used.work_units + work > s.budget.work || s.used.requests >= s.budget.jobs as u64 {
            return Err(Error::Budget);
        }
        let values: Vec<f64> = input.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect();
        // A row that is not finite is refused before it runs; a finite one that overflows inside is `numeric`.
        if !values.iter().all(|v| v.is_finite()) { return Err(Error::Malformed); }
        if self.jobs.iter().filter(|j| j.owner == badge).count() >= JOBS_PER_BADGE { return Err(Error::Busy); }
        let held = input.len() as u64;
        self.room(badge, held)?;
        self.used += held;
        let id = next(&mut self.next_job);
        self.jobs.push(Job { id, session, owner: badge, rows, input: values, state: State::Queued, cancel: false, gone: false, held });
        Ok(id)
    }

    /// The next queued job to start on a worker, if a worker is free.
    pub fn start(&mut self) -> Option<Run> {
        if self.workers >= self.workers_max { return None; }
        let at = self.jobs.iter().position(|j| j.state == State::Queued)?;
        let job = &self.jobs[at];
        let s = self.sessions.iter().find(|s| s.id == job.session)?;
        let package = &self.model(&s.model)?.package;
        let run = Run {
            job: job.id,
            model: s.model,
            owner: s.owner as u64,
            budget: nrl_runtime::Budget {
                max_rows: s.budget.rows - s.used.rows,
                max_work_units: s.budget.work - s.used.work_units,
                max_workspace_values: package.workspace_values,
                max_output_values: s.output_values,
                max_requests: s.budget.jobs as u64 - s.used.requests,
                deadline_tick: s.deadline,
            },
            output_values: job.rows * package.output_width,
        };
        self.jobs[at].state = State::Running;
        self.workers += 1;
        Some(run)
    }

    /// A worker's answer for `job`: the session charged as the runtime charged, ended unless still ready.
    pub fn finish(&mut self, job: u32, outcome: Outcome) {
        self.workers = self.workers.saturating_sub(1);
        let Some(j) = self.jobs.iter_mut().find(|j| j.id == job) else { return };
        let Some(s) = self.sessions.iter_mut().find(|s| s.id == j.session) else { return };
        s.used.rows += outcome.usage.rows;
        s.used.work_units += outcome.usage.work_units;
        s.used.requests += outcome.usage.requests;
        let went_back = outcome.first_tick < s.last_tick;
        s.last_tick = s.last_tick.max(outcome.last_tick);
        let state = match outcome.result {
            _ if went_back => State::Failed(Error::Failed),
            Ok(()) => State::Done(outcome.output),
            Err(e) => State::Failed(match e {
                nrl_runtime::Error::Cancelled => Error::Cancelled,
                nrl_runtime::Error::Deadline => Error::Deadline,
                nrl_runtime::Error::Numeric if outcome.status == Status::Ready => Error::Malformed,
                nrl_runtime::Error::Numeric => Error::Numeric,
                nrl_runtime::Error::Closed => Error::Poisoned,
                nrl_runtime::Error::Limit => Error::Budget,
                nrl_runtime::Error::Shape => Error::Malformed,
                _ => Error::Failed,
            }),
        };
        if went_back || outcome.status != Status::Ready { s.ended = true; }
        j.state = state;
        let (closed, session) = (s.closed, s.id);
        if closed { self.drop_jobs(|j| j.session == session); self.drop_sessions(|s| s.id == session); }
    }

    /// A job whose worker could not start (every CPU's worker live): queued again.
    pub fn requeue(&mut self, job: u32) {
        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == job && j.state == State::Running) { j.state = State::Queued; self.workers = self.workers.saturating_sub(1); }
    }

    /// A worker that ended without answering: its session poisoned.
    pub fn lost(&mut self, job: u32) {
        let charged = nrl_runtime::Usage { rows: 0, work_units: 0, requests: 0 };
        self.finish(job, Outcome { result: Err(nrl_runtime::Error::Profile), status: Status::Failed, usage: charged, output: Vec::new(), first_tick: u64::MAX, last_tick: 0 });
    }

    /// Stops `badge`'s job: a queued one at once, a running one at its next checkpoint (`cancel` set for the worker).
    pub fn cancel(&mut self, badge: u16, job: u32) -> Result<(), Error> {
        let j = self.jobs.iter_mut().find(|j| j.id == job && !j.gone).ok_or(Error::NotFound)?;
        if j.owner != badge { return Err(Error::Owner); }
        j.cancel = true;
        if j.state == State::Queued {
            j.state = State::Failed(Error::Cancelled);
            let session = j.session;
            if let Some(s) = self.sessions.iter_mut().find(|s| s.id == session) { s.ended = true; }
        }
        Ok(())
    }

    /// `badge`'s job's end: its output (then forgotten, its input's memory freed), or why it failed; `pending` until it
    /// ends.
    pub fn take(&mut self, badge: u16, job: u32) -> Result<Vec<f64>, Error> {
        let at = self.jobs.iter().position(|j| j.id == job && !j.gone).ok_or(Error::NotFound)?;
        if self.jobs[at].owner != badge { return Err(Error::Owner); }
        if matches!(self.jobs[at].state, State::Queued | State::Running) { return Err(Error::Pending); }
        let j = self.jobs.remove(at);
        self.used -= j.held;
        match j.state { State::Done(output) => Ok(output), State::Failed(e) => Err(e), _ => unreachable!() }
    }

    pub fn usage(&self, badge: u16, session: u32, now_ns: u64) -> Result<Usage, Error> {
        let s = self.sessions.iter().find(|s| s.id == session && !s.closed).ok_or(Error::NotFound)?;
        if s.owner != badge { return Err(Error::Owner); }
        Ok(Usage { rows: s.used.rows, work: s.used.work_units, jobs: s.used.requests as u32, memory: s.budget.memory, elapsed_ms: now_ns.saturating_sub(s.opened_ns) / 1_000_000, ended: s.ended })
    }

    /// Ends `badge`'s session: its queued and finished jobs dropped, a running one stopped and dropped when it ends.
    pub fn close(&mut self, badge: u16, session: u32) -> Result<(), Error> {
        self.session_mut(badge, session)?;
        self.end_sessions(|s| s.id == session);
        Ok(())
    }

    // Sessions matching `which` ended: their idle jobs and they are dropped now; one with a running job when it ends.
    fn end_sessions(&mut self, which: impl Fn(&Session) -> bool) {
        let ids: Vec<u32> = self.sessions.iter().filter(|s| which(s)).map(|s| s.id).collect();
        for id in ids {
            self.drop_jobs(|j| j.session == id && j.state != State::Running);
            let running = self.jobs.iter_mut().filter(|j| j.session == id).map(|j| { j.cancel = true; j.gone = true; }).count();
            if running > 0 {
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) { s.closed = true; s.ended = true; }
            } else {
                self.drop_sessions(|s| s.id == id);
            }
        }
    }

    fn drop_jobs(&mut self, which: impl Fn(&Job) -> bool) {
        let used = &mut self.used;
        self.jobs.retain(|j| { let keep = !which(j); if !keep { *used -= j.held; } keep });
    }

    fn drop_sessions(&mut self, which: impl Fn(&Session) -> bool) {
        let mut freed = Vec::new();
        self.sessions.retain(|s| { let keep = !which(s); if !keep { freed.push((s.model, s.budget.memory)); } keep });
        for (model, memory) in freed {
            self.used -= memory;
            if let Some(m) = self.models.iter_mut().find(|m| m.package.id == model) { m.sessions -= 1; }
        }
        self.collect();
    }

    /// The shell's retirement of `badge`: its sessions, jobs and holds dropped before the badge is reused.
    pub fn retire(&mut self, caller: u16, badge: u16) -> Result<(), Error> {
        if caller != SHELL { return Err(Error::Owner); }
        self.end_sessions(|s| s.owner == badge);
        for m in &mut self.models { m.holders.retain(|b| *b != badge); }
        self.collect();
        Ok(())
    }

    /// The shell's retirement of every badge, at its start.
    pub fn retire_all(&mut self, caller: u16) -> Result<(), Error> {
        if caller != SHELL { return Err(Error::Owner); }
        self.end_sessions(|_| true);
        for m in &mut self.models { m.holders.clear(); }
        self.collect();
        Ok(())
    }

    /// Badges that hold a model, a session or a job.
    pub fn badges(&self) -> usize {
        let mut seen: Vec<u16> = self.models.iter().flat_map(|m| m.holders.iter().copied()).chain(self.sessions.iter().map(|s| s.owner)).chain(self.jobs.iter().map(|j| j.owner)).collect();
        seen.sort_unstable();
        seen.dedup();
        seen.len()
    }
}

/// A job run on `package` within `run`: what the worker does, with `control` its checkpoints.
pub fn execute(package: &Package, run: &Run, input: &[f64], control: &mut dyn Control) -> Outcome {
    let mut output = alloc::vec![0.0; run.output_values];
    let mut workspace = alloc::vec![0.0; package.workspace_values];
    let failed = |output: Vec<f64>| Outcome { result: Err(nrl_runtime::Error::Profile), status: Status::Failed, usage: nrl_runtime::Usage { rows: 0, work_units: 0, requests: 0 }, output, first_tick: 0, last_tick: 0 };
    let Ok(prepared) = crate::program::Prepared::new(package) else { return failed(Vec::new()) };
    let Ok(nodes) = prepared.operations() else { return failed(Vec::new()) };
    let Ok(program) = prepared.program(&nodes) else { return failed(Vec::new()) };
    let Ok(mut session) = nrl_runtime::Session::new(run.owner, run.model, run.budget) else { return failed(Vec::new()) };
    let mut ticks = Ticks { inner: control, first: None, last: 0 };
    let result = session.execute(run.owner, &program, input, &mut output, &mut workspace, &mut ticks);
    if result.is_err() { output.clear(); }
    Outcome { result, status: session.status(), usage: session.usage(), output, first_tick: ticks.first.unwrap_or(u64::MAX), last_tick: ticks.last }
}

// Keeps the first and last ticks a control gives.
struct Ticks<'c> { inner: &'c mut dyn Control, first: Option<u64>, last: u64 }
impl Control for Ticks<'_> {
    fn checkpoint(&mut self) -> Checkpoint { let c = self.inner.checkpoint(); self.first.get_or_insert(c.tick); self.last = c.tick; c }
}
