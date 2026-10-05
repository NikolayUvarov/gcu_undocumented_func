//! console's own commands (issue u006), done with what console holds: the task table from system information (`ps`),
//! the user's files (`ls`, `cat`, `mkdir`, `rm`, `mv`, `write`), the clock (`date`, `time`), and the network grant its
//! launcher may have got for it from the policy broker (`ping`). What the shell alone can do is not here.
use crate::screen::{Kind, Screen};
use alloc::format;
use alloc::string::String;
use core::fmt::Write;
use mind::abi::*;
use mind::fs::{self, File};
use mind::idl::{socket, sysinfo};
use mind::ipc::Endpoint;

/// Runs builtin `name` with `args`; its lines go to `screen`.
pub fn run(screen: &mut Screen, name: &str, args: &str) {
    let mut out = String::new();
    let ok = match name {
        "ps" => ps(&mut out),
        "ls" => ls(&mut out, args),
        "cat" => cat(&mut out, args),
        "date" => date(&mut out),
        "time" => time(&mut out),
        "ping" => ping(&mut out, args),
        "mkdir" | "rm" | "mv" | "write" => change(&mut out, name, args),
        _ => { let _ = write!(out, "{}: not a command of console", name); false }
    };
    screen.say(out.trim_end_matches('\n'), if ok { Kind::Output } else { Kind::Error });
}

fn state(wait: u8) -> &'static str {
    match wait { WAIT_RUNNING => "RUN", WAIT_NONE => "READY", WAIT_SEND => "SEND", WAIT_RECEIVE => "RECV", WAIT_REPLY => "CALL", WAIT_SLEEP => "SLEEP", WAIT_IRQ => "IRQ", WAIT_FLUSH => "FLUSH", WAIT_EXITED => "EXITED", _ => "?" }
}

// The tasks as sysmon reports them (console asks for system information to pass it on, and uses it here).
fn ps(out: &mut String) -> bool {
    let tasks = match sysinfo::tasks(Endpoint::SYSINFO) {
        Ok(Ok(tasks)) => tasks,
        Ok(Err(error)) => { let _ = write!(out, "ps: sysmon: {:?}", error); return false; }
        Err(_) => { let _ = write!(out, "ps: no system information (console was started without it)"); return false; }
    };
    let _ = writeln!(out, "{:>4} {:<16} {:<6} {:>3} {:>9} {:>8}", "PID", "NAME", "STATE", "CPU", "TIME", "MEMORY");
    let mut list: alloc::vec::Vec<&sysinfo::Task> = tasks.as_slice().iter().collect();
    list.sort_by_key(|t| t.pid);
    for t in &list {
        let seconds = t.run_ns / 1_000_000_000;
        let memory = (t.image + t.stack + t.screen + t.heap + t.retained) / 1024;
        let _ = writeln!(out, "{:>4} {:<16} {:<6} {:>3} {:>3}:{:02}.{:02} {:>7}K{}", t.pid, t.name.as_str(), state(t.wait), t.cpu, seconds / 60, seconds % 60,
                         t.run_ns / 10_000_000 % 100, memory, if t.flags & 1 != 0 { "  service" } else { "" });
    }
    let _ = write!(out, "{} tasks", list.len());
    true
}

fn ls(out: &mut String, path: &str) -> bool {
    let (mut files, mut bytes) = (0usize, 0u64);
    let result = fs::list(path.trim(), |e| {
        if e.is_dir { let _ = writeln!(out, "{:<32} {:>10}", e.name_str(), "<DIR>"); } else { let _ = writeln!(out, "{:<32} {:>10}", e.name_str(), e.size); files += 1; bytes += e.size as u64; }
    });
    match result {
        Ok(count) => { let _ = write!(out, "{} entries, {} files, {} bytes", count, files, bytes); true }
        Err(error) => { let _ = write!(out, "ls: {}: {:?}", if path.is_empty() { "A:/" } else { path }, error); false }
    }
}

// The text of a file, the first 16 KiB.
fn cat(out: &mut String, path: &str) -> bool {
    let path = path.trim();
    if path.is_empty() { let _ = write!(out, "cat <file>"); return false; }
    let mut file = match File::open(path) { Ok(f) => f, Err(e) => { let _ = write!(out, "cat: {}: {:?}", path, e); return false; } };
    let mut buffer = [0u8; 4096];
    let mut shown = alloc::vec::Vec::new();
    while shown.len() < 16 * 1024 {
        match file.read(&mut buffer) { Ok(0) => break, Ok(n) => shown.extend_from_slice(&buffer[..n]), Err(e) => { let _ = write!(out, "cat: {}: {:?}", path, e); return false; } }
    }
    out.push_str(&String::from_utf8_lossy(&shown));
    if file.size() > shown.len() { let _ = write!(out, "\n... {} more bytes", file.size() - shown.len()); }
    true
}

