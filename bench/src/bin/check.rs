#![no_std]
#![no_main]
// check [group …]: a self-test of what is done (176-KRN-0063). Each check passes (✓), fails with its reason (✗) or is
// skipped with its reason (○); the summary counts them and the exit status is 1 if any failed. The table goes to the
// screen and, with each check's details, to log:checkNNNN.txt. Of init's lifecycle interface it calls `boot`, `list`, `mode`, and `set-mode` to see it refused.
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use bench::child::{self, ECHO, HASH, PEEK, READ, READ_END, SLOTS, SURVIVED};
use bench::out::{self, Log};
use bench::report::{self, Align, Table, Verdict};
use mind::abi::*;
use mind::idl::{init as lifecycle, loader, socket, sysinfo};
use mind::ipc::{self, Endpoint, Message};
use mind::mem::Pages;

mind::request!(REQUEST_CONSOLE | REQUEST_FILES | REQUEST_SYSINFO | REQUEST_LIFECYCLE | REQUEST_NETWORK);

const GROUPS: [(&str, &str); 8] = [
    ("kernel", "Kernel"), ("security", "Security"), ("clock", "Clocks"), ("files", "Files and programs"),
    ("services", "Boot services"), ("devices", "Devices"), ("sound", "Sound"), ("network", "Network"),
];
const USAGE: &str = "usage: check [kernel|security|clock|files|services|devices|sound|network …]";
const SYSMON: Endpoint = Endpoint(SLOT_SYSINFO);
// Where a text input's writer arrives from the loader, as record's copies do: an application holds no SLOT_WINDOWS.
const RECEIVE: usize = SLOT_WINDOWS;
const FED_BYTES: usize = 100 * 1024; // more than the kernel queues (TEXT_QUEUE)
const STACK: Endpoint = Endpoint(SLOT_NETWORK);

fn now() -> u64 { mind::time::monotonic_ns() }
fn pass(detail: String) -> Verdict { Verdict::Passed(detail) }
fn fail(detail: String) -> Verdict { Verdict::Failed(detail) }
fn skip(detail: String) -> Verdict { Verdict::Skipped(detail) }
fn granted(slot: usize) -> bool { mind::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT }
fn ip(a: u32) -> String { let b = a.to_be_bytes(); format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]) }

struct Run { log: Log, table: Table, verdicts: Vec<Verdict>, names: Vec<String> }

impl Run {
    fn group(&mut self, title: &str) {
        let line = self.table.row(&["", title]);
        self.log.both(&line);
        self.log.detail(&format!("== {} (at {})", title, report::duration(self.log.elapsed())));
    }

    fn report(&mut self, name: &str, verdict: Verdict) {
        let line = self.table.row(&[verdict.mark(), name, verdict.detail()]);
        self.log.both(&line);
        self.log.detail(&format!("{} {}: {}", verdict.word(), name, verdict.detail()));
        self.verdicts.push(verdict);
        self.names.push(String::from(name));
    }
}

