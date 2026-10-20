//! mindlink: MIND Link's end on Linux and WSL (800-LNK-0004). As master it moves files, follows the log and types at
//! the other end's console; `serve` plays the slave over a directory. docs/link/mindlink.md says how to use it.
mod dir;
mod port;

use linkproto::app::{Client, FileService};
use linkproto::endpoint::code;
use linkproto::frame::MAX_WIRE;
use linkproto::{Error, Slave};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const USAGE: &str = "usage: mindlink [options] <port> <command> [arguments]

port:     a serial port (/dev/ttyUSB0), tcp:HOST:PORT, listen:PORT (one connection; 0 picks a port),
          or pty (serve only: a pseudo-terminal for tests)
commands: hello                       the other end's name and version
          put <file> [<name>] [--force]  send a file; skipped when the other end has the same bytes, unless --force
          get <name> [<file>]         fetch a file
          ls                          the other end's files
          stat <name>                 a file's size and SHA-256
          rm <name>                   delete a file
          log [-f]                    the other end's log; -f follows it
          console                     type at the other end's console (lines from standard input, Ctrl-D ends)
          board <line>                a MIND Link board's control port: one command (hello, status, type <text>,
                                      key <chord>, move <dx> <dy>, click, abs <x> <y>; docs/link/multi-device.md)
          serve <dir> [--log <file>] [--console <command>]
                                      be the slave: files in <dir>, the log from <file>, a console running <command>
options:  --baud <rate>               serial rate (default 115200)
          --timeout <ms>              how long one try waits for an answer (default from the rate)
          --tries <n>                 tries for each request (default 8)
          --turnaround <us>           serve: pause before each answer, for RS-485 converters (default 500)
          --payload <bytes>           the largest frame's payload, 64 to 1024 (default 1024): smaller for a noisy line,
                                      or for QEMU's emulated converter, whose buffer holds 384 bytes";

pub fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }

fn refusal(c: u8) -> &'static str {
    match c {
        code::NOT_FOUND => "no such file",
        code::DENIED => "not allowed (a file's name is one component, without / \\ :)",
        code::IO => "the other end's storage failed",
        code::MISMATCH => "SHA-256 does not match: the file was not kept",
        code::BAD_REQUEST => "the other end did not take the request",
        code::BUSY => "the other end is busy",
        code::UNSUPPORTED => "the other end does not serve this",
        _ => "refused",
    }
}

fn explain(e: Error) -> String {
    match e {
        Error::Timeout => "no answer: check the port, the rate on both ends and the wiring (A to A, B to B)".into(),
        Error::Line => "the line failed (closed, or the converter was pulled out)".into(),
        Error::TooBig => "too big for one frame".into(),
        Error::Protocol => "the other end answered out of protocol".into(),
        Error::Refused(c) => format!("{} (code {c})", refusal(c)),
    }
}

fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("mindlink: {msg}");
    std::process::exit(1)
}

extern "C" { fn isatty(fd: i32) -> i32; }

/// Progress goes to standard error only when a person watches it.
fn progress(text: String) { if unsafe { isatty(2) } == 1 { eprint!("\r{text}   "); } }

fn rate(bytes: u64, t: Duration) -> String {
    let s = t.as_secs_f64().max(0.001);
    format!("{:.1} s, {:.1} KB/s", s, bytes as f64 / s / 1000.0)
}

