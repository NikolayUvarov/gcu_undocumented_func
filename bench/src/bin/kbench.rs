#![no_std]
#![no_main]
// kbench [group …] [--quick]: the kernel's performance (176-KRN-0062). Each measurement is repeated; the screen shows
// its median with a bar on a log scale, its minimum and its 99th percentile, and log:kbenchNNNN.txt every statistic
// with a histogram. The IPC and process groups talk to copies of kbench it starts (`kbench --child`), the workers group
// to workers of itself (262-KRN-0163), which it may start only when the owner names it in data/memory.conf; the pages group
// takes a GiB, which it may only with that line too (262-KRN-0161).
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use bench::child::{self, ECHO, EXIT, MINTS};
use bench::measure::{batches, now, Run};
use bench::report;
use core::hint::black_box;
use mind::abi::{CAP_GRANT, CAP_READ, CAP_WRITE, ERR_LIMIT, SLOT_INIT};
use mind::ipc::{self, Endpoint, Message};
use mind::mem::Pages;

mind::request!(REQUEST_CONSOLE | REQUEST_FILES | REQUEST_SYSINFO);

const GROUPS: [(&str, &str); 8] = [
    ("syscall", "System calls"), ("ipc", "IPC, two processes"), ("caps", "Capabilities"),
    ("memory", "Memory"), ("pages", "Random reads in 1 GiB"), ("timer", "The timer"), ("process", "Processes"), ("workers", "Workers, other CPUs"),
];
const USAGE: &str = "usage: kbench [syscall|ipc|caps|memory|pages|timer|process|workers …] [--quick]";
const MIB: usize = 1024 * 1024;
const PAGE: usize = 4096;

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
    revocations(run);
}