fn kernel(run: &mut Run) {
    let (t0, u0) = (now(), mind::time::uptime_ms());
    mind::time::sleep(30);
    let (t1, u1) = (now(), mind::time::uptime_ms());
    run.report("clocks advance", if t1 > t0 && u1 > u0 { pass(format!("monotonic +{}, uptime +{} ms", report::duration(t1 - t0), u1 - u0)) }
        else { fail(format!("monotonic {} → {}, uptime {} → {} ms", t0, t1, u0, u1)) });
    run.report("memory: write, read back", match Pages::new(64 * 1024) {
        Some(mut block) => {
            for (i, b) in block.as_mut_slice().iter_mut().enumerate() { *b = (i * 7 + i / 251) as u8; }
            let bad = block.as_slice().iter().enumerate().filter(|&(i, &b)| b != (i * 7 + i / 251) as u8).count();
            if bad == 0 { pass(String::from("64 KiB taken, written, read back and freed")) } else { fail(format!("{} bytes read back wrong", bad)) }
        }
        None => fail(String::from("64 KiB refused")),
    });
    run.report("memory: quota holds", match Pages::new(1 << 40) { None => pass(String::from("1 TiB refused")), Some(_) => fail(String::from("1 TiB granted")) });
    let Ok(endpoint) = Endpoint::create() else { return run.report("endpoint", fail(String::from("no endpoint created"))) };
    run.report("capability rights", match ipc::mint(endpoint.0, CAP_WRITE, 0, 0) {
        Ok(send_only) => {
            let refused = Endpoint(send_only).recv_timeout(0, 10);
            let _ = ipc::drop_cap(send_only);
            if matches!(refused, Err(mind::Error::Rights)) { pass(String::from("a send-only copy may not receive")) } else { fail(format!("receive on a send-only copy: {:?}", refused.map(|r| r.data))) }
        }
        Err(e) => fail(format!("mint: {:?}", e)),
    });
    run.report("capability revocation", match ipc::mint(endpoint.0, CAP_WRITE, 0, 0) {
        Ok(copy) => {
            let removed = ipc::revoke(endpoint.0);
            if removed == Ok(1) && ipc::drop_cap(copy).is_err() { pass(String::from("a revoked copy is gone")) } else { fail(format!("revoke removed {:?}; the copy still there: {}", removed, ipc::drop_cap(copy).is_ok())) }
        }
        Err(e) => fail(format!("mint: {:?}", e)),
    });
    // A second process: a call and its reply, a lent page read on the other side, its exit seen.
    let pid = match child::start("check", "--child", endpoint).and_then(|pid| child::hello(endpoint).map(|()| pid)) {
        Ok(pid) => { run.report("a program starts", pass(format!("check --child is PID {}", pid))); pid }
        Err(why) => { run.report("a program starts", fail(why)); let _ = ipc::drop_cap(endpoint.0); return; }
    };
    run.report("IPC: call and reply", match endpoint.call_timeout(&Message::new(ECHO, 0x5EED), 0, 5000) {
        Ok(reply) if reply.data == [ECHO, 0x5EED] => pass(format!("PID {} answered", pid)),
        other => fail(format!("{:?}", other.map(|r| r.data))),
    });
    run.report("IPC: a lent page", match Pages::new(4096).and_then(|mut page| { page.as_mut_slice()[..8].copy_from_slice(&0xC0FF_EE00_1234_5678u64.to_le_bytes()); page.share().ok().map(|cap| (page, cap)) }) {
        Some((_page, cap)) => {
            let reply = ipc::mint(cap, CAP_READ, 0, 0).and_then(|lent| endpoint.call_timeout(&Message::new(PEEK, 0).with_cap(lent, 0), 0, 5000));
            let _ = ipc::revoke(cap);
            let _ = ipc::drop_cap(cap);
            match reply {
                Ok(r) if r.data[1] as u64 == 0xC0FF_EE00_1234_5678 => pass(String::from("the other process read the page lent to it")),
                other => fail(format!("{:?}", other.map(|r| r.data))),
            }
        }
        None => fail(String::from("no page to lend")),
    });
    run.report("a program ends", if child::stop(endpoint, pid) { pass(format!("PID {} ended when told", pid)) } else { fail(format!("PID {} still runs", pid)) });
    let _ = ipc::drop_cap(endpoint.0);
    text_input(run);
    every_slot(run);
    stop_and_notice(run);
    console_closes(run);
}

// The loader ends a program it started for check, and only such a one, and sends each one's exit notice where check
// asked (loader 1.11, 800-KRN-0138): one that waits, stopped, and one that ends at once with code 3.
fn stop_and_notice(run: &mut Run) {
    let name = "launch: stop and notice";
    let Ok(endpoint) = Endpoint::create() else { return run.report(name, fail(String::from("no endpoint created"))) };
    let killed = child::start_watched("check", "--child", endpoint).and_then(|pid| child::hello(endpoint).map(|()| pid)).and_then(|pid| match loader::stop(Endpoint::LOADER, pid) {
        Ok(Ok(())) => child::exit_notice(endpoint, pid).map(|exit| (pid, exit)),
        other => { let _ = child::stop(endpoint, pid); Err(format!("stop: {:?}", other)) }
    });
    let exited = child::start_watched("check", "--exit 3", endpoint).and_then(|pid| child::exit_notice(endpoint, pid));
    // Init (PID 1) is not check's program, nor is one that has ended.
    let refused = [loader::stop(Endpoint::LOADER, 1), loader::stop(Endpoint::LOADER, killed.as_ref().map_or(1, |&(pid, _)| pid))];
    // A watch needs the right to receive.
    let send_only = loader::begin(Endpoint::LOADER, "check", "--exit 0").ok().and_then(Result::ok).map(|session| {
        let watched = ipc::mint(endpoint.0, CAP_WRITE, 0, 0).map(|copy| { let watched = loader::watch(Endpoint::LOADER, session, copy); let _ = ipc::drop_cap(copy); watched });
        let _ = loader::abort(Endpoint::LOADER, session);
        watched
    });
    let not_found = |r: &mind::Result<Result<(), loader::Error>>| matches!(r, Ok(Err(loader::Error::NotFound)));
    run.report(name, match (killed, exited) {
        (Ok((pid, mind::process::Exit::Killed)), Ok(mind::process::Exit::Code(3))) if refused.iter().all(not_found) && matches!(send_only, Some(Ok(Ok(Err(loader::Error::Rights))))) =>
            pass(format!("PID {} killed; code 3 noticed; 3 refused", pid)),
        (Ok(_), Ok(_)) if !refused.iter().all(not_found) => fail(format!("stop of PID 1 or an ended one: {:?}", refused)),
        (Ok(_), Ok(_)) if !matches!(send_only, Some(Ok(Ok(Err(loader::Error::Rights))))) => fail(format!("a send-only watch: {:?}", send_only)),
        (killed, exited) => fail(format!("stopped: {:?}; ended at once: {:?}", killed.map(|k| k.1), exited)),
    });
    let _ = ipc::drop_cap(endpoint.0);
}

