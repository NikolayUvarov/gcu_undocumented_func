#![no_std]
#![no_main]
// memtest: test program of task memory beyond the kernel arena (issue 150).
// `memtest alloc <MiB>` takes MiB of heap in 16 MiB blocks, writes and checks every word, tries one block beyond its
// quota, and frees everything; `memtest hold <MiB>` keeps them until killed. `memtest share <MiB>` fills a block, seals it (a read-only memory object), maps it and
// starts a second memtest that maps the same object; both check the contents. `memtest fill` takes all the heap it can
// get, in ever smaller blocks down to 64 KiB, and keeps it until killed (issue 169: the recovery reserve stops it).
// `memtest big <MiB>` takes them in blocks of up to 1 GiB, each as one ALLOC (262-KRN-0160), checks that every page
// came zeroed, writes and reads back a word in each page, and says whether each whole GiB lies on a GiB boundary; with
// hold it keeps them until killed.
// `memtest workers <n> [hold|fault]` starts n workers of itself (262-KRN-0163) on one shared page, each counting into its
// own word once the page says go; it reports each one's count and how it ended. hold: they count until memtest is
// killed; fault: the last one faults instead of counting.
// `memtest load <path> [workers]` reads a file into a block of its own in one `fill` call (262-KRN-0162), times it beside
// read's 16 KiB calls over its first 64 MiB, then maps the block again through a read-only child of its capability and
// prints its SHA-256 from there; its workers map read-only children: all but the last hash it, the last writes and faults.
use mind::abi::{BootInfo, CAP_READ};
use mind::idl::loader;
use mind::ipc::{self, Endpoint, Message};
use mind::mem::{Mapping, Pages};

mind::request!(REQUEST_CONSOLE | REQUEST_FILES, memory: QUOTA_MIB);

const QUOTA_MIB: u32 = 160;
const MIB: usize = 1024 * 1024;
const BLOCK: usize = 16 * MIB;
const RECEIVED: usize = 9; // the (fixed) slot the child receives the object in
const COUNT: u64 = 200_000; // what each worker counts to
const GO: usize = 0; // the shared page's word that starts the workers; word i is worker i's count