// CAP_REVOKE of subtrees of 10 to 10 000 nodes over four tasks: a copy of the root in each of three echoes, which mint
// the rest under it. The time grows with the nodes removed; each step of the walk holds the lock for at most 256 of
// them, and the longest step is noted (000-KRN-0104, MC-5.4).
fn revocations(run: &mut Run) {
    const ECHOES: usize = 3;
    // An endpoint for each echo, which receives on it. The endpoints the rows before dropped count until the kernel's
    // idle pass reclaims them: at the limit, wait for it.
    let create = || {
        let mut endpoint = Endpoint::create();
        for _ in 0..300 {
            if !matches!(endpoint, Err(mind::Error::Other(ERR_LIMIT))) { break; }
            mind::time::sleep(10); endpoint = Endpoint::create();
        }
        endpoint.ok()
    };
    let mut echoes: Vec<(Endpoint, u64)> = Vec::new();
    for _ in 0..ECHOES {
        let Some(endpoint) = create() else { run.fail("revoke a subtree", "no endpoint"); break };
        match child::start("kbench", "--child", endpoint).and_then(|pid| child::hello(endpoint).map(|()| pid)) {
            Ok(pid) => echoes.push((endpoint, pid)),
            Err(why) => { let _ = ipc::drop_cap(endpoint.0); run.fail("revoke a subtree", &why); break; }
        }
    }
    let root = echoes.first().and_then(|&(endpoint, _)| ipc::mint(endpoint.0, CAP_WRITE | CAP_GRANT, 0, 0).ok());
    if let Some(root) = root.filter(|_| echoes.len() == ECHOES) {
        for nodes in [10usize, 100, 1000, 10_000] {
            let name = format!("revoke {} nodes", nodes);
            let n = run.reps(if nodes >= 1000 { 10 } else { 50 });
            let (mut samples, mut longest, mut steps) = (Vec::with_capacity(n), 0u64, 0usize);
            for _ in 0..n {
                // Each echo gets a copy of the root and mints its share of the rest under it.
                let mut made = 0;
                for (k, &(endpoint, _)) in echoes.iter().enumerate() {
                    let share = (nodes - ECHOES) / ECHOES + usize::from(k < (nodes - ECHOES) % ECHOES);
                    let Ok(copy) = ipc::mint(root, CAP_WRITE | CAP_GRANT, 0, 0) else { break };
                    if let Ok(reply) = endpoint.call_timeout(&Message::new(MINTS, share).with_cap_moved(copy, u8::MAX), 0, 10_000) { made += 1 + reply.data[1]; }
                }
                let t = now();
                let revoked = ipc::revoke_measured(root);
                let dt = now() - t;
                match revoked {
                    Ok((removed, s, ns)) if removed == made && made == nodes => { samples.push(dt); longest = longest.max(ns); steps = steps.max(s); }
                    other => { run.fail(&name, &format!("made {}, revoke said {:?}", made, other)); break; }
                }
            }
            if samples.len() == n {
                run.row(&name, &mut samples, 1);
                run.note(format!("{} over 4 tasks: {} steps, the longest {} µs under the lock", name, steps, longest / 1000));
            }
        }
        let _ = ipc::drop_cap(root);
    }
    for (endpoint, pid) in echoes { child::stop(endpoint, pid); let _ = ipc::drop_cap(endpoint.0); }
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

// Reads at random places of a GiB, each place found from the word read before, so that no read overlaps the next: in
// 2 MiB and 1 GiB pages, then in 4 KiB pages (ALLOC_SMALL_PAGES), the same memory touched beforehand (262-KRN-0161).
fn pages(run: &mut Run) {
    const SPAN: usize = 1 << 30;
    let n = run.reps(200);
    let mut medians = Vec::new();
    for (name, small) in [("large pages", false), ("4 KiB pages", true)] {
        let Some(mut block) = (if small { Pages::new_small(SPAN) } else { Pages::new(SPAN) }) else {
            return run.note(String::from("pages: not measured, a GiB is beyond kbench's quota (name it in data/memory.conf)"));
        };
        for page in block.as_mut_slice().chunks_mut(PAGE) { unsafe { core::ptr::write_volatile(page.as_mut_ptr(), 1) }; }
        let words = block.address() as *const u64;
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        let mut samples = batches(n, 1024, || {
            x ^= x << 13; x ^= x >> 7; x ^= x << 17;
            x ^= unsafe { core::ptr::read_volatile(words.add((x as usize >> 3) % (SPAN / 8))) };
        });
        medians.push(run.row(name, &mut samples, 1024).median);
    }
    if let [large, small] = medians[..] {
        run.note(format!("pages: a random read over 1 GiB takes {} in large pages and {} in 4 KiB pages (medians, this machine only)", report::duration(large), report::duration(small)));
    }
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

// A worker's start until it says hello, and a call answered by a worker on another CPU (262-KRN-0163).
fn workers(run: &mut Run) {
    let Ok(endpoint) = Endpoint::create() else { return run.fail("worker start", "no endpoint") };
    let Ok(client) = ipc::mint(endpoint.0, CAP_READ | CAP_WRITE | CAP_GRANT, 0, 0) else { let _ = ipc::drop_cap(endpoint.0); return run.fail("worker start", "no client") };
    let grants = [mind::process::grant(SLOT_INIT, client, u8::MAX)];
    let n = run.reps(20);
    let (mut start, mut worker) = (Vec::with_capacity(n), 0);
    for k in 0..n {
        let t = now();
        let pid = match mind::process::start_worker(0, 0, &grants) {
            Ok(pid) => pid,
            Err(mind::Error::Rights) => { run.note(String::from("workers: not measured, kbench is not named in data/memory.conf")); break; }
            Err(e) => { run.fail("worker start", &format!("start: {:?}", e)); break; }
        };
        if let Err(why) = child::hello(endpoint) { run.fail("worker start", &why); break; }
        start.push(now() - t);
        if k + 1 < n { if !child::stop(endpoint, pid) { run.fail("worker start", "a worker did not end"); break; } } else { worker = pid; }
    }
    if worker != 0 {
        run.row("worker start", &mut start, 1);
        let n = run.reps(1000);
        let mut samples = Vec::with_capacity(n);
        for k in 0..n {
            let t = now();
            let reply = endpoint.call_timeout(&Message::new(ECHO, k), 0, 5000);
            let dt = now() - t;
            if !matches!(reply, Ok(r) if r.data[1] == k) { run.fail("worker call and reply", &format!("call {}: {:?}", k, reply.map(|r| r.data))); break; }
            samples.push(dt);
        }
        if samples.len() == n { run.row("worker call and reply", &mut samples, 1); }
        child::stop(endpoint, worker);
    }
    let _ = ipc::drop_cap(client);
    let _ = ipc::drop_cap(endpoint.0);
}

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("kbench — the kernel's performance: system calls, IPC, capabilities, memory, large pages, the timer, processes, workers.\nUsage: kbench [syscall|ipc|caps|memory|pages|timer|process|workers …] [--quick]   (default: every group)\nA table on the screen (median with a log-scale bar, minimum, 99th percentile); every statistic and a histogram in log:kbenchNNNN.txt (ram: without the log volume). --quick: a tenth of the repetitions.");
    let args = mind::process::args_str().trim();
    if args == "--child" || mind::process::worker().is_some() { return child::serve(); }
    let words: Vec<&str> = args.split_whitespace().collect();
    let quick = words.contains(&"--quick");
    let chosen: Vec<&str> = words.iter().copied().filter(|w| *w != "--quick").collect();
    if let Some(unknown) = chosen.iter().find(|w| !GROUPS.iter().any(|(name, _)| name == *w)) { mind::println!("kbench: no group {}\n{}", unknown, USAGE); mind::process::exit_with(2); }
    let mut run = Run::start("kbench", "kbench — the kernel's performance (176-KRN-0062)", info, args, quick);
    for (name, title) in GROUPS {
        if !chosen.is_empty() && !chosen.contains(&name) { continue; }
        run.group(title);
        match name {
            "syscall" => syscalls(&mut run),
            "ipc" => ipc(&mut run),
            "caps" => caps(&mut run),
            "memory" => memory(&mut run),
            "pages" => pages(&mut run),
            "timer" => timer(&mut run),
            "workers" => workers(&mut run),
            _ => processes(&mut run),
        }
    }
    run.finish("kbench");
}