// A tool whose console is gone stops at its next row (176-KRN-0103): kbench, its output sent to an endpoint that goes.
fn console_closes(run: &mut Run) {
    let Ok(endpoint) = Endpoint::create() else { return run.report("a tool's console closes", fail(String::from("no endpoint created"))) };
    let verdict = match child::start_with_console("kbench", "", endpoint) {
        Err(why) => { let _ = ipc::drop_cap(endpoint.0); fail(why) }
        Ok(pid) => {
            let pieces = (0..4).take_while(|_| endpoint.recv_timeout(0, 10_000).is_ok()).count();
            let t = now();
            let _ = ipc::drop_cap(endpoint.0);
            let gone = child::gone_within(pid, 30_000);
            if !gone && granted(SLOT_LIFECYCLE) { let _ = lifecycle::stop_task(Endpoint(SLOT_LIFECYCLE), pid); }
            match (pieces, gone) {
                (4, true) => pass(format!("kbench ended {} after its console went", report::duration(now() - t))),
                (4, false) => fail(String::from("kbench still ran 30 s after its console went")),
                (n, _) => fail(format!("{} pieces of kbench's output came, not 4", n)),
            }
        }
    };
    run.report("a tool's console closes", verdict);
}

// A launch session that lends every launch slot at once (000-KRN-0101): the program holds them all.
fn every_slot(run: &mut Run) {
    let Ok(endpoint) = Endpoint::create() else { return run.report("launch: every slot", fail(String::from("no endpoint created"))) };
    let all = (1usize << LAUNCH_SLOTS.len()) - 1;
    let verdict = match child::start_in_every_slot("check", "--slots", endpoint).and_then(|pid| child::hello(endpoint).map(|()| pid)) {
        Err(why) => fail(why),
        Ok(pid) => {
            let said = endpoint.recv_timeout(0, 5000);
            let _ = child::gone_within(pid, 2000);
            match said {
                Ok(m) if m.data == [SLOTS, all] => pass(format!("{} slots lent in one session, all held", LAUNCH_SLOTS.len())),
                Ok(m) => fail(format!("slots held {:#x} of {:#x}", m.data[1], all)),
                Err(e) => fail(format!("no answer: {:?}", e)),
            }
        }
    };
    run.report("launch: every slot", verdict);
    let _ = ipc::drop_cap(endpoint.0);
}

// What a `--reader` child said: the bytes it read, their hash and how its input ended.
fn reader_said(endpoint: Endpoint) -> Result<(usize, u64, usize), String> {
    let read = endpoint.recv_timeout(0, 10_000).map_err(|e| format!("no answer from the reader: {:?}", e))?;
    let hash = endpoint.recv_timeout(0, 5000).map_err(|e| format!("no hash from the reader: {:?}", e))?;
    if read.data[0] & 0xFF != READ || hash.data[0] != HASH { return Err(format!("the reader said {:?} {:?}", read.data, hash.data)); }
    Ok((read.data[1], hash.data[1] as u64, read.data[0] >> 8))
}