fn word(index: usize) -> u64 { (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5A5A }

fn fill(memory: &mut [u8], first: usize) {
    for (i, chunk) in memory.chunks_exact_mut(8).enumerate() { chunk.copy_from_slice(&word(first + i).to_le_bytes()); }
}
fn intact(memory: &[u8], first: usize) -> bool {
    memory.chunks_exact(8).enumerate().all(|(i, chunk)| u64::from_le_bytes(chunk.try_into().unwrap()) == word(first + i))
}

fn big(mib: usize, hold: bool) {
    const GIB: usize = 1024 * MIB;
    let mut blocks: [Option<Pages>; 64] = [const { None }; 64];
    let (mut taken, mut zeroed, mut aligned, mut n) = (0, true, true, 0);
    while taken < mib * MIB && n < blocks.len() {
        let size = (mib * MIB - taken).min(GIB);
        let Some(pages) = Pages::new(size) else { mind::println!("[MEMTEST] BIG: OUT OF MEMORY AFTER {} MiB", taken / MIB); break };
        let words = pages.address() as *mut u64;
        for page in 0..size / 4096 { unsafe {
            let at = words.add(page * 512 + page % 512);
            zeroed &= at.read_volatile() == 0;
            at.write_volatile(word(taken / 8 + page));
        } }
        aligned &= size < GIB || pages.address() % GIB == 0;
        blocks[n] = Some(pages); taken += size; n += 1;
    }
    let mut first = 0;
    let intact = blocks[..n].iter().flatten().all(|p| { let words = p.address() as *const u64; let ok = (0..p.len() / 4096).all(|page| unsafe { words.add(page * 512 + page % 512).read_volatile() } == word(first / 8 + page)); first += p.len(); ok });
    mind::println!("[MEMTEST] BIG HELD {} MiB IN {} BLOCKS ZEROED={} INTACT={} GIB ALIGNED={}", taken / MIB, n, zeroed, intact, aligned);
    if hold { loop { mind::time::sleep(1000); } } // until killed
    for block in blocks.iter_mut() { block.take(); }
    mind::println!("[MEMTEST] BIG FREED");
}

fn alloc(mib: usize, hold: bool) {
    let mut blocks: [Option<Pages>; 64] = [const { None }; 64];
    let count = mib.div_ceil(16).min(blocks.len());
    for (n, block) in blocks[..count].iter_mut().enumerate() {
        let Some(mut pages) = Pages::new(BLOCK) else { mind::println!("[MEMTEST] OUT OF MEMORY AFTER {} MiB", n * 16); return };
        fill(pages.as_mut_slice(), n * BLOCK / 8);
        *block = Some(pages);
    }
    let ok = blocks[..count].iter().enumerate().all(|(n, b)| b.as_ref().is_some_and(|p| intact(p.as_slice(), n * BLOCK / 8)));
    mind::println!("[MEMTEST] HELD {} MiB INTACT={}", count * 16, ok);
    if hold { loop { mind::time::sleep(1000); } } // until killed
    // The quota stops the next block (memory beyond it belongs to others).
    let beyond = Pages::new(QUOTA_MIB as usize * MIB);
    mind::println!("[MEMTEST] BEYOND QUOTA: {}", if beyond.is_some() { "GRANTED" } else { "REFUSED" });
    drop(beyond);
    for block in blocks.iter_mut() { block.take(); }
    mind::println!("[MEMTEST] FREED");
}

fn fill_all() {
    let mut blocks: [Option<Pages>; 64] = [const { None }; 64];
    let (mut used, mut total) = (0, 0);
    for size in [BLOCK, 4 * MIB, MIB, 256 * 1024, 64 * 1024] {
        while used < blocks.len() {
            let Some(mut pages) = Pages::new(size) else { break };
            pages.as_mut_slice()[0] = 1;
            blocks[used] = Some(pages); used += 1; total += size;
        }
    }
    mind::println!("[MEMTEST] FILLED {} KiB", total / 1024);
    loop { mind::time::sleep(1000); } // until killed
}

// Creates endpoints until its quota refuses one and keeps them until killed (issue 171: the endpoint table grows).
fn endpoints() {
    let mut count = 0;
    while Endpoint::create().is_ok() { count += 1; }
    mind::println!("[MEMTEST] ENDPOINTS {}", count);
    loop { mind::time::sleep(1000); }
}

// Whether the contents are the pattern, and their sum, from one word in every 512 bytes (emulation is slow).
fn check(memory: &[u8]) -> (bool, u64) {
    (0..memory.len() / 512).fold((true, 0u64), |(ok, s), n| {
        let (i, at) = (n * 64 + n % 64, (n * 64 + n % 64) * 8);
        let v = u64::from_le_bytes(memory[at..at + 8].try_into().unwrap());
        (ok && v == word(i), s.wrapping_add(v))
    })
}

fn share(mib: usize) {
    let Some(mut pages) = Pages::new(mib * MIB) else { mind::println!("[MEMTEST] OUT OF MEMORY"); return };
    fill(pages.as_mut_slice(), 0);
    // A sealed object: detached from this heap, only a read-only capability left.
    let Ok(object) = pages.detach() else { mind::println!("[MEMTEST] DETACH FAILED"); return };
    let Ok(sealed) = ipc::mint(object, CAP_READ, 0, 0) else { mind::println!("[MEMTEST] MINT FAILED"); return };
    let _ = ipc::drop_cap(object);
    mind::println!("[MEMTEST] SEALED={}", mind::mem::sealed(sealed));
    let mapping = match Mapping::new(sealed) { Ok(m) => m, Err(e) => { mind::println!("[MEMTEST] MAP FAILED {:?}", e); return } };
    let (ok, total) = check(mapping.as_slice());
    mind::println!("[MEMTEST] PARENT MAPPED {} MiB INTACT={} SUM={:x}", mapping.len() / MIB, ok, total);
    // The second task gets this endpoint in its INIT slot and asks for the object.
    let Ok(endpoint) = Endpoint::create() else { mind::println!("[MEMTEST] NO ENDPOINT"); return };
    let started = loader::begin(Endpoint::LOADER, "memtest", "child")
        .and_then(|s| s.map_err(|_| mind::Error::Invalid))
        .and_then(|session| { let client = ipc::mint(endpoint.0, mind::abi::CAP_WRITE | mind::abi::CAP_GRANT, 0, 0)?; loader::grant(Endpoint::LOADER, session, mind::abi::SLOT_INIT as u8, client)?.map_err(|_| mind::Error::Invalid)?; Ok(session) })
        .and_then(|session| loader::commit(Endpoint::LOADER, session)?.map_err(|_| mind::Error::Invalid));
    let Ok(child) = started else { mind::println!("[MEMTEST] CHILD NOT STARTED {:?}", started); return };
    mind::println!("[MEMTEST] CHILD PID={}", child);
    match endpoint.recv_timeout(0, 10_000) {
        Ok(call) if call.is_call => { let _ = ipc::reply(&Message::new(0, 0).with_cap(sealed, CAP_READ)); }
        other => { mind::println!("[MEMTEST] NO CALL FROM THE CHILD {:?}", other.map(|r| r.data)); return }
    }
    // The child's answer: its sum of the contents.
    match endpoint.recv_timeout(0, 120_000) {
        Ok(answer) => {
            let (mib, sealed, ok) = (answer.data[1] >> 2, answer.data[1] & 2 != 0, answer.data[1] & 1 != 0);
            mind::println!("[MEMTEST] CHILD MAPPED {} MiB READ-ONLY SEALED={} INTACT={} SAME SUM={}", mib, sealed, ok, answer.data[0] as u64 == total);
        }
        Err(e) => mind::println!("[MEMTEST] NO ANSWER {:?}", e),
    }
    drop(mapping);
    let _ = ipc::drop_cap(sealed);
    mind::println!("[MEMTEST] SHARE DONE");
}

fn child() {
    let Ok(_) = Endpoint::INIT.call(&Message::new(0, 0), RECEIVED) else { mind::println!("[MEMTEST] CHILD CALL FAILED"); return };
    let mapping = match Mapping::new(RECEIVED) { Ok(m) => m, Err(e) => { mind::println!("[MEMTEST] CHILD MAP FAILED {:?}", e); return } };
    let (ok, total) = check(mapping.as_slice());
    // The parent reports it: a console program's output is gone once it ends.
    let flags = mapping.len() / MIB << 2 | (mind::mem::sealed(RECEIVED) as usize) << 1 | ok as usize;
    let _ = Endpoint::INIT.send(&Message::new(total as usize, flags));
}

fn workers(count: usize, mode: &str) {
    let Some(page) = Pages::new(4096) else { mind::println!("[MEMTEST] OUT OF MEMORY"); return };
    let Ok(object) = page.share() else { mind::println!("[MEMTEST] SHARE FAILED"); return };
    let Ok(notices) = Endpoint::create() else { mind::println!("[MEMTEST] NO ENDPOINT"); return };
    let words = page.address() as *mut u64;
    let argument = match mode { "hold" => 1, "fault" => 2, _ => 0 };
    let mut started = 0;
    for index in 1..=count {
        let grants = [mind::process::grant(RECEIVED, object, u8::MAX)];
        match mind::process::start_worker(argument | (count as u64) << 8, 0, &grants) {
            Ok(pid) => { let _ = mind::process::watch(pid, notices); started += 1; mind::println!("[MEMTEST] WORKER {} PID={}", index, pid); }
            Err(e) => mind::println!("[MEMTEST] WORKER {} REFUSED {:?}", index, e),
        }
    }
    unsafe { words.add(GO).write_volatile(1); }
    if argument == 1 { loop { mind::time::sleep(1000); } }
    for _ in 0..started {
        match notices.recv_timeout(0, 120_000) {
            Ok(received) => if let Some(exit) = received.exit { mind::println!("[MEMTEST] WORKER PID={} ENDED {:?}", exit.pid, mind::process::Exit::from_reason(exit.reason)) },
            Err(e) => { mind::println!("[MEMTEST] NO EXIT NOTICE {:?}", e); break }
        }
    }
    let counts: [u64; 8] = core::array::from_fn(|i| if i < started { unsafe { words.add(1 + i).read_volatile() } } else { 0 });
    let counted = counts[..started].iter().filter(|&&n| n == COUNT).count();
    mind::println!("[MEMTEST] WORKERS {} STARTED, {} COUNTED {} EACH", started, counted, COUNT);
}

// A SHA-256 digest in lower-case hex (memtest has no heap allocator).
fn hex<'a>(digest: &[u8; 32], out: &'a mut [u8; 64]) -> &'a str {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for (i, b) in digest.iter().enumerate() { out[2 * i] = DIGITS[(b >> 4) as usize]; out[2 * i + 1] = DIGITS[(b & 15) as usize]; }
    core::str::from_utf8(out).unwrap_or("")
}