struct Options { baud: u32, timeout: Option<u32>, tries: u32, turnaround_us: u64, payload: usize }

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut o = Options { baud: 115200, timeout: None, tries: 8, turnaround_us: 500, payload: linkproto::frame::MAX_PAYLOAD };
    let num = |args: &mut Vec<String>, i: usize| -> u64 {
        let v = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or_else(|| die(format!("{} takes a number", args[i])));
        args.drain(i..i + 2);
        v
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--baud" => o.baud = num(&mut args, i) as u32,
            "--timeout" => o.timeout = Some(num(&mut args, i) as u32),
            "--tries" => o.tries = num(&mut args, i).max(1) as u32,
            "--turnaround" => o.turnaround_us = num(&mut args, i),
            "--payload" => o.payload = num(&mut args, i) as usize,
            "-h" | "--help" => { println!("{USAGE}"); return; }
            _ => i += 1,
        }
    }
    if args.len() < 2 { eprintln!("{USAGE}"); std::process::exit(2); }
    let spec = args.remove(0);
    let cmd = args.remove(0);
    let mut port = port::Port::open(&spec, o.baud).unwrap_or_else(|e| die(e));
    if cmd == "serve" { return serve(port, &args, &o); }
    if cmd == "board" { return board(port, &args.join(" ")); }

    // One try waits for a request and an answer of the largest size at the line's rate, and a margin.
    let serial = !spec.contains(':');
    let wait = o.timeout.unwrap_or(if serial { 200 + (4 * MAX_WIRE as u64 * 10 * 1000 / o.baud as u64) as u32 } else { 2000 });
    port.turnaround = Duration::ZERO;
    let latency = if serial { port.latency_ms() } else { None };
    let mut c = Client::new(port);
    c.master.timeout_ms = wait;
    c.master.tries = o.tries;
    c.max_payload = o.payload;
    let mut theirs = [0u8; 64];
    let (hello, n) = c.hello("mindlink", &mut theirs).unwrap_or_else(|e| die(format!("{spec}: {}", explain(e))));
    let result = match (cmd.as_str(), args.iter().map(String::as_str).collect::<Vec<_>>().as_slice()) {
        ("hello", []) => {
            println!("{spec}: \"{}\", protocol {}, payload up to {} bytes, {} ms a try, {} tries",
                     String::from_utf8_lossy(&theirs[..n]), hello.version, hello.max_payload, wait, o.tries);
            if let Some(l) = latency.filter(|&l| l > 2) {
                println!("note: the FTDI latency timer is {l} ms, which slows every answer; as root: echo 1 > /sys/bus/usb-serial/devices/<tty>/latency_timer");
            }
            Ok(())
        }
        ("put", [file, rest @ ..]) => {
            let force = rest.contains(&"--force");
            let name = rest.iter().find(|a| **a != "--force").copied();
            put(&mut c, Path::new(file), name, force)
        }
        ("get", [name]) => get(&mut c, name, Path::new(name)),
        ("get", [name, file]) => get(&mut c, name, Path::new(file)),
        ("ls", []) => ls(&mut c),
        ("stat", [name]) => stat(&mut c, name).map(|(size, d)| println!("{name}\t{size}\t{}", hex(&d))),
        ("rm", [name]) => c.delete(name).map(|_| println!("deleted {name}")),
        ("log", []) => log(&mut c, false),
        ("log", ["-f"]) => log(&mut c, true),
        ("console", []) => console(&mut c),
        _ => { eprintln!("{USAGE}"); std::process::exit(2); }
    };
    if let Err(e) = result { die(explain(e)); }
}

/// The other end hashes the whole file to answer, which on a slow store takes seconds: each try waits at least 15 s.
fn stat(c: &mut Client<port::Port>, name: &str) -> Result<(u64, [u8; 32]), Error> {
    let wait = c.master.timeout_ms;
    c.master.timeout_ms = wait.max(15_000);
    let r = c.stat(name);
    c.master.timeout_ms = wait;
    r
}

fn put(c: &mut Client<port::Port>, path: &Path, name: Option<&str>, force: bool) -> Result<(), Error> {
    let mut file = File::open(path).unwrap_or_else(|e| die(format!("{}: {e}", path.display())));
    let size = file.metadata().map(|m| m.len()).unwrap_or_else(|e| die(e));
    let name = name.map(str::to_string).unwrap_or_else(|| path.file_name().and_then(|n| n.to_str()).unwrap_or_else(|| die("give the file a name")).to_string());
    if !force {
        if let Ok((their_size, digest)) = stat(c, &name) {
            let mut sha = linkproto::sha256::Sha256::new();
            let mut buf = vec![0u8; 1 << 16];
            loop { let n = file.read(&mut buf).unwrap_or_else(|e| die(e)); if n == 0 { break; } sha.update(&buf[..n]); }
            if their_size == size && sha.finish() == digest {
                println!("{name}: the other end has these {size} bytes already (SHA-256 {}), not sent", hex(&digest));
                return Ok(());
            }
        }
    }
    let start = Instant::now();
    let mut shown = Instant::now();
    let (digest, resumed) = c.put(&name, size, |offset, out| {
        if shown.elapsed() > Duration::from_secs(1) {
            progress(format!("{name}: {offset} of {size} bytes, {}", rate(offset, start.elapsed())));
            shown = Instant::now();
        }
        file.seek(SeekFrom::Start(offset)).ok();
        file.read(out).unwrap_or(0)
    })?;
    let stats = c.master.stats;
    progress(String::new());
    println!("put {} as {name}: {size} bytes, {}, SHA-256 {}{}{}", path.display(), rate(size, start.elapsed()), hex(&digest),
             if resumed > 0 { format!(", resumed from {resumed}") } else { String::new() },
             if stats.retries > 0 { format!(", {} requests sent again", stats.retries) } else { String::new() });
    Ok(())
}

fn get(c: &mut Client<port::Port>, name: &str, path: &Path) -> Result<(), Error> {
    let path = if path.is_dir() { path.join(name) } else { path.to_path_buf() };
    let part = PathBuf::from(format!("{}.part", path.display()));
    let mut file = File::create(&part).unwrap_or_else(|e| die(format!("{}: {e}", part.display())));
    let start = Instant::now();
    let mut shown = Instant::now();
    let got = c.get(name, |offset, data| {
        if shown.elapsed() > Duration::from_secs(1) {
            progress(format!("{name}: {offset} bytes, {}", rate(offset, start.elapsed())));
            shown = Instant::now();
        }
        file.write_all(data).is_ok()
    });
    progress(String::new());
    let (size, digest) = match got {
        Ok(v) => v,
        Err(e) => { fs::remove_file(&part).ok(); return Err(e); }
    };
    file.sync_all().ok();
    fs::rename(&part, &path).unwrap_or_else(|e| die(format!("{}: {e}", path.display())));
    let stats = c.master.stats;
    println!("got {name} as {}: {size} bytes, {}, SHA-256 {}{}", path.display(), rate(size, start.elapsed()), hex(&digest),
             if stats.retries > 0 { format!(", {} requests sent again", stats.retries) } else { String::new() });
    Ok(())
}