// A program's text input (000-KRN-0094): none without one; 100 KiB through the kernel's 4 KiB queue to its end; the
// writer's rights; and the end once no writer is left.
fn text_input(run: &mut Run) {
    let t = now();
    let none = mind::io::read(&mut [0u8; 16]);
    run.report("text input: none", match none {
        Err(mind::Error::NotFound) if !mind::io::has_input() => pass(format!("a program started without one is told so in {}", report::duration(now() - t))),
        other => fail(format!("a read without one: {:?}", other)),
    });
    let Ok(endpoint) = Endpoint::create() else { return run.report("text input: to its end", fail(String::from("no endpoint created"))) };
    let data: Vec<u8> = (0..FED_BYTES).map(|i| (i * 31 + i / 4096) as u8).collect();
    let fed = child::start_fed("check", "--reader", endpoint, RECEIVE).and_then(|(pid, writer)| child::hello(endpoint).map(|()| (pid, writer)));
    let (pid, spare) = match fed {
        Err(why) => { run.report("text input: to its end", fail(why)); let _ = ipc::drop_cap(endpoint.0); return; }
        Ok((pid, writer)) => {
            // A copy kept past the close: written through once the reader ended.
            let spare = ipc::mint(writer.handle(), u8::MAX, 0, 0).ok().and_then(mind::io::Writer::new);
            let t = now();
            let written = writer.write_all(&data).and_then(|()| writer.close());
            let verdict = match (written, reader_said(endpoint)) {
                (Err(e), _) => fail(format!("writing: {:?}", e)),
                (_, Err(why)) => fail(why),
                (Ok(()), Ok((count, hash, how))) if count == data.len() && hash == child::fnv(child::FNV_START, &data) && how == READ_END =>
                    pass(format!("{} KiB through a {} KiB queue in {}, every byte; the end seen, and again", count / 1024, TEXT_QUEUE / 1024, report::duration(now() - t))),
                (Ok(()), Ok((count, hash, how))) => fail(format!("{} of {} bytes, hash {}, ended {}", count, data.len(), if hash == child::fnv(child::FNV_START, &data) { "right" } else { "wrong" }, how)),
            };
            run.report("text input: to its end", verdict);
            (pid, spare)
        }
    };
    // Only a writer writes; and nobody once the reader ended.
    let refused = mind::sys::syscall(SYSCALL_TEXT_WRITE, endpoint.0, 0, [data.as_ptr() as usize, 16, 0, 0]).result;
    let gone = child::gone_within(pid, 3000);
    let late = spare.as_ref().map(|w| w.write_some(&data[..16]));
    run.report("text input: rights", match (refused, gone, late) {
        (ERR_RIGHTS, true, Some(Err(mind::Error::Peer))) => pass(String::from("refused through an endpoint, and to a reader that ended")),
        (code, gone, late) => fail(format!("through an endpoint: {:#x}; reader gone: {}; to it then: {:?}", code, gone, late)),
    });
    drop(spare);
    // A writer dropped without a close: the reader still comes to its end, after the bytes queued.
    let verdict = match child::start_fed("check", "--reader", endpoint, RECEIVE).and_then(|(pid, writer)| child::hello(endpoint).map(|()| (pid, writer))) {
        Err(why) => fail(why),
        Ok((pid, writer)) => {
            let written = writer.write_all(&data[..1000]);
            drop(writer);
            let said = reader_said(endpoint);
            let _ = child::gone_within(pid, 2000);
            match (written, said) {
                (Err(e), _) => fail(format!("writing: {:?}", e)),
                (_, Err(why)) => fail(why),
                (Ok(()), Ok((1000, hash, READ_END))) if hash == child::fnv(child::FNV_START, &data[..1000]) => pass(String::from("1000 bytes, then the end once no writer was left")),
                (Ok(()), Ok((count, _, how))) => fail(format!("{} of 1000 bytes, ended {}", count, how)),
            }
        }
    };
    run.report("text input: writer gone", verdict);
    let _ = ipc::drop_cap(endpoint.0);
}

// A copy of check that makes an access the kernel must stop.
fn probe(run: &mut Run, kind: &str, name: &str, what: &str) {
    let Ok(endpoint) = Endpoint::create() else { return run.report(name, fail(String::from("no endpoint created"))) };
    let verdict = match child::start("check", &format!("--probe {}", kind), endpoint).and_then(|pid| child::hello(endpoint).map(|()| pid)) {
        Err(why) => fail(why),
        Ok(pid) => match endpoint.recv_timeout(0, 3000) {
            Ok(m) if m.data[0] == SURVIVED => fail(format!("{} succeeded: the program was not stopped", what)),
            _ if child::gone_within(pid, 2000) => pass(format!("{} faulted; the program was stopped", what)),
            _ => fail(String::from("the probe neither answered nor ended")),
        },
    };
    run.report(name, verdict);
    let _ = ipc::drop_cap(endpoint.0);
}

