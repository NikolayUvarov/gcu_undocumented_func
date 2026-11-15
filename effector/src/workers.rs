//! The session's workers (262-KRN-0163; `init` lets `effector` start them since 750-KRN-0159): copies of this program
//! on other CPUs, each holding only what the session grants it.
//! - `BENCH`: `effector_bogomips_v1` until a deadline, its count in a page the session shares (`cpu_parallel`,
//!   750-EFF-0016).
//! - `NOTICE`: waits for `init`'s stop notice (`init.wit` 1.12, 750-KRN-0194) and tells the session, which sends its
//!   last heartbeat and ends within the notice (750-EFF-0006). It marks the notice's reason on a page the session
//!   shares first, which the session's waits for the network read, so a request that hangs never holds it (801-EFF-0036).
//!
//! Each worker's end arrives on the session's one endpoint as an exit notice, and the stop notice as a message on it.
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use effector::bench;
use mind::abi::SLOT_LIFECYCLE;
use mind::ipc::{Endpoint, Message};
use mind::mem::{Mapping, Pages};

const BENCH: u64 = 1;
const NOTICE: u64 = 2;
const PAGE_SLOT: usize = 9; // a benchmark worker's slot for the shared page
const BACK_SLOT: usize = 9; // the notice worker's slot for the session's endpoint
const MARK_SLOT: usize = 10; // the notice worker's slot for the page it marks the notice on
const NOTICE_TAG: usize = 0x4E4F_5449; // a stop notice's first word ("NOTI"); the second is the reason
const REFUSED_TAG: usize = 0x5245_4655; // init's refusal of the notice's call ("REFU"); the second word is its error
const MAX: usize = 32; // workers, at most: init's task quota for `effector`
const GO: usize = 0; // the page's words: the start, the deadline in monotonic ns, then a count and a state each
const DEADLINE: usize = 1;

/// In a worker: its work, then it ends.
pub fn run(index: u64, argument: u64) {
    match argument & 0xFF {
        BENCH => { let _ = index; bench_worker((argument >> 8) as usize) }
        NOTICE => notice_worker(),
        _ => {}
    }
}

// `place`: its words on the page, 1 to MAX, from its argument (a worker's own index counts every worker started).
fn bench_worker(place: usize) {
    let index = place;
    if index == 0 || index > MAX { return; }
    let Ok(page) = Mapping::new(PAGE_SLOT) else { return };
    let words = page.as_ptr::<u64>();
    let read = |i: usize| unsafe { words.add(i).read_volatile() };
    let begun = mind::time::monotonic_ns();
    while read(GO) == 0 {
        if mind::time::monotonic_ns() - begun > 30_000_000_000 { return; }
        mind::time::sleep(1);
    }
    let deadline = read(DEADLINE);
    let (mut state, mut count) = (bench::SEED, 0u64);
    while mind::time::monotonic_ns() < deadline {
        for _ in 0..16 { state = bench::batch(state); count += bench::BATCH; }
        unsafe { words.add(2 + 2 * index).write_volatile(count); }
    }
    unsafe { words.add(3 + 2 * index).write_volatile(state); }
}

fn notice_worker() {
    let (lifecycle, back) = (Endpoint(SLOT_LIFECYCLE), Endpoint(BACK_SLOT));
    let mark = Mapping::new(MARK_SLOT).ok();
    loop {
        match mind::idl::init::stop_notice(lifecycle) {
            Ok(Ok(reason)) => {
                // The page first: the session sees it in the middle of a request, the message between two.
                if let Some(page) = &mark { unsafe { page.as_ptr::<u64>().write_volatile(reason as u64 + 1); } }
                let _ = back.send(&Message::new(NOTICE_TAG, reason as usize));
                return;
            }
            // Replaced by a later call, or refused: this worker has nothing more to wait for.
            // A worker has no log of its own: the session says it.
            Ok(Err(e)) => { let _ = back.send(&Message::new(REFUSED_TAG, e as usize)); return }
            // The call itself failed (init restarting): again a second later.
            Err(_) => { mind::time::sleep(1000); }
        }
    }
}

// The page the notice worker marks, as the session maps it; 0 until it is made.
static MARK_AT: AtomicUsize = AtomicUsize::new(0);

/// The stop notice's reason as soon as the notice worker has it (801-EFF-0036): read by the session's waits for the
/// network, which a message on its endpoint would reach only between two requests.
pub fn stop_marked() -> Option<mind::idl::init::StopReason> {
    let at = MARK_AT.load(Relaxed);
    if at == 0 { return None; }
    match unsafe { (at as *const u64).read_volatile() } { 0 => None, word => mind::idl::init::StopReason::from_code(word as usize - 1) }
}

/// The session's side: its endpoint, the benchmark's page and workers, and the notice worker with its page.
pub struct Pool { endpoint: Option<Endpoint>, page: Option<Pages>, bench: Vec<u64>, ended: usize, notice: Option<u64>, mark: Option<Pages>, said: bool,
    /// The online CPUs, from the gateway's last sample.
    pub cpus: u16 }

/// What the endpoint brought.
pub enum Event { Stop(mind::idl::init::StopReason), None }

