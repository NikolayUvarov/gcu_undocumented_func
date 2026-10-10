#![no_std]
#![no_main]
// bench [group …] [--quick]: the components' performance (176-KRN-0064), in kbench's table: files on each volume,
// round trips to the services, SHA-256, the camera's frames. Rates go under the table; every statistic with a
// histogram goes to log:benchNNNN.txt.
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use bench::measure::{batches, now, Run};
use bench::report;
use core::hint::black_box;
use mind::abi::{CAP_GRANT, CAP_KIND_ENDPOINT, CAP_READ, CAP_WRITE, SLOT_CAMERA, SLOT_SYSINFO};
use mind::fs::File;
use mind::idl::{loader, sysinfo, video};
use mind::ipc::Endpoint;
use mind::mem::Pages;

mind::request!(REQUEST_CONSOLE | REQUEST_FILES | REQUEST_SYSINFO | REQUEST_CAMERA);

const GROUPS: [(&str, &str); 4] = [("files", "Files"), ("services", "Service round trips"), ("crypto", "Hashing"), ("camera", "The camera")];
const USAGE: &str = "usage: bench [files|services|crypto|camera …] [--quick]";
const MIB: usize = 1024 * 1024;
const BIG: usize = 2 * MIB;
const SMALL: usize = 1024;
const CHUNK: usize = 16 * 1024;
const CAMERA: Endpoint = Endpoint(SLOT_CAMERA);

fn granted(slot: usize) -> bool { mind::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT }

// Writes `data` to a new `path` in chunks and flushes it.
fn write(path: &str, data: &[u8]) -> Result<(), mind::fs::Error> {
    let mut file = File::create(path)?;
    for chunk in data.chunks(CHUNK) { if file.write(chunk)? != chunk.len() { return Err(mind::fs::Error::NoSpace); } }
    file.flush()
}

// Reads `path` to its end; the bytes read.
fn read(path: &str, buffer: &mut [u8]) -> Result<usize, mind::fs::Error> {
    let mut file = File::open(path)?;
    let mut total = 0;
    loop { match file.read(&mut buffer[..CHUNK])? { 0 => return Ok(total), n => total += n } }
}

// Writing and reading BIG bytes and a cycle of small files on one volume.
fn volume(run: &mut Run, label: &str, dir: &str) {
    let big = format!("{}bench.tmp", dir);
    let data: Vec<u8> = (0..BIG).map(|i| (i * 31 + i / 4096) as u8).collect();
    match write(&big, &data[..SMALL]) {
        Ok(()) => {}
        Err(e @ (mind::fs::Error::NotFound | mind::fs::Error::ReadOnly | mind::fs::Error::Denied | mind::fs::Error::NoService)) => {
            let _ = mind::fs::remove(&big);
            return run.skip(&format!("{} files", label), &format!("not writable here: {:?}", e));
        }
        Err(e) => { let _ = mind::fs::remove(&big); return run.fail(&format!("{} files", label), &format!("{:?}", e)); }
    }
    let n = run.reps(8);
    let mut buffer = alloc::vec![0u8; CHUNK];
    let (mut writes, mut reads, mut error) = (Vec::new(), Vec::new(), None);
    for _ in 0..n {
        let t = now();
        if let Err(e) = write(&big, &data) { error = Some(format!("write: {:?}", e)); break; }
        writes.push(now() - t);
        let t = now();
        match read(&big, &mut buffer) { Ok(got) if got == BIG => reads.push(now() - t), other => { error = Some(format!("read: {:?}", other)); break; } }
    }
    let _ = mind::fs::remove(&big);
    if let Some(why) = error { return run.fail(&format!("{} 2 MiB", label), &why); }
    let w = run.row(&format!("{} write 2 MiB", label), &mut writes, 1);
    let r = run.row(&format!("{} read 2 MiB", label), &mut reads, 1);
    // A small file's life: created, written, read, removed.
    let mut errors = 0;
    let mut cycles = batches(run.reps(20), 10, || {
        let path = format!("{}bench-small.tmp", dir);
        if write(&path, &data[..SMALL]).is_err() || read(&path, &mut buffer).map_or(true, |got| got != SMALL) || mind::fs::remove(&path).is_err() { errors += 1; }
    });
    let c = if errors == 0 { Some(run.row(&format!("{} 1 KiB file cycle", label), &mut cycles, 10)) } else { run.fail(&format!("{} 1 KiB file cycle", label), &format!("{} cycles failed", errors)); None };
    run.note(format!("{} write {}, read {} (vfs_server's cache may serve reads){}", label, report::rate(BIG as u64, w.median), report::rate(BIG as u64, r.median),
        c.map_or(String::new(), |c| format!(", {} small files a second", 1_000_000_000 / c.median.max(1)))));
}

fn files(run: &mut Run) {
    volume(run, "ram:", "ram:");
    volume(run, "data/", "data/");
    volume(run, "log:", "log:");
}

// A round trip measured in batches; `op` says whether it succeeded.
fn round_trips(run: &mut Run, name: &str, samples: usize, batch: usize, mut op: impl FnMut() -> bool) {
    let mut failed = 0;
    let mut times = batches(samples, batch, || { if !op() { failed += 1; } });
    if failed == 0 { run.row(name, &mut times, batch); } else { run.fail(name, &format!("{} of {} failed", failed, samples * batch)); }
}