fn security(run: &mut Run, info: &mind::BootInfo) {
    probe(run, "kernel", "kernel memory unreadable", "the read");
    probe(run, "code", "own code unwritable", "the write");
    probe(run, "null", "null pointer faults", "the read");
    let launch = info.launch;
    let key: String = launch.key.iter().map(|&b| b as char).filter(char::is_ascii_hexdigit).collect();
    run.report("the boot was checked", if launch.images > 0 {
        pass(format!("{} images, key {}{}", launch.images, key, if launch.test_key != 0 { " (test key)" } else { "" }))
    } else if mind::fs::metadata("MANIFEST").is_ok() {
        fail(String::from("MANIFEST is on the boot disk, yet the bootloader checked nothing"))
    } else {
        skip(String::from("no MANIFEST on the boot disk: nothing to check against"))
    });
    let entropy = if cfg!(target_arch = "aarch64") { "RNDR" } else { "RDRAND" };
    run.report("hardware random numbers", if info.cpu_features & FEATURE_ENTROPY != 0 { pass(format!("{} is there", entropy)) } else { skip(format!("no {} on this processor", entropy)) });
}

fn clock(run: &mut Run) {
    let date = mind::rtc::date();
    run.report("the RTC's date", match (date, mind::rtc::seconds_since_midnight()) {
        (Some((y, m, d)), Some(s)) if (2000..2100).contains(&y) => pass(format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, s / 3600, s / 60 % 60, s % 60)),
        (Some((y, _, _)), _) => fail(format!("the year is {}: set the clock (date set)", y)),
        _ => fail(String::from("the RTC service did not answer")),
    });
    // The RTC moves with the monotonic clock: about 1 s over 1.2 s.
    let (s0, t0) = (mind::rtc::seconds_since_midnight(), now());
    while now() - t0 < 1_200_000_000 { mind::time::sleep(100); }
    run.report("the RTC advances", match (s0, mind::rtc::seconds_since_midnight()) {
        (Some(a), Some(b)) => { let moved = (b + 86_400 - a) % 86_400; if (1..=2).contains(&moved) { pass(format!("{} s in {}", moved, report::duration(now() - t0))) } else { fail(format!("{} s in {}", moved, report::duration(now() - t0))) } }
        _ => fail(String::from("the RTC service did not answer")),
    });
    let (_, resolution, hz) = mind::time::clock_info();
    run.report("the monotonic clock", if hz != 0 { pass(format!("{} ns resolution, {} MHz counter", resolution, hz / 1_000_000)) } else { skip(format!("the tick only (resolution {})", report::duration(resolution))) });
    let shorter = (0..20).filter(|_| { let t = now(); mind::time::sleep(10); now() - t < 10_000_000 }).count();
    run.report("sleeps last as asked", if shorter == 0 { pass(String::from("20 sleeps of 10 ms, none shorter")) } else { fail(format!("{}/20 sleeps of 10 ms shorter (000-KRN-0065)", shorter)) });
}

// Writes 64 KiB to `path`, reads it back and removes it.
fn round_trip(path: &str) -> Verdict {
    let data: Vec<u8> = (0..65536usize).map(|i| (i * 13 + i / 256) as u8).collect();
    let result = (|| -> Result<bool, mind::fs::Error> {
        let mut file = mind::fs::File::create(path)?;
        for chunk in data.chunks(4096) { if file.write(chunk)? != chunk.len() { return Ok(false); } }
        file.flush()?;
        drop(file);
        let mut file = mind::fs::File::open(path)?;
        let mut back = alloc::vec![0u8; data.len()];
        let mut got = 0;
        while got < back.len() { match file.read(&mut back[got..])? { 0 => break, n => got += n } }
        drop(file);
        mind::fs::remove(path)?;
        Ok(back[..got] == data[..])
    })();
    match result {
        Ok(true) => pass(String::from("64 KiB written, read back and removed")),
        Ok(false) => { let _ = mind::fs::remove(path); fail(String::from("what was read back differs")) }
        Err(e @ (mind::fs::Error::ReadOnly | mind::fs::Error::Denied)) => skip(format!("not writable here: {:?}", e)),
        Err(mind::fs::Error::NotFound) if path.rsplit_once('/').is_some_and(|(dir, _)| mind::fs::metadata(dir).is_err()) => skip(format!("no {}/ on this disk", path.rsplit_once('/').unwrap().0)),
        Err(e) => { let _ = mind::fs::remove(path); fail(format!("{:?}", e)) }
    }
}

