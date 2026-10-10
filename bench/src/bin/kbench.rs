#![no_std]
#![no_main]
// kbench [group …] [--quick]: the kernel's performance (176-KRN-0062). Each measurement is repeated; the screen shows
// its median with a bar on a log scale, its minimum and its 99th percentile, and log:kbenchNNNN.txt every statistic
// with a histogram. The IPC and process groups talk to copies of kbench it starts (`kbench --child`).
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use bench::out::{self, Log};
use bench::report::{self, Align, Stats, Table};
use core::hint::black_box;
use bench::child::{self, ECHO, EXIT};
use mind::abi::{CAP_READ, CAP_WRITE, ERR_LIMIT};
use mind::ipc::{self, Endpoint, Message};
use mind::mem::Pages;

mind::request!(REQUEST_CONSOLE | REQUEST_FILES | REQUEST_SYSINFO);

const GROUPS: [(&str, &str); 6] = [
    ("syscall", "System calls"), ("ipc", "IPC, two processes"), ("caps", "Capabilities"),
    ("memory", "Memory"), ("timer", "The timer"), ("process", "Processes"),
];
const USAGE: &str = "usage: kbench [syscall|ipc|caps|memory|timer|process …] [--quick]";
const MIB: usize = 1024 * 1024;
const PAGE: usize = 4096;

fn now() -> u64 { mind::time::monotonic_ns() }

// `samples` samples, each the mean of `batch` runs of `op`.
fn batches(samples: usize, batch: usize, mut op: impl FnMut()) -> Vec<u64> {
    (0..samples).map(|_| { let t = now(); for _ in 0..batch { op(); } (now() - t) / batch as u64 }).collect()
}

struct Run { log: Log, table: Table, quick: bool, rows: usize, failed: usize, notes: Vec<String> }

impl Run {
    // The repetitions of a measurement: a tenth with --quick, at least 3.
    fn reps(&self, full: usize) -> usize { if self.quick { (full / 10).max(3) } else { full } }

    fn group(&mut self, title: &str) {
        let line = self.table.row(&[title]);
        self.log.both(&line);
        self.log.detail(&format!("== {} (at {})", title, report::duration(self.log.elapsed())));
    }

    // A measured row: `samples` in ns, each the mean of `batch` operations.
    fn row(&mut self, name: &str, samples: &mut [u64], batch: usize) -> Stats {
        let stats = Stats::of(samples);
        self.rows += 1;
        let line = self.table.row(&[&format!("  {}", name), &report::duration(stats.median), &report::bar(stats.median, 15), &report::duration(stats.min), &report::duration(stats.p99)]);
        self.log.both(&line);
        let unit = if batch > 1 { format!("ns, each sample the mean of {} operations", batch) } else { String::from("ns") };
        self.log.detail_all(&report::stats_lines(name, &unit, &stats));
        stats
    }

    fn fail(&mut self, name: &str, why: &str) {
        self.rows += 1;
        self.failed += 1;
        let line = self.table.row(&[&format!("  {}", name), "failed", why]);
        self.log.both(&line);
        self.log.detail(&format!("{}: FAILED: {}", name, why));
    }

    fn note(&mut self, text: String) { self.notes.push(text); }
}

fn syscalls(run: &mut Run) {
    let n = run.reps(1000);
    run.row("uptime (empty call)", &mut batches(n, 32, || { black_box(mind::time::uptime_ms()); }), 32);
    run.row("monotonic clock read", &mut batches(n, 32, || { black_box(mind::time::monotonic_ns()); }), 32);
    run.row("task alive?", &mut batches(n, 32, || { black_box(mind::process::alive(1)); }), 32);
}