const LOAD: u64 = 3; // a worker's mode for `load`: argument = LOAD | workers << 8 | file bytes << 16

fn load(path: &str, count: usize) {
    let Ok(file) = mind::fs::File::open(path) else { return mind::println!("[MEMTEST] LOAD: NO FILE {}", path) };
    let size = file.size();
    let Some(pages) = Pages::new(size.max(1).next_multiple_of(4096)) else { return mind::println!("[MEMTEST] LOAD: OUT OF MEMORY FOR {} BYTES", size) };
    let Ok(lent) = pages.share() else { return mind::println!("[MEMTEST] LOAD: SHARE FAILED") };
    let started = mind::time::monotonic_ns();
    let filled = file.fill(0, lent);
    let fill_ms = (mind::time::monotonic_ns() - started) / 1_000_000;
    let Ok(filled) = filled else { return mind::println!("[MEMTEST] LOAD: FILL FAILED {:?}", filled) };
    // The same file's first 64 MiB through read's 16 KiB calls, for the time and the bytes.
    let part = size.min(64 * MIB);
    let Some(mut scratch) = Pages::new(part.max(1)) else { return mind::println!("[MEMTEST] LOAD: OUT OF MEMORY FOR THE READ") };
    let started = mind::time::monotonic_ns();
    let read = file.read_at(0, &mut scratch.as_mut_slice()[..part]);
    let read_ms = (mind::time::monotonic_ns() - started) / 1_000_000;
    let same = matches!(read, Ok(n) if n == part) && scratch.as_slice()[..part] == pages.as_slice()[..part];
    drop(scratch);
    // The block through a read-only child of its capability (an object of 1 GiB is past MEM_DETACH's 256 MiB in all).
    let object = lent;
    let Ok(read_only) = ipc::mint(object, CAP_READ, 0, 0) else { return mind::println!("[MEMTEST] LOAD: MINT FAILED") };
    let Ok(view) = Mapping::new(read_only) else { return mind::println!("[MEMTEST] LOAD: MAP FAILED") };
    let digest = mind::sha256::digest(&view.as_slice()[..size]);
    mind::println!("[MEMTEST] LOAD {} BYTES IN ONE FILL: {} MS; READ'S 16 KIB CALLS: {} MS FOR {} MIB, THE SAME BYTES={}; SHA256 {}",
        filled, fill_ms, read_ms, part / MIB, same, hex(&digest, &mut [0; 64]));
    if count == 0 { return; }
    // The workers: a read-only child of the object each, and a page of their own for the hashes.
    let (Some(results), Ok(notices)) = (Pages::new(4096), Endpoint::create()) else { return mind::println!("[MEMTEST] LOAD: NO RESULT PAGE") };
    let Ok(shared) = results.share() else { return mind::println!("[MEMTEST] LOAD: SHARE FAILED") };
    let (mut started, mut faulted) = (0, 0);
    for index in 1..=count {
        let Ok(child) = ipc::mint(object, CAP_READ, 0, 0) else { break };
        let grants = [mind::process::grant(RECEIVED, child, u8::MAX), mind::process::grant(RECEIVED + 1, shared, u8::MAX)];
        match mind::process::start_worker(LOAD | (count as u64) << 8 | (size as u64) << 16, 0, &grants) {
            Ok(pid) => { let _ = mind::process::watch(pid, notices); started += 1; }
            Err(e) => mind::println!("[MEMTEST] LOAD: WORKER {} REFUSED {:?}", index, e),
        }
        let _ = ipc::drop_cap(child);
    }
    for _ in 0..started {
        match notices.recv_timeout(0, 600_000) {
            Ok(received) => if let Some(exit) = received.exit { if matches!(mind::process::Exit::from_reason(exit.reason), mind::process::Exit::Fault(_)) { faulted += 1; } },
            Err(e) => { mind::println!("[MEMTEST] LOAD: NO EXIT NOTICE {:?}", e); break }
        }
    }
    let hashes = results.as_slice();
    let agreed = (1..count).filter(|&i| hashes[i * 32..(i + 1) * 32] == digest).count();
    mind::println!("[MEMTEST] LOAD WORKERS: {} SAW THE SAME SHA256, {} FAULTED ON A WRITE", agreed, faulted);
}