fn files(run: &mut Run) {
    run.report("ram: write, read", round_trip("ram:check.tmp"));
    run.report("data/ write, read", round_trip("data/check.tmp"));
    let mut names = Vec::new();
    run.report("log: the log volume", match mind::fs::list("log:", |e| names.push(String::from_utf8_lossy(e.name).to_lowercase())) {
        Ok(n) => { let boot = report::highest_number(names.iter().map(String::as_str), "boot", ".log"); pass(format!("{} files; this boot's log is boot{:04}.log", n, boot)) }
        Err(mind::fs::Error::NotFound) => skip(String::from("no log volume on this disk")),
        Err(e) => fail(format!("{:?}", e)),
    });
    for (volume, name) in [("", "boot volume consistent"), ("ram", "ram: consistent"), ("log", "log: consistent")] {
        let verdict = match mind::fs::check(volume, |r| (r.files, r.directories, r.lost + r.cross_linked + r.bad_chains + r.sizes + r.bad_entries, String::from(r.first.as_str()))) {
            Ok((files, dirs, 0, _)) => pass(format!("{} files, {} directories, no problem", files, dirs)),
            Ok((_, _, problems, first)) => fail(format!("{} problems, first: {}", problems, first)),
            Err(mind::fs::Error::NotFound) => skip(String::from("no such volume")),
            Err(e) => fail(format!("{:?}", e)),
        };
        run.report(name, verdict);
    }
    // Every program on the boot disk: its ELF accepted by the loader. The list gives at most 16 characters of a name, so
    // a name that long may be cut: such a program is named in the log, not inspected.
    let verdict = match loader::list(Endpoint::LOADER) {
        Ok(list) => {
            let (cut, whole): (Vec<&loader::Program>, Vec<&loader::Program>) = list.as_slice().iter().filter(|p| !p.service).partition(|p| p.name.as_str().len() >= 16);
            let bad: Vec<String> = whole.iter().filter_map(|p| match loader::inspect(Endpoint::LOADER, p.name.as_str()) {
                Ok(Ok(_)) => None,
                other => Some(format!("{} ({:?})", p.name.as_str(), other.map(|r| r.err()))),
            }).collect();
            run.log.detail(&format!("programs: {}", list.as_slice().iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(" ")));
            if !cut.is_empty() { run.log.detail(&format!("not inspected, the list cuts their names at 16 characters: {}", cut.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(" "))); }
            let uncut = if cut.is_empty() { String::new() } else { format!("; {} long names not checked", cut.len()) };
            if bad.is_empty() { pass(format!("{} programs accepted{}", whole.len(), uncut)) } else { fail(format!("refused: {}", bad.join(", "))) }
        }
        Err(e) => fail(format!("the loader did not answer: {:?}", e)),
    };
    run.report("programs on the disk", verdict);
}

fn services(run: &mut Run, info: &mind::BootInfo) {
    if !granted(SLOT_LIFECYCLE) { return run.report("boot services", skip(String::from("no lifecycle client: start check from the shell"))); }
    // What init says of this boot (init.wit 1.4, 000-KRN-0102), against what the bootloader handed the kernel.
    let slot = info.boot_slot;
    run.report("the boot", match lifecycle::boot(Endpoint(SLOT_LIFECYCLE)) {
        Ok(Ok(b)) if b.slot as u32 != slot.slot || b.trial != (slot.trial != 0) => fail(format!("init says slot {}, trial {}; the bootloader said slot {}, trial {}", b.slot, b.trial, slot.slot, slot.trial != 0)),
        Ok(Ok(b)) if b.trial && !b.confirmed => fail(String::from("a trial boot init did not confirm: the machine restarts at the deadline")),
        Ok(Ok(b)) if !b.trial && b.confirmed => fail(String::from("init says it confirmed a boot that was not on trial")),
        Ok(Ok(b)) => pass(format!("{}, {}", match b.slot as u32 { BOOT_SLOT_A => "slot A", BOOT_SLOT_B => "slot B", _ => "the volume's root" }, if b.trial { "on trial, confirmed" } else { "not on trial" })),
        other => fail(format!("init did not say: {:?}", other.map(|r| r.err()))),
    });
    // The machine's modes (init.wit 1.5, 750-KRN-0143): any client reads them; only the shell's mode client changes them.
    run.report("the modes", match lifecycle::mode(Endpoint(SLOT_LIFECYCLE)) {
        Ok(Ok(m)) => pass(format!("{}, as set by {}", match (m.development, m.maintenance) { (true, true) => "development and maintenance", (true, false) => "development", (false, true) => "maintenance", _ => "normal" }, m.by.as_str())),
        other => fail(format!("init did not say: {:?}", other.map(|r| r.err()))),
    });
    run.report("a mode change refused", match lifecycle::set_mode(Endpoint(SLOT_LIFECYCLE), lifecycle::Mode::Maintenance, true, "check: a lifecycle client may not") {
        Ok(Err(lifecycle::Error::Denied)) => pass(String::from("denied to a lifecycle client")),
        Ok(Ok(())) => {
            let _ = lifecycle::set_mode(Endpoint(SLOT_LIFECYCLE), lifecycle::Mode::Maintenance, false, "check: undoing a change it should not have made");
            fail(String::from("init turned the maintenance mode on for a lifecycle client"))
        }
        other => fail(format!("not denied: {:?}", other.map(|r| r.err()))),
    });
    // 800-KRN-0144: replacing a service is link's developer client's alone; nothing is stopped or read for another.
    run.report("a replace refused", match lifecycle::replace(Endpoint(SLOT_LIFECYCLE), "rtc", "check.elf") {
        Ok(Err(lifecycle::Error::Denied)) => pass(String::from("denied to a lifecycle client")),
        other => fail(format!("not denied: {:?}", other.map(|r| r.err()))),
    });
    match lifecycle::list(Endpoint(SLOT_LIFECYCLE)) {
        Ok(Ok(list)) => for s in list.as_slice() {
            let verdict = if s.running {
                pass(format!("PID {}{}", s.pid, if s.starts > 1 { format!(", started {} times", s.starts) } else { String::new() }))
            } else if s.quarantined {
                fail(format!("quarantined after {} starts", s.starts))
            } else if s.starts == 0 {
                skip(String::from("not started (no device, or not planned)"))
            } else {
                skip(format!("stopped after {} starts (by request, or it ended)", s.starts))
            };
            run.log.detail(&format!("    {}: holds {}", s.name.as_str(), s.holds.as_str()));
            run.report(s.name.as_str(), verdict);
        },
        other => run.report("boot services", fail(format!("init did not list them: {:?}", other.map(|r| r.err())))),
    }
}