fn ls(c: &mut Client<port::Port>) -> Result<(), Error> {
    let mut out = vec![0u8; linkproto::frame::MAX_PAYLOAD];
    let mut cursor = 0;
    loop {
        let (n, next) = c.list(cursor, &mut out)?;
        for line in String::from_utf8_lossy(&out[..n]).lines() {
            let (name, size) = line.split_once('\t').unwrap_or((line, ""));
            println!("{size:>12}  {name}");
        }
        match next { Some(n) => cursor = n, None => return Ok(()) }
    }
}

fn log(c: &mut Client<port::Port>, follow: bool) -> Result<(), Error> {
    let mut out = vec![0u8; linkproto::frame::MAX_PAYLOAD];
    let mut cursor = 0;
    let mut stdout = std::io::stdout();
    loop {
        let (n, next) = c.log(cursor, &mut out)?;
        stdout.write_all(&out[..n]).ok();
        stdout.flush().ok();
        cursor = next;
        if n == 0 {
            if !follow { return Ok(()); }
            std::thread::sleep(Duration::from_millis(300));
        }
    }
}

fn console(c: &mut Client<port::Port>) -> Result<(), Error> {
    let (tx, rx) = std::sync::mpsc::channel::<Option<Vec<u8>>>();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 512];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) | Err(_) => { tx.send(None).ok(); return; }
                Ok(n) => if tx.send(Some(buf[..n].to_vec())).is_err() { return; },
            }
        }
    });
    let mut out = vec![0u8; linkproto::frame::MAX_PAYLOAD];
    let mut stdout = std::io::stdout();
    let mut pending: Vec<u8> = Vec::new();
    let mut ended: Option<Instant> = None;
    loop {
        while let Ok(m) = rx.try_recv() {
            match m { Some(b) => pending.extend_from_slice(&b), None => ended = ended.or(Some(Instant::now())) }
        }
        let take = pending.len().min(c.chunk);
        let input: Vec<u8> = pending.drain(..take).collect();
        let n = c.console(&input, &mut out)?;
        stdout.write_all(&out[..n]).ok();
        stdout.flush().ok();
        // After the input ends, the output that follows within a second is still shown.
        if let Some(t) = ended { if pending.is_empty() && n == 0 && t.elapsed() > Duration::from_secs(1) { return Ok(()); } }
        if n == 0 && input.is_empty() { std::thread::sleep(Duration::from_millis(100)); }
    }
}

/// One line to a multi-device's control port, and its answer (800-LNK-0005); the indicator's `@` lines have none.
fn board(mut port: port::Port, line: &str) {
    use linkproto::Line;
    if line.is_empty() || line.contains('\n') { eprintln!("{USAGE}"); std::process::exit(2); }
    // Whatever an earlier command left unread goes first.
    let mut buf = [0u8; 256];
    while port.read(&mut buf, 0) > 0 {}
    if !port.write(format!("{line}\n").as_bytes()) { die("the control port failed"); }
    if line.starts_with('@') { return; }
    let mut answer = Vec::new();
    let end = Instant::now() + Duration::from_secs(2);
    while !answer.ends_with(b"\n") && Instant::now() < end {
        let n = port.read(&mut buf, 100);
        answer.extend_from_slice(&buf[..n]);
    }
    let text = String::from_utf8_lossy(&answer);
    print!("{text}");
    if text.is_empty() { die("the board did not answer: is this its control port (interface 4)?"); }
    if text.starts_with("err") { std::process::exit(1); }
}

fn serve(mut port: port::Port, args: &[String], o: &Options) {
    let mut dir = None;
    let (mut log, mut cmd) = (None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--log" => log = Some(PathBuf::from(it.next().unwrap_or_else(|| die("--log takes a file")))),
            "--console" => cmd = Some(it.next().unwrap_or_else(|| die("--console takes a command")).clone()),
            d if dir.is_none() => dir = Some(PathBuf::from(d)),
            _ => { eprintln!("{USAGE}"); std::process::exit(2); }
        }
    }
    let dir = dir.unwrap_or_else(|| { eprintln!("{USAGE}"); std::process::exit(2) });
    let files = dir::Dir::new(dir.clone(), log, cmd.as_deref()).unwrap_or_else(|e| die(e));
    port.turnaround = Duration::from_micros(o.turnaround_us);
    let name = port.name.clone();
    let mut service = FileService::new(files);
    let host = fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
    service.set_name(&format!("mindlink serve on {}", host.trim()));
    let mut slave = Slave::new(port);
    eprintln!("serving {} on {name}", dir.display());
    loop {
        if let Err(e) = slave.serve(&mut service, 1000) { die(explain(e)); }
        if slave.line.failed() {
            eprintln!("{name}: the line closed");
            return;
        }
    }
}