impl Pool {
    pub fn new() -> Self { Pool { endpoint: None, page: None, bench: Vec::new(), ended: 0, notice: None, mark: None, said: false, cpus: 1 } }

    // The one endpoint `init` lets the session make (750-KRN-0159), made once.
    fn endpoint(&mut self) -> Option<Endpoint> {
        if self.endpoint.is_none() {
            match Endpoint::create() {
                Ok(e) => self.endpoint = Some(e),
                Err(e) => if !core::mem::replace(&mut self.said, true) { mind::println!("EFFECTOR: NO ENDPOINT FOR WORKERS: {:?}", e) },
            }
        }
        self.endpoint
    }

    /// Starts the worker that waits for `init`'s stop notice, with a copy of the session's notice client.
    pub fn start_notice(&mut self) {
        if self.notice.is_some() || mind::dev::cap_info(SLOT_LIFECYCLE).0 != mind::abi::CAP_KIND_ENDPOINT { return; }
        let Some(endpoint) = self.endpoint() else { return };
        // The page the worker marks the notice on; without it the message alone tells the session, between requests.
        let mark = Pages::new(4096);
        if let Some(page) = &mark { unsafe { (page.address() as *mut u64).write_volatile(0); } }
        let object = mark.as_ref().and_then(|p| p.share().ok());
        let mut grants = Vec::from([mind::process::grant(SLOT_LIFECYCLE, SLOT_LIFECYCLE, u8::MAX), mind::process::grant(BACK_SLOT, endpoint.0, u8::MAX)]);
        if let Some(object) = object { grants.push(mind::process::grant(MARK_SLOT, object, u8::MAX)); }
        let started = mind::process::start_worker(NOTICE, 0, &grants);
        if let Some(object) = object { let _ = mind::ipc::drop_cap(object); }
        match started {
            Ok(pid) => {
                let _ = mind::process::watch(pid, endpoint);
                if let (Some(page), Some(_)) = (mark, object) { MARK_AT.store(page.address(), Relaxed); self.mark = Some(page); }
                self.notice = Some(pid);
                mind::println!("EFFECTOR: init'S STOP NOTICE AWAITED (WORKER PID={})", pid);
            }
            Err(e) => mind::println!("EFFECTOR: NO WORKER FOR init'S STOP NOTICE: {:?}", e),
        }
    }

    /// What arrived since the last look: workers' ends are counted; a stop notice is returned.
    pub fn poll(&mut self) -> Event {
        let Some(endpoint) = self.endpoint else { return Event::None };
        while let Ok(received) = endpoint.recv_timeout(0, 1) {
            if let Some(exit) = received.exit {
                if self.notice == Some(exit.pid) { self.notice = None; mind::println!("EFFECTOR: THE STOP NOTICE'S WORKER ENDED"); }
                else if self.bench.contains(&exit.pid) { self.ended += 1; }
                continue;
            }
            if received.data[0] == NOTICE_TAG {
                if let Some(reason) = mind::idl::init::StopReason::from_code(received.data[1]) { return Event::Stop(reason); }
            }
            if received.data[0] == REFUSED_TAG {
                match mind::idl::init::Error::from_code(received.data[1]) {
                    Some(e) => mind::println!("EFFECTOR: init ANSWERED THE STOP NOTICE'S WORKER {:?}: NO NOTICE AWAITED", e),
                    None => mind::println!("EFFECTOR: init ANSWERED THE STOP NOTICE'S WORKER WITH CODE {}: NO NOTICE AWAITED", received.data[1]),
                }
            }
        }
        Event::None
    }
}

impl bench::Parallel for Pool {
    fn start(&mut self, ms: u64) -> u16 {
        let Some(endpoint) = self.endpoint() else { return 0 };
        let Some(page) = Pages::new(4096) else { return 0 };
        let Ok(object) = page.share() else { return 0 };
        let words = page.address() as *mut u64;
        unsafe { for i in 0..512 { words.add(i).write_volatile(0); } }
        self.bench.clear();
        self.ended = 0;
        // One live worker a CPU (ABI 18), the notice's among them; the session's own thread is the last.
        let cpus = self.cpus.max(1) as usize;
        for place in 1..=cpus.saturating_sub(1 + self.notice.is_some() as usize).min(MAX) {
            match mind::process::start_worker(BENCH | (place as u64) << 8, 0, &[mind::process::grant(PAGE_SLOT, object, u8::MAX)]) {
                Ok(pid) => { let _ = mind::process::watch(pid, endpoint); self.bench.push(pid); }
                Err(_) => break,
            }
        }
        let _ = mind::ipc::drop_cap(object);
        if self.bench.is_empty() { return 0; }
        unsafe {
            words.add(DEADLINE).write_volatile(mind::time::monotonic_ns() + ms * 1_000_000);
            words.add(GO).write_volatile(1);
        }
        self.page = Some(page);
        self.bench.len() as u16
    }

    // The workers' ends are counted by the session's `poll` each pass of its loop.
    fn collect(&mut self) -> Option<u64> {
        if self.ended < self.bench.len() { return None; }
        let page = self.page.take()?;
        let words = page.address() as *const u64;
        let counted = (1..=self.bench.len()).map(|place| unsafe { words.add(2 + 2 * place).read_volatile() }).sum();
        self.bench.clear();
        Some(counted)
    }
}
