#![no_std]
#![no_main]
// The model service (262-APP-0112, docs/nn/runtime.md 1.0): it loads checked `nrl.model/0` packages read-only and runs
// them for its clients, each known by the badge the kernel stamps on its messages (idl/nn.wit). A package is parsed in
// a worker that holds only its bytes; nnd reads the answer's layout, checks the model ID and encodes each object again
// against its file, so it trusts no text (MC-11.11). A job runs in a worker of its own that holds the model read-only
// and the job's memory, whose first words carry the cancel flag and the answer; nnd learns its end from the exit
// notice. A boot service of init (262-KRN-0178): `--memory <MiB>` is the quota init gives it from `data/memory.conf`,
// SPAWN_WORKERS its right to start workers; without that right it parses and runs nothing (`no-workers`). A load holds
// the service until its parsers end.
extern crate alloc;

use alloc::vec::Vec;
use mind::abi::BootInfo;
use mind::idl::codec::{List, Text};
use mind::idl::nn::{self, Request};
use mind::idl::wire::{self, Call};
use mind::ipc::{self, Endpoint, Received};
use mind::mem::{Mapping, Pages};
use nnpkg::package::{self, Package};
use nnpkg::service::{self, Budget, Error, Outcome, Run, Service, State};
use nnpkg::Refusal;
use nrl_runtime::{Checkpoint, Control, Status};

// No screen: run by hand (the shell, wm's menu) it says in a console that it is init's service, and ends.
mind::request!(mind::process::REQUEST_CONSOLE);

const RECEIVED: usize = 9; // where a request's capability lands
const INPUT: usize = 9; // a worker's read-only bytes: the files to parse, or the model's description
const OUTPUT: usize = 10; // a worker's answer, or the job's memory
const PROBE: u64 = 1; // a worker's kind, the argument's low byte
const MANIFEST: u64 = 2;
const OBJECTS: u64 = 3;
const JOB: u64 = 4;
const MIB: u64 = 1 << 20;
const DEFAULT_MIB: u64 = 16; // without `--memory`
const WORKER_MIB: u64 = 4; // what a live worker may take of the quota: its image, stack and heap
const PARSE_MS: u32 = 60_000; // a parser's longest run
const PAGE: usize = 4096;

// The job memory's words: the answer and the cancel flag first, then what the worker runs, the input, the output.
const END: usize = 0; // the worker's last write: 1 once the answer is whole
const CANCEL: usize = 1;
const RESULT: usize = 2; // 0, or the runtime's error + 1
const STATUS: usize = 3;
const ROWS: usize = 4;
const WORK: usize = 5;
const REQUESTS: usize = 6;
const FIRST_TICK: usize = 7;
const LAST_TICK: usize = 8;
const OWNER: usize = 9;
const BUDGET: usize = 10; // rows, work, workspace, output, requests, deadline
const OUTPUT_VALUES: usize = 16;
const INPUT_VALUES: usize = 17;
const DESCRIPTION: usize = 18;
const MODEL: usize = 19; // 4 words
const HEADER: usize = 256;

const ERRORS: [nrl_runtime::Error; 11] = {
    use nrl_runtime::Error::*;
    [Profile, Graph, Shape, Limit, Owner, Model, Closed, Cancelled, Deadline, Clock, Numeric]
};

static mut SCRATCH: [u8; nn::REQUEST_MAX] = [0; nn::REQUEST_MAX];

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("nnd — the model service: checked nrl.model/0 packages loaded read-only and run for programs within each session's budget (idl/nn.wit, docs/nn/runtime.md).\nA boot service of init (262-KRN-0178): run from the shell it has no endpoint to serve and ends.");
    match mind::process::worker() {
        Some((_, argument)) => match argument & 0xFF {
            MANIFEST | OBJECTS => parse(argument),
            JOB => job(),
            _ => {}
        },
        None => serve(info.cpu_count.max(1) as u32),
    }
}

// ---- the parser worker: INPUT's bytes in, the layout of their typed description in OUTPUT ----