fn ipc(run: &mut Run) {
    let Ok(endpoint) = Endpoint::create() else { return run.fail("call and reply", "no endpoint") };
    let child = match child::start("kbench", "--child", endpoint).and_then(|pid| child::hello(endpoint).map(|()| pid)) {
        Ok(pid) => pid,
        Err(why) => { let _ = ipc::drop_cap(endpoint.0); return run.fail("call and reply", &why); }
    };
    run.log.detail(&format!("the echo child is PID {}", child));
    let n = run.reps(1000);
    let mut samples = Vec::with_capacity(n);
    for k in 0..n {
        let t = now();
        let reply = endpoint.call_timeout(&Message::new(ECHO, k), 0, 5000);
        let dt = now() - t;
        if !matches!(reply, Ok(r) if r.data[1] == k) { run.fail("call and reply", &format!("call {}: {:?}", k, reply.map(|r| r.data))); break; }
        samples.push(dt);
    }
    if samples.len() == n { run.row("call and reply", &mut samples, 1); }
    // A page lent for each call (read-only) and revoked after it, as ping does.
    let page = Pages::new(PAGE).and_then(|p| p.share().ok().map(|cap| (p, cap)));
    match page {
        None => run.fail("… with a lent page", "no page to lend"),
        Some((_page, cap)) => {
            let n = run.reps(500);
            let mut samples = Vec::with_capacity(n);
            for k in 0..n {
                let t = now();
                let reply = ipc::mint(cap, CAP_READ, 0, 0).and_then(|lent| endpoint.call_timeout(&Message::new(ECHO, k).with_cap(lent, 0), 0, 5000));
                let _ = ipc::revoke(cap);
                let dt = now() - t;
                if !matches!(reply, Ok(r) if r.data[1] == k) { run.fail("… with a lent page", &format!("call {}: {:?}", k, reply.map(|r| r.data))); break; }
                samples.push(dt);
            }
            if samples.len() == n { run.row("… with a lent page", &mut samples, 1); }
            let _ = ipc::drop_cap(cap);
        }
    }
    child::stop(endpoint, child);
    let _ = ipc::drop_cap(endpoint.0);
}

fn caps(run: &mut Run) {
    let Ok(endpoint) = Endpoint::create() else { return run.fail("mint and drop", "no endpoint") };
    let n = run.reps(500);
    let mut errors = 0;
    let mut samples = batches(n, 16, || match ipc::mint(endpoint.0, CAP_WRITE, 0, 0) { Ok(c) => { let _ = ipc::drop_cap(c); } Err(_) => errors += 1 });
    if errors == 0 { run.row("mint and drop", &mut samples, 16); } else { run.fail("mint and drop", &format!("{} mints refused", errors)); }
    let mut samples = batches(n, 16, || match ipc::mint_badged(endpoint.0, CAP_WRITE, 7) { Ok(c) => { let _ = ipc::drop_cap(c); } Err(_) => errors += 1 });
    if errors == 0 { run.row("badged mint and drop", &mut samples, 16); } else { run.fail("badged mint and drop", &format!("{} mints refused", errors)); }
    let mut samples = batches(n, 16, || { if ipc::mint(endpoint.0, CAP_WRITE, 0, 0).is_err() { errors += 1; } if ipc::revoke(endpoint.0) != Ok(1) { errors += 1; } });
    if errors == 0 { run.row("mint and revoke", &mut samples, 16); } else { run.fail("mint and revoke", &format!("{} refused or revoked other than one", errors)); }
    // A dropped endpoint counts against the quota until the kernel's idle pass reclaims it: at the limit, wait for it.
    let (mut samples, mut waits) = (Vec::with_capacity(n), 0);
    while samples.len() < n && waits < 300 {
        let t = now();
        match Endpoint::create() {
            Ok(e) => { let _ = ipc::drop_cap(e.0); samples.push(now() - t); }
            Err(mind::Error::Other(ERR_LIMIT)) => { waits += 1; mind::time::sleep(10); }
            Err(e) => { errors += 1; run.log.detail(&format!("endpoint create: {:?}", e)); break; }
        }
    }
    if errors == 0 && samples.len() == n {
        run.row("endpoint create+drop", &mut samples, 1);
        if waits > 0 { run.note(format!("endpoint create+drop: the quota was reached {} times; dropped endpoints count until the kernel's idle pass reclaims them", waits)); }
    } else { run.fail("endpoint create+drop", &format!("{} of {} created, {} waits", samples.len(), n, waits)); }
    let _ = ipc::drop_cap(endpoint.0);
}

fn memory(run: &mut Run) {
    let n = run.reps(500);
    let mut errors = 0;
    let mut samples = batches(n, 8, || match Pages::new(PAGE) { Some(p) => { black_box(p.address()); } None => errors += 1 });
    if errors == 0 { run.row("a page: take and free", &mut samples, 8); } else { run.fail("a page: take and free", &format!("{} refused", errors)); }
    let n = run.reps(50);
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let t = now();
        let Some(mut block) = Pages::new(MIB) else { break };
        for page in block.as_mut_slice().chunks_mut(PAGE) { unsafe { core::ptr::write_volatile(page.as_mut_ptr(), 1) }; }
        drop(block);
        samples.push(now() - t);
    }
    if samples.len() < n { return run.fail("1 MiB take+touch+free", "out of memory"); }
    let stats = run.row("1 MiB take+touch+free", &mut samples, 1);
    let rate = report::rate(MIB as u64, stats.median);
    run.log.detail(&format!("    1 MiB at {} (median)", rate));
    run.note(format!("1 MiB taken, touched and freed at {} (median)", rate));
}