// A load worker: maps the object read-only; all but the last hash it into the result page, the last writes to it.
fn load_work(index: u64, argument: u64) {
    let (count, size) = ((argument >> 8 & 0xFF) as usize, (argument >> 16) as usize);
    let (Ok(object), Ok(mut results)) = (Mapping::new(RECEIVED), Mapping::new(RECEIVED + 1)) else { return };
    if index as usize == count { unsafe { object.as_ptr::<u8>().write_volatile(1) } } // a fault on purpose
    let digest = mind::sha256::digest(&object.as_slice()[..size]);
    results.as_mut_slice()[index as usize * 32..(index as usize + 1) * 32].copy_from_slice(&digest);
}

// A worker: maps the page, waits for go, then counts into its word (or faults, or counts on for ever).
fn work(index: u64, argument: u64) {
    if argument & 0xFF == LOAD { return load_work(index, argument); }
    let Ok(mapping) = Mapping::new(RECEIVED) else { return };
    let words = mapping.as_ptr::<u64>();
    while unsafe { words.add(GO).read_volatile() } == 0 { mind::time::sleep(1); }
    let (mode, count) = (argument & 0xFF, argument >> 8);
    if mode == 2 && index == count { unsafe { (8 as *mut u64).write_volatile(index) } } // a fault on purpose
    let mut n = 0;
    while mode == 1 || n < COUNT { n += 1; unsafe { words.add(index as usize).write_volatile(n); } }
}