// OUTPUT: a status word (0, or the refusal's kind + 1), the answer's length, then the answer (the layout, or the reason).
fn parse(argument: u64) {
    let (Ok(input), Ok(mut output)) = (Mapping::new(INPUT), Mapping::new(OUTPUT)) else { return };
    let Some(bytes) = input.as_slice().get(..(argument >> 8) as usize) else { return };
    let answer = if argument & 0xFF == MANIFEST { package::parse_manifest(bytes) } else { objects(bytes) };
    let out = output.as_mut_slice();
    let (status, body): (u64, &[u8]) = match &answer { Ok(layout) => (0, layout), Err(r) => (kind(*r) + 1, r.reason().as_bytes()) };
    let (status, body) = if 16 + body.len() > out.len() { (kind(Refusal::TooLarge("")) + 1, &b"THE ANSWER IS LARGER THAN ITS ROOM"[..]) } else { (status, body) };
    out[16..16 + body.len()].copy_from_slice(body);
    out[8..16].copy_from_slice(&(body.len() as u64).to_le_bytes());
    out[..8].copy_from_slice(&status.to_le_bytes());
}

// Object files after a table: their count, then each length (u32 each); the answer: each layout after its length.
fn objects(bytes: &[u8]) -> Result<Vec<u8>, Refusal> {
    let broken = Refusal::Malformed("THE OBJECT FILES' TABLE IS BROKEN");
    let word = |at: usize| bytes.get(at..at + 4).map(|w| u32::from_le_bytes(w.try_into().unwrap()) as usize).ok_or(broken);
    let count = word(0)?;
    if count > nnpkg::check::MAX_OBJECTS { return Err(broken); }
    let mut at = 4 + 4 * count;
    let mut answer = Vec::new();
    for i in 0..count {
        let length = word(4 + 4 * i)?;
        let file = bytes.get(at..at + length).ok_or(broken)?;
        let layout = package::parse_object(file)?;
        answer.extend_from_slice(&(layout.len() as u64).to_le_bytes());
        answer.extend_from_slice(&layout);
        at += length;
    }
    Ok(answer)
}

fn kind(r: Refusal) -> u64 {
    match r { Refusal::NotFound(_) => 0, Refusal::Digest(_) => 1, Refusal::Malformed(_) => 2, Refusal::Unsupported(_) => 3, Refusal::TooLarge(_) => 4 }
}

// ---- the job worker: the model's description and the job's memory ----

// The checkpoints of `nrl-runtime`: the monotonic clock in nanoseconds, and the job's cancel flag.
struct Clock(*const u64);
impl Control for Clock {
    fn checkpoint(&mut self) -> Checkpoint { Checkpoint { tick: mind::time::monotonic_ns(), cancelled: unsafe { self.0.read_volatile() } != 0 } }
}

fn job() {
    let (Ok(model), Ok(memory)) = (Mapping::new(INPUT), Mapping::new(OUTPUT)) else { return };
    let words = memory.as_ptr::<u64>();
    let read = |i: usize| unsafe { words.add(i).read_volatile() };
    let write = |i: usize, v: u64| unsafe { words.add(i).write_volatile(v) };
    let Some(description) = model.as_slice().get(..read(DESCRIPTION) as usize) else { return };
    let Ok(package) = Package::from_description(description) else { return };
    let mut id = [0u8; 32];
    for (i, chunk) in id.chunks_exact_mut(8).enumerate() { chunk.copy_from_slice(&read(MODEL + i).to_le_bytes()); }
    let (inputs, outputs) = (read(INPUT_VALUES) as usize, read(OUTPUT_VALUES) as usize);
    if HEADER + (inputs + outputs) * 8 > memory.len() || id != package.id { return; }
    let values = unsafe { core::slice::from_raw_parts_mut(memory.as_ptr::<u8>().add(HEADER) as *mut f64, inputs + outputs) };
    let input: Vec<f64> = values[..inputs].to_vec();
    let run = Run {
        job: 0,
        model: id,
        owner: read(OWNER),
        budget: nrl_runtime::Budget { max_rows: read(BUDGET), max_work_units: read(BUDGET + 1), max_workspace_values: read(BUDGET + 2) as usize, max_output_values: read(BUDGET + 3) as usize, max_requests: read(BUDGET + 4), deadline_tick: read(BUDGET + 5) },
        output_values: outputs,
    };
    let outcome = service::execute(&package, &run, &input, &mut Clock(unsafe { words.add(CANCEL) }));
    if outcome.output.len() == outputs { values[inputs..].copy_from_slice(&outcome.output); }
    write(RESULT, outcome.result.err().map_or(0, |e| ERRORS.iter().position(|x| *x == e).unwrap_or(0) as u64 + 1));
    write(STATUS, match outcome.status { Status::Ready => 0, Status::Cancelled => 1, Status::Failed => 2 });
    write(ROWS, outcome.usage.rows);
    write(WORK, outcome.usage.work_units);
    write(REQUESTS, outcome.usage.requests);
    write(FIRST_TICK, outcome.first_tick);
    write(LAST_TICK, outcome.last_tick);
    write(END, 1);
}