// Every task's name by PID, from sysmon.
fn task_names() -> Vec<(u64, String)> {
    let mut out = Vec::new();
    let mut start = 0;
    while let Ok(Ok(tasks)) = sysinfo::tasks(SYSMON, start) {
        out.extend(tasks.as_slice().iter().map(|t| (t.pid, String::from(t.name.as_str()))));
        if tasks.len() < 40 { break; }
        start += tasks.len() as u32;
    }
    out
}

fn devices(run: &mut Run) {
    if !granted(SLOT_SYSINFO) { return run.report("PCI devices", skip(String::from("no sysmon client: start check from the shell"))); }
    let names = task_names();
    let Ok(Ok(list)) = sysinfo::devices(SYSMON) else { return run.report("PCI devices", fail(String::from("sysmon did not list them"))) };
    let mut bridges = 0;
    for d in list.as_slice() {
        let place = format!("{:02x}:{:02x}.{} {}", d.location >> 8, d.location >> 3 & 31, d.location & 7, mind::stat::class_name(d.class));
        run.log.detail(&format!("    {} class {:06x} irq {} holder {} bars {:x} {:x} {:x}", place, d.class >> 8, d.irq, d.holder, d.bar0, d.bar1, d.bar2));
        if d.class >> 24 == 0x06 && d.holder == 0 { bridges += 1; continue; }
        let verdict = match names.iter().find(|(pid, _)| *pid == d.holder) {
            Some((pid, name)) if d.holder != 0 => pass(format!("{} (PID {})", name, pid)),
            _ if d.holder != 0 => pass(format!("PID {}", d.holder)),
            _ => skip(String::from("no driver holds it")),
        };
        run.report(&place, verdict);
    }
    if bridges > 0 { run.report("bridges", pass(format!("{} bridges, nothing to drive", bridges))); }
    if list.is_empty() { run.report("PCI devices", skip(String::from("none listed"))); }
}

fn sound(run: &mut Run) {
    match mind::audio::info() {
        Ok(i) if i.present => run.report("sound device", pass(format!("{} Hz", i.rate))),
        Ok(_) => return run.report("sound device", skip(String::from("none here"))),
        Err(e) => return run.report("sound device", skip(format!("no audio service here ({:?})", e))),
    }
    run.report("speaker", match mind::audio::tone(880, 150) {
        Ok(()) => pass(String::from("a 150 ms tone at 880 Hz played: it should have been heard")),
        Err(e) => fail(format!("{:?}", e)),
    });
    mind::time::sleep(200);
    run.report("microphone", match mind::audio::record_start() {
        Err(mind::Error::Other(ERR_BUSY)) => skip(String::from("another program records")),
        Err(e) => skip(format!("no capture: {:?}", e)),
        Ok(()) => {
            let (mut buffer, mut count, mut peak, t) = (alloc::vec![0i16; 8192], 0usize, 0i32, now());
            while now() - t < 600_000_000 {
                match mind::audio::record_read(&mut buffer) {
                    Ok((n, _)) => { count += n; peak = buffer[..n].iter().fold(peak, |p, &s| p.max((s as i32).abs())); if n == 0 { mind::time::sleep(20); } }
                    Err(_) => break,
                }
            }
            let _ = mind::audio::record_stop();
            if count == 0 { fail(String::from("no input from the microphone in 0.6 s")) }
            else if peak < 64 { skip(format!("silent: peak {} of 32767 in {} samples (quiet, or the microphone is off)", peak, count)) }
            else { pass(format!("{} samples in 0.6 s, peak {} of 32767", count, peak)) }
        }
    });
}