// Sleeps of 10 ms for `ms` (0: until killed), and the longest so far once a second: while another CPU zeroes a large
// block, the boot CPU's tick still ends them (262-KRN-0165).
fn naps(ms: u64) {
    let start = mind::time::monotonic_ns();
    let (mut longest, mut count, mut shown) = (0, 0, start);
    while ms == 0 || mind::time::monotonic_ns() < start + ms * 1_000_000 {
        let before = mind::time::monotonic_ns();
        mind::time::sleep(10);
        let now = mind::time::monotonic_ns();
        longest = longest.max(now - before); count += 1;
        if now - shown >= 1_000_000_000 { shown = now; mind::println!("[MEMTEST] NAPS {} LONGEST {} MS", count, longest / 1_000_000); }
    }
    mind::println!("[MEMTEST] NAPS {} LONGEST {} MS", count, longest / 1_000_000);
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    if let Some((index, argument)) = mind::process::worker() { return work(index, argument); }
    mind::about!("memtest — test program of task memory beyond the kernel arena: large heaps and shared read-only objects.\nUsage: memtest alloc <MiB> | memtest hold <MiB> | memtest share <MiB> | memtest fill | memtest endpoints | memtest workers <n> [hold|fault] | memtest big <MiB> [hold] | memtest load <path> [workers] | memtest naps <ms>");
    let mut words = mind::process::args_str().split_whitespace();
    let (command, mib) = (words.next().unwrap_or(""), words.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(16));
    match command {
        "alloc" => alloc(mib, false),
        "big" => big(mib, words.next() == Some("hold")),
        "hold" => alloc(mib, true),
        "share" => share(mib),
        "fill" => fill_all(),
        "endpoints" => endpoints(),
        "child" => child(),
        "workers" => workers(mib.min(7), words.next().unwrap_or("")),
        "naps" => naps(mib as u64),
        "load" => { let path = mind::process::args_str().split_whitespace().nth(1).unwrap_or(""); load(path, words.next().and_then(|s| s.parse().ok()).unwrap_or(0).min(7)) }
        _ => mind::println!("USAGE: MEMTEST ALLOC <MiB> | MEMTEST HOLD <MiB> | MEMTEST SHARE <MiB> | MEMTEST FILL | MEMTEST ENDPOINTS | MEMTEST WORKERS <n> [HOLD|FAULT] | MEMTEST BIG <MiB> | MEMTEST LOAD <PATH> [WORKERS] | MEMTEST NAPS <MS>"),
    }
}