// ---- the service ----

// A model's description, sealed read-only, for its jobs' workers.
struct Sealed { id: [u8; 32], cap: usize, len: usize }
// A job's worker and its memory.
struct Worker { job: u32, pid: u64, memory: Pages }
// A `wait` answered when its job ends or its time is up.
struct Waiter { job: u32, badge: u16, until_ns: u64, call: Call }

struct Nnd { service: Service, sealed: Vec<Sealed>, workers: Vec<Worker>, waiters: Vec<Waiter> }

fn arg(name: &str) -> Option<u64> {
    let mut words = mind::process::args_str().split_whitespace();
    while let Some(w) = words.next() { if w == name { return words.next()?.parse().ok(); } }
    None
}

fn error(e: Error) -> nn::Error {
    match e {
        Error::NotFound => nn::Error::NotFound, Error::Digest => nn::Error::Digest, Error::Malformed => nn::Error::Malformed, Error::Unsupported => nn::Error::Unsupported,
        Error::TooLarge => nn::Error::TooLarge, Error::NoMemory => nn::Error::NoMemory, Error::NoWorkers => nn::Error::NoWorkers, Error::Owner => nn::Error::Owner,
        Error::Budget => nn::Error::Budget, Error::Busy => nn::Error::Busy, Error::Pending => nn::Error::Pending, Error::Cancelled => nn::Error::Cancelled,
        Error::Deadline => nn::Error::Deadline, Error::Numeric => nn::Error::Numeric, Error::Poisoned => nn::Error::Poisoned, Error::Failed => nn::Error::Failed,
    }
}

fn refused(r: Refusal) -> Error {
    match r { Refusal::NotFound(_) => Error::NotFound, Refusal::Digest(_) => Error::Digest, Refusal::Malformed(_) => Error::Malformed, Refusal::Unsupported(_) => Error::Unsupported, Refusal::TooLarge(_) => Error::TooLarge }
}

// Whether a worker may be started: a probe that ends at once.
fn may_start_workers() -> bool {
    let Ok(notices) = Endpoint::create() else { return false };
    let started = mind::process::start_worker(PROBE, 0, &[]).map(|pid| { let _ = mind::process::watch(pid, notices); });
    if started.is_ok() { let _ = notices.recv_timeout(0, 5_000); }
    let _ = ipc::drop_cap(notices.0);
    started.is_ok()
}

fn serve(cpus: u32) {
    let quota = arg("--memory").unwrap_or(DEFAULT_MIB) * MIB;
    let workers_max = if may_start_workers() { cpus } else { 0 };
    let reserved = workers_max as u64 * WORKER_MIB * MIB + 2 * MIB;
    let mut nnd = Nnd { service: Service::new(quota.saturating_sub(reserved), workers_max), sealed: Vec::new(), workers: Vec::new(), waiters: Vec::new() };
    mind::println!("[NND] READY: QUOTA {} MIB, {} WORKERS AT MOST{}", quota / MIB, workers_max, if workers_max == 0 { " (NO SPAWN_WORKERS: NOTHING PARSED OR RUN)" } else { "" });
    let scratch = unsafe { &mut *core::ptr::addr_of_mut!(SCRATCH) };
    loop {
        let now = mind::time::monotonic_ns();
        let wait_ms = nnd.waiters.iter().map(|w| w.until_ns.saturating_sub(now) / 1_000_000 + 1).min().unwrap_or(60_000).min(60_000) as u32;
        match Endpoint::SERVICE.recv_timeout(RECEIVED, wait_ms) {
            Ok(request) if request.exit.is_some() => nnd.ended(request.exit.unwrap().pid),
            Ok(request) if request.is_call => nnd.answer(&request, scratch),
            Ok(request) if request.cap_received => { let _ = ipc::drop_cap(RECEIVED); }
            Err(mind::sys::Error::Rights | mind::sys::Error::NotFound | mind::sys::Error::Invalid) => {
                return mind::println!("[NND] NO ENDPOINT TO SERVE: NND IS A BOOT SERVICE OF INIT");
            }
            _ => {}
        }
        nnd.start_jobs();
        nnd.answer_waiters();
    }
}