fn network(run: &mut Run) {
    if !granted(SLOT_NETWORK) { return run.report("network", skip(String::from("no flow grant from netpolicy for check"))); }
    let config = match socket::config(STACK) {
        Ok(Ok(c)) if c.address != 0 => { run.report("address", pass(format!("{}/{} via {}, DNS {}, {}", ip(c.address), c.prefix, ip(c.gateway), ip(c.dns), if c.dhcp { "DHCP" } else { "static" }))); c }
        Ok(Ok(_)) => return run.report("address", fail(String::from("not configured"))),
        other => return run.report("address", fail(format!("{:?}", other.map(|r| r.err())))),
    };
    let refused = |e: socket::Error| if e == socket::Error::Denied { skip(String::from("the policy does not let check reach it")) } else { fail(format!("{:?}", e)) };
    run.report("the gateway answers", match socket::ping(STACK, config.gateway, 2000) {
        Ok(Ok(ms)) => pass(format!("{} answered in {} ms", ip(config.gateway), ms)),
        Ok(Err(e)) => refused(e),
        Err(e) => fail(format!("{:?}", e)),
    });
    run.report("a name resolves", match socket::resolve(STACK, "example.com", config.dns, 53, 3000) {
        Ok(Ok(a)) => pass(format!("example.com is {}", ip(a))),
        Ok(Err(e)) => refused(e),
        Err(e) => fail(format!("{:?}", e)),
    });
}

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("check — a self-test of what is done: the kernel, security, clocks, files and programs, boot services, devices, sound, the network.\nUsage: check [kernel|security|clock|files|services|devices|sound|network …]   (default: every group)\n✓ passed, ✗ failed with the reason, ○ skipped with the reason; the exit status is 1 if any failed. Details in log:checkNNNN.txt (ram: without the log volume).");
    let args = mind::process::args_str().trim();
    if args == "--child" { return child::serve(); }
    if args == "--reader" { return child::read_input(); }
    if args == "--slots" { return child::report_slots(); }
    if let Some(code) = args.strip_prefix("--exit ") { mind::process::exit_with(code.parse().unwrap_or(1)); }
    if let Some(kind) = args.strip_prefix("--probe ") { return child::probe(kind); }
    let chosen: Vec<&str> = args.split_whitespace().collect();
    if let Some(unknown) = chosen.iter().find(|w| !GROUPS.iter().any(|(name, _)| name == *w)) { mind::println!("check: no group {}\n{}", unknown, USAGE); mind::process::exit_with(2); }
    let mut run = Run { log: Log::new("check"), table: Table::new(&[(1, Align::Left), (24, Align::Left), (44, Align::Left)]), verdicts: Vec::new(), names: Vec::new() };
    run.log.both("check — a self-test of what is done (176-KRN-0063)");
    for line in out::machine(info) { run.log.both(&line); }
    run.log.log(&format!("arguments: {}", args));
    let (top, middle, bottom) = (run.table.top(), run.table.middle(), run.table.bottom());
    let header = run.table.row(&["", "check", "result"]);
    run.log.both(&top);
    run.log.both(&header);
    run.log.both(&middle);
    for (name, title) in GROUPS {
        if !chosen.is_empty() && !chosen.contains(&name) { continue; }
        run.group(title);
        match name {
            "kernel" => kernel(&mut run),
            "security" => security(&mut run, info),
            "clock" => clock(&mut run),
            "files" => files(&mut run),
            "services" => services(&mut run, info),
            "devices" => devices(&mut run),
            "sound" => sound(&mut run),
            _ => network(&mut run),
        }
    }
    run.log.both(&bottom);
    let (passed, failed, skipped) = report::tally(&run.verdicts);
    for (name, v) in run.names.iter().zip(&run.verdicts).filter(|(_, v)| matches!(v, Verdict::Failed(_))) { mind::println!("  ✗ {}: {}", name, v.detail()); }
    let summary = format!("check: {} passed, {} failed, {} skipped ({} checks), {}", passed, failed, skipped, run.verdicts.len(), report::duration(run.log.elapsed()));
    run.log.log(&summary);
    match run.log.save() {
        Ok(path) => mind::println!("{}; full log: {}", summary, path),
        Err(why) => mind::println!("{}; the log was not written ({})", summary, why),
    }
    if failed > 0 { mind::process::exit_with(1); }
}