fn services(run: &mut Run) {
    let n = run.reps(500);
    let _ = write("ram:bench-stat.tmp", b"bench");
    round_trips(run, "vfs_server: attributes", n, 4, || mind::fs::metadata("ram:bench-stat.tmp").is_ok());
    let _ = mind::fs::remove("ram:bench-stat.tmp");
    // sysmon admits 40 requests a second from a client (MC-10.2): one call every 40 ms, the pause not timed.
    if granted(SLOT_SYSINFO) {
        let (mut times, mut failed) = (Vec::new(), 0);
        for _ in 0..run.reps(40) {
            let t = now();
            if matches!(sysinfo::memory(Endpoint(SLOT_SYSINFO)), Ok(Ok(_))) { times.push(now() - t); } else { failed += 1; }
            mind::time::sleep(40);
        }
        if failed == 0 { run.row("sysmon: memory figures", &mut times, 1); } else { run.fail("sysmon: memory figures", &format!("{} refused", failed)); }
    } else { run.skip("sysmon: memory figures", "no sysmon client"); }
    round_trips(run, "rtc: the time", n, 4, || mind::rtc::seconds_since_midnight().is_some());
    round_trips(run, "loader: inspect", run.reps(100), 2, || matches!(loader::inspect(Endpoint::LOADER, "bench"), Ok(Ok(_))));
    match mind::audio::info() {
        Ok(_) => round_trips(run, "audio_gw: the device", n, 4, || mind::audio::info().is_ok()),
        Err(e) => run.skip("audio_gw: the device", &format!("no audio service ({:?})", e)),
    }
}

fn crypto(run: &mut Run) {
    let data: Vec<u8> = (0..4 * MIB).map(|i| (i * 7) as u8).collect();
    let mut samples: Vec<u64> = (0..run.reps(10)).map(|_| { let t = now(); black_box(mind::sha256::digest(black_box(&data))); now() - t }).collect();
    let stats = run.row("SHA-256 of 4 MiB", &mut samples, 1);
    run.note(format!("SHA-256 at {} (median)", report::rate(4 * MIB as u64, stats.median)));
    let mut samples = batches(run.reps(500), 16, || { black_box(mind::sha256::digest(black_box(&data[..64]))); });
    run.row("SHA-256 of 64 bytes", &mut samples, 16);
}

fn camera(run: &mut Run) {
    if !granted(SLOT_CAMERA) { return run.skip("frames", "no camera was lent (none, or not allowed)"); }
    let cameras = video::cameras(CAMERA).unwrap_or_default();
    let Some(camera) = cameras.as_slice().first() else { return run.skip("frames", "the video gateway lists no camera") };
    let (width, height) = ((camera.width as usize).min(640), (camera.height as usize).min(480));
    match video::open(CAMERA, 0, width as u16, height as u16, camera.rate) {
        Ok(Ok(())) => {}
        other => return run.fail("frames", &format!("open {}x{} {}/s: {:?}", width, height, camera.rate, other.map(|r| r.err()))),
    }
    let bytes = width * height * 4;
    let Some(pages) = Pages::new(bytes) else { let _ = video::close(CAMERA); return run.fail("frames", "no memory for a frame") };
    let Ok(cap) = pages.share() else { let _ = video::close(CAMERA); return run.fail("frames", "cannot share the buffer") };
    let (seconds, mut intervals, mut last, mut previous, mut frames, mut gaps) = (if run.quick { 1 } else { 3 }, Vec::new(), 0u64, 0u32, 0u32, 0u32);
    let started = now();
    let mut error = None;
    while now() - started < seconds * 1_000_000_000 {
        let read = mind::ipc::mint(cap, CAP_READ | CAP_WRITE | CAP_GRANT, 0, 0).and_then(|lent| video::read(CAMERA, bytes as u32, lent));
        let _ = mind::ipc::revoke(cap);
        match read {
            Ok(Ok(sequence)) => {
                let t = now();
                if frames > 0 { intervals.push(t - last); gaps += sequence.saturating_sub(previous + 1); }
                (last, previous) = (t, sequence);
                frames += 1;
            }
            other => { error = Some(format!("read: {:?}", other.map(|r| r.err()))); break; }
        }
    }
    let _ = video::close(CAMERA);
    let _ = mind::ipc::drop_cap(cap);
    drop(pages);
    if let Some(why) = error { return run.fail("frames", &why); }
    if intervals.is_empty() { return run.fail("frames", "no frame in the time"); }
    let stats = run.row(&format!("frame interval {}x{}", width, height), &mut intervals, 1);
    run.note(format!("{}: {} frames in {} s at {}x{} (asked {}/s): {}.{} a second (median interval {}), {} skipped by the gateway",
        camera.name.as_str(), frames, seconds, width, height, camera.rate, 1_000_000_000 / stats.median.max(1), 10_000_000_000 / stats.median.max(1) % 10,
        report::duration(stats.median), gaps));
}

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("bench — the components' performance: files on ram:, data/ and log:, round trips to the services, SHA-256, the camera.\nUsage: bench [files|services|crypto|camera …] [--quick]   (default: every group)\nkbench's table on the screen and the rates under it; every statistic and a histogram in log:benchNNNN.txt (ram: without the log volume). --quick: a tenth of the repetitions.");
    let args = mind::process::args_str().trim();
    let words: Vec<&str> = args.split_whitespace().collect();
    let quick = words.contains(&"--quick");
    let chosen: Vec<&str> = words.iter().copied().filter(|w| *w != "--quick").collect();
    if let Some(unknown) = chosen.iter().find(|w| !GROUPS.iter().any(|(name, _)| name == *w)) { mind::println!("bench: no group {}\n{}", unknown, USAGE); mind::process::exit_with(2); }
    let mut run = Run::start("bench", "bench — the components' performance (176-KRN-0064)", info, args, quick);
    for (name, title) in GROUPS {
        if !chosen.is_empty() && !chosen.contains(&name) { continue; }
        run.group(title);
        match name {
            "files" => files(&mut run),
            "services" => services(&mut run),
            "crypto" => crypto(&mut run),
            _ => camera(&mut run),
        }
    }
    run.finish("bench");
}