impl Nnd {
    fn answer(&mut self, request: &Received, scratch: &mut [u8; nn::REQUEST_MAX]) {
        let badge = request.badge;
        let now = mind::time::monotonic_ns();
        let (request, call) = match nn::decode(request, RECEIVED, scratch) { Ok(decoded) => decoded, Err(reason) => { let _ = wire::reject(reason); return; } };
        let s = &mut self.service;
        let _ = match request {
            Request::Load { path, id } => {
                let id = id32(id.as_slice());
                let model = self.load(badge, path.as_str(), id).map(|_| model_record(&self.service.model(&id).unwrap().package, &self.service));
                nn::reply_load(call, model.as_ref().map_err(|e| error(*e)))
            }
            Request::Models { start } => {
                let list: Vec<nn::Model> = self.service.models.iter().skip(start as usize).take(8).map(|m| model_record(&m.package, &self.service)).collect();
                nn::reply_models(call, &list)
            }
            Request::Unload { id } => { let r = s.unload(badge, &id32(id.as_slice())); self.forget_unused(); nn::reply_unload(call, r.map_err(error)) }
            Request::Open { id, budget } => {
                let b = Budget { rows: budget.rows, work: budget.work, jobs: budget.jobs, memory: budget.memory, deadline_ms: budget.deadline_ms };
                nn::reply_open(call, s.open(badge, &id32(id.as_slice()), b, now).map_err(error))
            }
            Request::Submit { session, rows, input } => nn::reply_submit(call, s.submit(badge, session, rows, input).map_err(error)),
            Request::Wait { job, wait_ms } => match s.take(badge, job) {
                Err(Error::Pending) if wait_ms > 0 => {
                    let mut call = call;
                    if call.defer().is_ok() { self.waiters.push(Waiter { job, badge, until_ns: now + wait_ms as u64 * 1_000_000, call }); Ok(()) } else { nn::reply_wait(call, Err(nn::Error::Pending)) }
                }
                taken => reply_output(call, taken),
            },
            Request::Cancel { job } => {
                let r = s.cancel(badge, job);
                if r.is_ok() { self.flag_cancel(job); }
                nn::reply_cancel(call, r.map_err(error))
            }
            Request::Usage { session } => {
                let u = s.usage(badge, session, now).map(|u| nn::Usage { rows: u.rows, work: u.work, jobs: u.jobs, memory: u.memory, elapsed_ms: u.elapsed_ms, ended: u.ended });
                nn::reply_usage(call, u.as_ref().map_err(|e| error(*e)))
            }
            Request::Close { session } => { let r = s.close(badge, session); self.flag_gone(); self.forget_unused(); nn::reply_close(call, r.map_err(error)) }
            Request::Status => {
                let status = nn::Status { quota: s.quota, used: s.used, workers: s.workers, workers_max: s.workers_max, models: s.models.len() as u32, sessions: s.sessions.len() as u32, badges: s.badges() as u32 };
                nn::reply_status(call, &status)
            }
            Request::Retire { badge: retired } => {
                let r = s.retire(badge, retired);
                if r.is_ok() { self.flag_gone(); self.forget_unused(); mind::println!("[NND] BADGE {} RETIRED", retired); }
                nn::reply_retire(call, r.map_err(error))
            }
            Request::RetireAll => {
                let r = s.retire_all(badge);
                if r.is_ok() { self.flag_gone(); self.forget_unused(); }
                nn::reply_retire_all(call, r.map_err(error))
            }
        };
    }

    // The workers of jobs whose session ended are told to stop.
    fn flag_gone(&mut self) {
        let gone: Vec<u32> = self.service.jobs.iter().filter(|j| j.gone).map(|j| j.id).collect();
        for job in gone { self.flag_cancel(job); }
    }

    fn flag_cancel(&mut self, job: u32) {
        if let Some(w) = self.workers.iter_mut().find(|w| w.job == job) { unsafe { (w.memory.address() as *mut u64).add(CANCEL).write_volatile(1) } }
    }

    // A model no longer loaded: its sealed description dropped.
    fn forget_unused(&mut self) {
        let service = &self.service;
        self.sealed.retain(|s| { let keep = service.model(&s.id).is_some(); if !keep { let _ = ipc::drop_cap(s.cap); } keep });
    }