// mkdir, rm, mv and write, where the user may write (ram: and data/ on the boot disk).
fn change(out: &mut String, name: &str, args: &str) -> bool {
    let mut words = args.split_whitespace();
    let (first, second) = (words.next().unwrap_or(""), words.next());
    let result = match (name, second) {
        (_, _) if first.is_empty() => { let _ = write!(out, "{} <path>{}", name, match name { "mv" => " <new path>", "write" => " <text>", _ => "" }); return false; }
        ("mkdir", _) => fs::mkdir(first),
        ("rm", _) => fs::remove(first),
        ("mv", Some(to)) => fs::rename(first, to),
        ("mv", None) => { let _ = write!(out, "mv <path> <new path>"); return false; }
        _ => {
            let text = args[first.len()..].trim();
            File::create(first).and_then(|mut f| { f.write(text.as_bytes())?; f.write(b"\n")?; f.flush() })
        }
    };
    match result {
        Ok(()) => { let _ = write!(out, "{}: done", name); true }
        Err(error) => { let _ = write!(out, "{}: {}: {:?}", name, first, error); false }
    }
}

fn date(out: &mut String) -> bool {
    match (mind::rtc::date(), mind::rtc::seconds_since_midnight()) {
        (Some((y, m, d)), Some(s)) => { let _ = write!(out, "{}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, s / 3600, s / 60 % 60, s % 60); true }
        _ => { let _ = write!(out, "date: the clock does not answer"); false }
    }
}

// The time of day and how long the system has run (the clock program is `clock`).
fn time(out: &mut String) -> bool {
    let up = mind::time::uptime_ms() / 1000;
    match mind::rtc::seconds_since_midnight() {
        Some(s) => { let _ = write!(out, "{:02}:{:02}:{:02}, up {}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60, up / 3600, up / 60 % 60, up % 60); true }
        None => { let _ = write!(out, "time: the clock does not answer; up {}:{:02}:{:02}", up / 3600, up / 60 % 60, up % 60); false }
    }
}

fn ipv4(text: &str) -> Option<u32> {
    let mut address = [0u8; 4]; let mut parts = text.split('.');
    for byte in address.iter_mut() { *byte = parts.next()?.parse().ok()?; }
    parts.next().is_none().then_some(u32::from_be_bytes(address))
}

fn dotted(address: u32) -> String { let b = address.to_be_bytes(); format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]) }

const NO_NETWORK: &str = "ping: no network here. A program gets the network only as netpolicy.txt names it for it (lines such as \
`console 1.1.1.1 icmp` and `console dns`), when the shell starts it; wm passes none. The shell's own ping needs no line: type it in the shell.";

// Three echo requests through console's flow grant (REQUEST_NETWORK, issue 102): only to what the policy names for it.
fn ping(out: &mut String, args: &str) -> bool {
    let Some(host) = args.split_whitespace().next() else { let _ = write!(out, "ping <host>"); return false };
    let stack = Endpoint(SLOT_NETWORK);
    if mind::dev::cap_info(SLOT_NETWORK).0 != CAP_KIND_ENDPOINT { out.push_str(NO_NETWORK); return false; }
    let target = match ipv4(host) {
        Some(address) => address,
        None => match socket::resolve(stack, host, 0, 0, 3000) {
            Ok(Ok(address)) => address,
            Ok(Err(socket::Error::Denied)) => { let _ = write!(out, "ping: {}: looking up names is not allowed for console (netpolicy.txt: `console dns`)", host); return false; }
            Ok(Err(error)) => { let _ = write!(out, "ping: {}: {:?}", host, error); return false; }
            Err(_) => { out.push_str(NO_NETWORK); return false; }
        },
    };
    let mut received = 0;
    for _ in 0..3 {
        match socket::ping(stack, target, 1000) {
            Ok(Ok(us)) => { received += 1; let _ = writeln!(out, "reply from {}: time={} us", dotted(target), us); }
            Ok(Err(socket::Error::Denied)) => { let _ = write!(out, "ping: {}: not allowed for console (netpolicy.txt: `console {} icmp`)", dotted(target), dotted(target)); return false; }
            Ok(Err(error)) => { let _ = writeln!(out, "ping {}: {:?}", dotted(target), error); }
            Err(_) => { out.push_str(NO_NETWORK); return false; }
        }
    }
    let _ = write!(out, "ping: 3 sent, {} received", received);
    received > 0
}