fn timer(run: &mut Run) {
    for (ms, full) in [(1usize, 100usize), (10, 50)] {
        let n = run.reps(full);
        let mut samples: Vec<u64> = (0..n).map(|_| { let t = now(); mind::time::sleep(ms); now() - t }).collect();
        let stats = run.row(&format!("sleep {} ms", ms), &mut samples, 1);
        let asked = ms as u64 * 1_000_000;
        let early = if stats.min < asked { format!("; {} of {} shorter than asked", samples.iter().filter(|&&s| s < asked).count(), n) } else { String::new() };
        run.note(format!("sleep {} ms: median {}, shortest {}, longest {}{}", ms, report::duration(stats.median), report::duration(stats.min), report::duration(stats.max), early));
    }
}

fn processes(run: &mut Run) {
    let Ok(endpoint) = Endpoint::create() else { return run.fail("start (the loader)", "no endpoint") };
    let n = run.reps(20);
    let (mut launch, mut running, mut exit) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..n {
        let t0 = now();
        let pid = match child::start("kbench", "--child", endpoint) { Ok(pid) => pid, Err(why) => { run.fail("start (the loader)", &why); break; } };
        let t1 = now();
        if let Err(why) = child::hello(endpoint) { run.fail("start to running", &why); break; }
        let t2 = now();
        let _ = endpoint.call_timeout(&Message::new(EXIT, 0), 0, 2000);
        let t3 = now();
        while mind::process::alive(pid) && now() - t3 < 2_000_000_000 {}
        let t4 = now();
        if mind::process::alive(pid) { run.fail("exit to gone", "the child is still alive after 2 s"); break; }
        launch.push(t1 - t0); running.push(t2 - t0); exit.push(t4 - t3);
    }
    if launch.len() == n {
        run.row("start (the loader)", &mut launch, 1);
        run.row("start to running", &mut running, 1);
        run.row("exit to gone", &mut exit, 1);
    }
    let _ = ipc::drop_cap(endpoint.0);
}

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("kbench — the kernel's performance: system calls, IPC, capabilities, memory, the timer, processes.\nUsage: kbench [syscall|ipc|caps|memory|timer|process …] [--quick]   (default: every group)\nA table on the screen (median with a log-scale bar, minimum, 99th percentile); every statistic and a histogram in log:kbenchNNNN.txt (ram: without the log volume). --quick: a tenth of the repetitions.");
    let args = mind::process::args_str().trim();
    if args == "--child" { return child::serve(); }
    let words: Vec<&str> = args.split_whitespace().collect();
    let quick = words.contains(&"--quick");
    let chosen: Vec<&str> = words.iter().copied().filter(|w| *w != "--quick").collect();
    if let Some(unknown) = chosen.iter().find(|w| !GROUPS.iter().any(|(name, _)| name == *w)) { mind::println!("kbench: no group {}\n{}", unknown, USAGE); mind::process::exit_with(2); }
    let table = Table::new(&[(24, Align::Left), (8, Align::Right), (15, Align::Left), (8, Align::Right), (8, Align::Right)]);
    let mut run = Run { log: Log::new("kbench"), table, quick, rows: 0, failed: 0, notes: Vec::new() };
    run.log.both("kbench — the kernel's performance (176-KRN-0062)");
    for line in out::machine(info) { run.log.both(&line); }
    run.log.both(if quick { "--quick: a tenth of the repetitions" } else { "full repetitions (--quick: a tenth)" });
    run.log.log(&format!("arguments: {}", args));
    let (top, header, middle, bottom) = (run.table.top(), run.table.row(&["measurement", "median", "8 ns  log  1 s", "min", "p99"]), run.table.middle(), run.table.bottom());
    run.log.both(&top);
    run.log.both(&header);
    run.log.both(&middle);
    for (name, title) in GROUPS {
        if !chosen.is_empty() && !chosen.contains(&name) { continue; }
        run.group(title);
        match name {
            "syscall" => syscalls(&mut run),
            "ipc" => ipc(&mut run),
            "caps" => caps(&mut run),
            "memory" => memory(&mut run),
            "timer" => timer(&mut run),
            _ => processes(&mut run),
        }
    }
    run.log.both(&bottom);
    let notes = core::mem::take(&mut run.notes);
    for note in notes { run.log.both(&format!("  • {}", note)); }
    let summary = format!("kbench: {} measurements, {} failed, {}", run.rows, run.failed, report::duration(run.log.elapsed()));
    run.log.log(&summary);
    match run.log.save() {
        Ok(path) => mind::println!("{}; full log: {}", summary, path),
        Err(why) => mind::println!("{}; the log was not written ({})", summary, why),
    }
    if run.failed > 0 { mind::process::exit_with(1); }
}