    /// Reads, parses (in workers) and checks the package at `path`, for `badge`.
    fn load(&mut self, badge: u16, path: &str, id: [u8; 32]) -> Result<(), Error> {
        if self.service.may_load(badge, &id)?.is_some() { return Ok(()); }
        let allowed = path.starts_with("models:") || path.starts_with("data/models/");
        if !allowed || path.split('/').any(|part| part == ".." || part == ".") { return Err(Error::NotFound); }
        let fail = |r: Refusal| { mind::println!("[NND] LOAD {} REFUSED FOR BADGE {}: {}", path, badge, r.reason()); refused(r) };
        let mut name = alloc::string::String::from(path);
        name.push_str("/manifest.json");
        let (manifest_pages, manifest_len) = read_file(&name, nnpkg::check::MAX_MANIFEST_BYTES)?;
        let answer = run_parser(MANIFEST, &manifest_pages, manifest_len).map_err(fail)?;
        let m = package::read_manifest(&answer, &id).map_err(fail)?;
        drop(manifest_pages);
        // The object files in one block after a table of their lengths, each hashed by nnd before the parser sees it.
        let mut total = manifest_len;
        for (index, (key, entry)) in m.objects.iter().enumerate() { total = package::entry_bound(index, key, entry, total).map_err(fail)?; }
        let table = 4 + 4 * m.objects.len();
        let length = table + m.objects.iter().map(|(_, e)| e.bytes).sum::<usize>();
        let mut all = Pages::new(length.next_multiple_of(PAGE)).ok_or(Error::NoMemory)?;
        let mut at = table;
        all.as_mut_slice()[..4].copy_from_slice(&(m.objects.len() as u32).to_le_bytes());
        for (i, (_, entry)) in m.objects.iter().enumerate() {
            all.as_mut_slice()[4 + 4 * i..8 + 4 * i].copy_from_slice(&(entry.bytes as u32).to_le_bytes());
            let mut name = alloc::string::String::from(path);
            name.push('/');
            name.push_str(&entry.file);
            let file = &mut all.as_mut_slice()[at..at + entry.bytes];
            read_into(&name, file)?;
            package::vet_file(file, entry).map_err(fail)?;
            at += entry.bytes;
        }
        let answer = run_parser(OBJECTS, &all, length).map_err(fail)?;
        let mut objects = Vec::new();
        let (mut at, mut file_at) = (0, table);
        for (key, entry) in &m.objects {
            let broken = || fail(Refusal::Malformed("THE PARSER'S ANSWER IS NOT IN THE LAYOUT"));
            let size = answer.get(at..at + 8).map(|w| u64::from_le_bytes(w.try_into().unwrap()) as usize).ok_or_else(broken)?;
            let layout = answer.get(at + 8..(at + 8).saturating_add(size)).ok_or_else(broken)?;
            objects.push((key.clone(), package::read_object(layout, &all.as_slice()[file_at..file_at + entry.bytes], entry).map_err(fail)?));
            at += 8 + size;
            file_at += entry.bytes;
        }
        drop(all);
        let checked = package::assemble(m, objects, id, total).map_err(fail)?;
        let description = checked.describe();
        let cap = mind::mem::sealed_copy(&description).map_err(|_| Error::NoMemory)?;
        if let Err(e) = self.service.insert(badge, checked) { let _ = ipc::drop_cap(cap); return Err(e); }
        self.sealed.push(Sealed { id, cap, len: description.len() });
        mind::println!("[NND] LOADED {} FOR BADGE {}: {} BYTES", path, badge, total);
        Ok(())
    }

    // Queued jobs on free workers.
    fn start_jobs(&mut self) {
        while let Some(run) = self.service.start() {
            match self.spawn(&run) {
                Ok(worker) => self.workers.push(worker),
                Err(mind::sys::Error::Other(_)) => { self.service.requeue(run.job); break; }
                Err(_) => self.service.lost(run.job),
            }
        }
    }

    fn spawn(&mut self, run: &Run) -> mind::sys::Result<Worker> {
        let sealed = self.sealed.iter().find(|s| s.id == run.model).ok_or(mind::sys::Error::NotFound)?;
        let job = self.service.jobs.iter().find(|j| j.id == run.job).ok_or(mind::sys::Error::NotFound)?;
        let bytes = HEADER + (job.input.len() + run.output_values) * 8;
        let mut memory = Pages::new(bytes.next_multiple_of(PAGE)).ok_or(mind::sys::Error::NoMemory)?;
        {
            let words = memory.address() as *mut u64;
            let write = |i: usize, v: u64| unsafe { words.add(i).write_volatile(v) };
            let b = &run.budget;
            write(OWNER, run.owner);
            for (i, v) in [b.max_rows, b.max_work_units, b.max_workspace_values as u64, b.max_output_values as u64, b.max_requests, b.deadline_tick].into_iter().enumerate() { write(BUDGET + i, v); }
            write(OUTPUT_VALUES, run.output_values as u64);
            write(INPUT_VALUES, job.input.len() as u64);
            write(DESCRIPTION, sealed.len as u64);
            for (i, chunk) in run.model.chunks_exact(8).enumerate() { write(MODEL + i, u64::from_le_bytes(chunk.try_into().unwrap())); }
            let values = &mut memory.as_mut_slice()[HEADER..HEADER + job.input.len() * 8];
            for (out, v) in values.chunks_exact_mut(8).zip(&job.input) { out.copy_from_slice(&v.to_le_bytes()); }
        }
        let shared = memory.share()?;
        let model = ipc::mint(sealed.cap, mind::abi::CAP_READ, 0, 0)?;
        let grants = [mind::process::grant(INPUT, model, u8::MAX), mind::process::grant(OUTPUT, shared, u8::MAX)];
        let started = mind::process::start_worker(JOB, 0, &grants);
        let _ = ipc::drop_cap(model);
        let _ = ipc::drop_cap(shared);
        let pid = started?;
        mind::process::watch(pid, Endpoint::SERVICE)?;
        Ok(Worker { job: run.job, pid, memory })
    }

    // A worker's exit notice: its answer read from the job's memory, or the job lost.
    fn ended(&mut self, pid: u64) {
        let Some(at) = self.workers.iter().position(|w| w.pid == pid) else { return };
        let w = self.workers.remove(at);
        let words = w.memory.address() as *const u64;
        let read = |i: usize| unsafe { words.add(i).read_volatile() };
        if read(END) != 1 { return self.service.lost(w.job); }
        let result = match read(RESULT) { 0 => Ok(()), n => Err(*ERRORS.get(n as usize - 1).unwrap_or(&nrl_runtime::Error::Profile)) };
        let (inputs, outputs) = (read(INPUT_VALUES) as usize, read(OUTPUT_VALUES) as usize);
        let output = if result.is_ok() { w.memory.as_slice()[HEADER + inputs * 8..HEADER + (inputs + outputs) * 8].chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect() } else { Vec::new() };
        let status = match read(STATUS) { 0 => Status::Ready, 1 => Status::Cancelled, _ => Status::Failed };
        let usage = nrl_runtime::Usage { rows: read(ROWS), work_units: read(WORK), requests: read(REQUESTS) };
        self.service.finish(w.job, Outcome { result, status, usage, output, first_tick: read(FIRST_TICK), last_tick: read(LAST_TICK) });
        self.forget_unused();
    }

    // Waits whose job ended or whose time is up.
    fn answer_waiters(&mut self) {
        let now = mind::time::monotonic_ns();
        let mut i = 0;
        while i < self.waiters.len() {
            let w = &self.waiters[i];
            let ended = self.service.jobs.iter().find(|j| j.id == w.job).is_none_or(|j| !matches!(j.state, State::Queued | State::Running));
            if ended || now >= w.until_ns {
                let w = self.waiters.remove(i);
                let _ = reply_output(w.call, self.service.take(w.badge, w.job));
            } else {
                i += 1;
            }
        }
    }
}

fn reply_output(call: Call, taken: Result<Vec<f64>, Error>) -> mind::sys::Result<()> {
    match taken {
        Ok(values) => { let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect(); nn::reply_wait(call, Ok(&bytes)) }
        Err(e) => nn::reply_wait(call, Err(error(e))),
    }
}

// The first `n` bytes of `s` or fewer, at a character's boundary.
fn cut(s: &str, n: usize) -> &str { let mut end = s.len().min(n); while !s.is_char_boundary(end) { end -= 1; } &s[..end] }

fn id32(bytes: &[u8]) -> [u8; 32] { let mut id = [0; 32]; let n = bytes.len().min(32); id[..n].copy_from_slice(&bytes[..n]); id }

fn model_record(p: &Package, service: &Service) -> nn::Model {
    let name = p.manifest.sources.first().map_or("", |s| s.identifier.as_str());
    let licence = p.manifest.sources.first().map_or("", |s| s.licence.as_str());
    nn::Model {
        id: List::from_slice(&p.id).unwrap_or_default(),
        name: Text::new(cut(name, 64)).unwrap_or_default(),
        inputs: p.input_width as u32,
        outputs: p.output_width as u32,
        rows_max: p.rows_max() as u32,
        bytes: p.held_bytes(),
        sessions: service.model(&p.id).map_or(0, |m| m.sessions),
        licence: Text::new(cut(licence, 32)).unwrap_or_default(),
    }
}

// A file of at most `limit` bytes in pages of its own.
fn read_file(path: &str, limit: usize) -> Result<(Pages, usize), Error> {
    let file = mind::fs::File::open(path).map_err(|_| Error::NotFound)?;
    let size = file.size();
    if size > limit { return Err(Error::TooLarge); }
    let mut pages = Pages::new(size.max(1).next_multiple_of(PAGE)).ok_or(Error::NoMemory)?;
    let mut at = 0;
    while at < size {
        let n = file.read_at(at, &mut pages.as_mut_slice()[at..size]).map_err(|_| Error::NotFound)?;
        if n == 0 { return Err(Error::NotFound); }
        at += n;
    }
    Ok((pages, size))
}

// A file that must be exactly `out`'s length, read into it.
fn read_into(path: &str, out: &mut [u8]) -> Result<(), Error> {
    let file = mind::fs::File::open(path).map_err(|_| Error::NotFound)?;
    if file.size() != out.len() { return Err(Error::Digest); }
    let mut at = 0;
    while at < out.len() {
        let n = file.read_at(at, &mut out[at..]).map_err(|_| Error::NotFound)?;
        if n == 0 { return Err(Error::NotFound); }
        at += n;
    }
    Ok(())
}

// A parser worker over `length` bytes of `bytes`: its answer, or why it refused.
fn run_parser(kind: u64, bytes: &Pages, length: usize) -> Result<Vec<u8>, Refusal> {
    let lost = Refusal::Malformed("THE PARSER ENDED WITHOUT AN ANSWER");
    let room = (2 * length + 64 * 1024).next_multiple_of(PAGE);
    let output = Pages::new(room).ok_or(Refusal::TooLarge("NO MEMORY FOR THE PARSER'S ANSWER"))?;
    let shared_out = output.share().map_err(|_| lost)?;
    let shared_in = bytes.share().map_err(|_| lost)?;
    let read_only = ipc::mint(shared_in, mind::abi::CAP_READ, 0, 0).map_err(|_| lost)?;
    let _ = ipc::drop_cap(shared_in);
    let notices = Endpoint::create().map_err(|_| lost)?;
    let grants = [mind::process::grant(INPUT, read_only, u8::MAX), mind::process::grant(OUTPUT, shared_out, u8::MAX)];
    let started = mind::process::start_worker(kind | (length as u64) << 8, 0, &grants);
    let _ = ipc::drop_cap(read_only);
    let _ = ipc::drop_cap(shared_out);
    let answered = started.ok().filter(|pid| mind::process::watch(*pid, notices).is_ok()).is_some_and(|_| notices.recv_timeout(0, PARSE_MS).is_ok());
    let _ = ipc::drop_cap(notices.0);
    if !answered { return Err(lost); }
    let out = output.as_slice();
    let status = u64::from_le_bytes(out[..8].try_into().unwrap());
    let len = (u64::from_le_bytes(out[8..16].try_into().unwrap()) as usize).min(out.len() - 16);
    let body = &out[16..16 + len];
    match status {
        0 => Ok(body.to_vec()),
        // The parser's reason is not trusted: a fixed one per kind.
        1 => Err(Refusal::NotFound("THE PARSER FOUND NO SUCH RECORD")),
        2 => Err(Refusal::Digest("THE PARSER REFUSED A DIGEST")),
        4 => Err(Refusal::Unsupported("THE PARSER FOUND A FAMILY, SCHEMA OR OPERATOR NOT RUN")),
        5 => Err(Refusal::TooLarge("THE PARSER FOUND A FILE PAST A BOUND")),
        _ => Err(Refusal::Malformed("THE PARSER REFUSED THE TEXT")),
    }
}
