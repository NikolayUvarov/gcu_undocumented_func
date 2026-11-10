//! effector485 against `mindlink serve --run` over a pseudo-terminal (800-LNK-0011): a batch of put, mkdir, move, list,
//! run with output and log; an idempotency key that repeats does the work once; cancel stops a running program; the
//! queue and the results outlive the daemon; a lease holds the machine for one session (800-LNK-0021); two machines
//! share one cable, each with its own queue and lease (800-LNK-0026); the API's version, the console stream and a
//! supervised daemon (800-LNK-0039); a line no machine answers, a fake U-Boot's prompt, typed on as it is (800-LNK-0043).
mod common;

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

const MINDLINK: &str = env!("CARGO_BIN_EXE_mindlink");
const EFFECTOR: &str = env!("CARGO_BIN_EXE_effector485");

struct Kill(Child);
impl Drop for Kill { fn drop(&mut self) { self.0.kill().ok(); self.0.wait().ok(); } }

/// Starts `program` and returns it with the first line it prints.
fn start(program: &str, args: &[&str]) -> (Kill, String) {
    let mut child = Command::new(program).args(args).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    (Kill(child), line)
}

fn daemon(pty: &str, state: &Path) -> (Kill, String) {
    let (d, line) = start(EFFECTOR, &["daemon", pty, "--listen", "127.0.0.1:0", "--state", state.to_str().unwrap()]);
    let at = line.split_whitespace().nth(3).unwrap().trim_end_matches(',').to_string();
    (d, at)
}

/// The client's exit code and its output.
fn e485(at: &str, args: &[&str]) -> (i32, String) {
    let out = Command::new(EFFECTOR).args(["--at", at]).args(args).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
}

fn field<'a>(json: &'a str, name: &str) -> &'a str {
    let at = json.find(&format!("\"{name}\":\"")).unwrap_or_else(|| panic!("no {name} in {json}")) + name.len() + 4;
    &json[at..at + json[at..].find('"').unwrap()]
}

#[test]
fn a_batch_idempotency_cancel_and_a_restart() {
    let d = std::env::temp_dir().join(format!("effector485-test-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    for sub in ["srv", "cli", "state"] { std::fs::create_dir_all(d.join(sub)).unwrap(); }
    std::fs::write(d.join("srv.log"), "booted\nready\n").unwrap();
    std::fs::write(d.join("cli/hello.sh"), b"#!/bin/sh\necho ran >> runs.txt\necho hello \"$@\"\nexit 3\n").unwrap();
    std::fs::write(d.join("cli/slow.sh"), b"#!/bin/sh\nwhile true; do echo tick; sleep 0.2; done\n").unwrap();
    let (_serve, line) = start(MINDLINK, &["pty", "serve", d.join("srv").to_str().unwrap(), "--log", d.join("srv.log").to_str().unwrap(), "--run"]);
    let pty = line.split_whitespace().nth(1).unwrap().to_string();
    let (daemon1, at) = daemon(&pty, &d.join("state"));
    let file = |name: &str| format!("file={}", d.join("cli").join(name).display());
    let ok = |args: &[&str]| { let (code, out) = e485(&at, args); assert_eq!(code, 0, "{args:?}: {out}"); out };

    ok(&["submit", "put", &file("hello.sh"), "--key", "T-put", "--reason", "test: put", "--wait"]);
    assert!(d.join("srv/hello.sh").exists());
    // A put into directories that are missing makes them.
    ok(&["submit", "put", &file("hello.sh"), "name=deep/er/h.sh", "--key", "T-put-deep", "--reason", "test: put into a new directory", "--wait"]);
    assert!(d.join("srv/deep/er/h.sh").exists());
    ok(&["submit", "mkdir", "name=apps/x", "--key", "T-mkdir", "--reason", "test: mkdir", "--wait"]);
    ok(&["submit", "move", "from=hello.sh", "to=apps/x/hello.sh", "--key", "T-move", "--reason", "test: move", "--wait"]);
    assert!(ok(&["submit", "list", "name=apps/x", "--key", "T-list", "--reason", "test: list", "--wait"]).contains("hello.sh\t"));
    let (code, out) = e485(&at, &["submit", "run", "name=apps/x/hello.sh", "args=a b", "--key", "T-run", "--reason", "test: run", "--wait"]);
    assert!(code == 3 && out.contains("hello a b") && out.contains("exit code 3"), "{code}: {out}");
    // The same key again: the same operation, not a second run.
    let again = ok(&["submit", "run", "name=apps/x/hello.sh", "args=a b", "--key", "T-run", "--reason", "test: run"]);
    assert!(again.contains("\"status\":\"completed\""), "{again}");
    assert_eq!(std::fs::read_to_string(d.join("srv/runs.txt")).unwrap(), "ran\n");
    let (code, out) = e485(&at, &["submit", "mkdir", "name=apps/x", "--key", "T-run", "--reason", "test: a used key"]);
    assert!(code != 0 && out.contains("was used for a run"), "{out}");
    assert!(ok(&["submit", "log", "--key", "T-log", "--reason", "test: log", "--wait"]).contains("booted\nready\n"));

    // A program cancelled while it runs.
    ok(&["submit", "install", &file("slow.sh"), "--key", "T-install", "--reason", "test: install", "--wait"]);
    let slow = ok(&["submit", "run", "name=apps/slow.sh", "--key", "T-slow", "--reason", "test: a long run"]);
    let id = field(&slow, "id").to_string();
    for _ in 0..50 {
        if ok(&["output", &id]).contains("tick\ntick\n") { break; }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    ok(&["cancel", &id, "--key", "T-cancel", "--reason", "test: cancel"]);
    let (code, out) = e485(&at, &["wait", &id]);
    assert!(code == 1 && out.contains("tick") && out.contains(": cancelled"), "{code}: {out}");
    assert_eq!(ok(&["list", "--active"]), "");
    assert!(ok(&["endpoints"]).contains("\"online\":true"));

    // A restarted daemon keeps the results and the keys.
    drop(daemon1);
    let (_daemon2, at2) = daemon(&pty, &d.join("state"));
    let (code, out) = e485(&at2, &["submit", "run", "name=apps/x/hello.sh", "--key", "T-run", "--reason", "test: run"]);
    assert!(code == 0 && out.contains("\"status\":\"completed\"") && out.contains("hello a b"), "{out}");
    assert_eq!(std::fs::read_to_string(d.join("srv/runs.txt")).unwrap(), "ran\n");
    // 800-LNK-0015: a line for the other end's shell, followed as a program is.
    let (code, out) = e485(&at2, &["submit", "shell", "line=echo shell $((6*7))", "--key", "T-shell", "--reason", "test: shell", "--wait"]);
    assert!(code == 0 && out.contains(": completed, exit code 0") && out.contains("shell 42"), "{out}");
    let listed = e485(&at2, &["list"]).1;
    assert_eq!(listed.lines().count(), 10, "{listed}");
    // 800-LNK-0032: keys, which `mindlink serve` does not take; a name that is no key fails before the line.
    let (code, out) = e485(&at2, &["submit", "key", "names=ctrl-alt-f6 alt-0", "pace_ms=0", "--key", "T-key", "--reason", "test: key", "--wait"]);
    assert!(code == 1 && out.contains(": failed") && out.contains("does not serve this"), "{code}: {out}");
    let (code, out) = e485(&at2, &["submit", "key", "names=ctrl-nope", "--key", "T-key2", "--reason", "test: key", "--wait"]);
    assert!(code == 1 && out.contains("is not a key name"), "{code}: {out}");
    let (code, out) = e485(&at2, &["submit", "type", "--key", "T-type", "--reason", "test: type"]);
    assert!(code == 1 && out.contains("a type needs \"text\""), "{code}: {out}");
    std::fs::remove_dir_all(&d).ok();
}


/// A raw HTTP request with the headers given; the answer's head and body.
fn raw(at: &str, head: &str) -> (String, String) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(at).unwrap();
    s.write_all(format!("{head}\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
    let mut answer = String::new();
    s.read_to_string(&mut answer).unwrap();
    let (h, b) = answer.split_once("\r\n\r\n").unwrap();
    (h.to_string(), b.to_string())
}

/// A WebSocket to the page's feed, as a browser on the daemon's own page opens it; the answer's head.
fn feed(at: &str, origin: &str) -> (std::net::TcpStream, String) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(at).unwrap();
    write!(s, "GET /api/v1/stream HTTP/1.1\r\nHost: {at}\r\nOrigin: {origin}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
               Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n").unwrap();
    let mut head = Vec::new();
    let mut b = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") { if s.read(&mut b).unwrap() == 0 { break; } head.push(b[0]); }
    s.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
    (s, String::from_utf8_lossy(&head).into_owned())
}

/// A masked text frame, as a page sends one.
fn page_says(s: &mut std::net::TcpStream, text: &str) {
    use std::io::Write;
    let mask = [0x37u8, 0xfa, 0x21, 0x3d];
    let mut f = vec![0x81, 0x80 | text.len() as u8];
    f.extend_from_slice(&mask);
    f.extend(text.bytes().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    s.write_all(&f).unwrap();
}

/// The feed's text messages until one has `want`; None when it closes, or after 20 s (its beat never lets a read time out).
fn feed_until(s: &mut std::net::TcpStream, want: &str) -> Option<String> {
    use std::io::Read;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while std::time::Instant::now() < deadline {
        let mut h = [0u8; 2];
        s.read_exact(&mut h).ok()?;
        let len = match h[1] & 0x7f {
            126 => { let mut l = [0u8; 2]; s.read_exact(&mut l).ok()?; u16::from_be_bytes(l) as usize }
            127 => { let mut l = [0u8; 8]; s.read_exact(&mut l).ok()?; u64::from_be_bytes(l) as usize }
            n => n as usize,
        };
        let mut payload = vec![0u8; len];
        s.read_exact(&mut payload).ok()?;
        if h[0] & 0x0f == 8 { return None; }
        let text = String::from_utf8_lossy(&payload).into_owned();
        if text.contains(want) { return Some(text); }
    }
    None
}

#[test]
fn the_page_its_feed_and_other_sites() {
    let d = std::env::temp_dir().join(format!("effector485-page-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    for sub in ["srv", "cli", "state", "state2"] { std::fs::create_dir_all(d.join(sub)).unwrap(); }
    std::fs::write(d.join("srv.log"), "booted\n").unwrap();
    std::fs::write(d.join("cli/count.sh"), b"#!/bin/sh\ni=0; while [ $i -lt 30 ]; do echo line $i; i=$((i+1)); sleep 0.05; done\n").unwrap();
    let (_serve, line) = start(MINDLINK, &["pty", "serve", d.join("srv").to_str().unwrap(), "--log", d.join("srv.log").to_str().unwrap(), "--run"]);
    let pty = line.split_whitespace().nth(1).unwrap().to_string();
    let (_daemon, at) = daemon(&pty, &d.join("state"));
    let own = format!("http://{at}");
    let ok = |args: &[&str]| { let (code, out) = e485(&at, args); assert_eq!(code, 0, "{args:?}: {out}"); out };

    // The page, from its own address; another site's page and another host name are refused.
    let (head, body) = raw(&at, &format!("GET / HTTP/1.1\r\nHost: {at}"));
    assert!(head.starts_with("HTTP/1.1 200") && head.contains("text/html") && body.contains("<title>effector485</title>"), "{head}");
    let (head, _) = raw(&at, &format!("POST /api/v1/operations HTTP/1.1\r\nHost: {at}\r\nOrigin: http://example.com\r\nContent-Length: 2\r\n\r\n{{}}"));
    assert!(head.starts_with("HTTP/1.1 403"), "{head}");
    let port = at.rsplit(':').next().unwrap();
    let (head, _) = raw(&at, &format!("GET /api/v1/endpoints HTTP/1.1\r\nHost: rebound.example:{port}"));
    assert!(head.starts_with("HTTP/1.1 403"), "{head}");
    let (_, head) = feed(&at, "http://example.com");
    assert!(head.starts_with("HTTP/1.1 403"), "{head}");

    // The feed: the cable's state, the operations, and the output of the one followed as it comes.
    let (mut s, head) = feed(&at, &own);
    assert!(head.starts_with("HTTP/1.1 101") && head.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="), "{head}");
    assert!(feed_until(&mut s, "\"type\":\"hello\"").unwrap().contains("\"key_needed\":false"));
    assert!(feed_until(&mut s, "\"type\":\"links\"").unwrap().contains("\"online\":true"));
    ok(&["submit", "install", &format!("file={}", d.join("cli/count.sh").display()), "--key", "P-install", "--reason", "test: install", "--wait"]);
    let run = ok(&["submit", "run", "name=apps/count.sh", "--key", "P-run", "--reason", "test: a run the page follows"]);
    let id = field(&run, "id").to_string();
    page_says(&mut s, &format!("{{\"follow\":\"{id}\"}}"));
    let mut text = String::new();
    loop {
        let m = feed_until(&mut s, "\"type\":\"output\"").expect("the run's output");
        text.push_str(&m[m.find("\"text\":\"").unwrap() + 8..m.rfind('"').unwrap()].replace("\\n", "\n"));
        if m.contains("\"status\":\"completed\"") { break; }
    }
    let lines: String = (0..30).map(|i| format!("line {i}\n")).collect();
    assert_eq!(text, lines);
    // 800-LNK-0018: the cable's totals, each second, for the speed; the endpoint answers them too.
    let traffic = feed_until(&mut s, "\"type\":\"traffic\"").expect("the traffic");
    let number = |m: &str, name: &str| -> u64 {
        let at = m.find(&format!("\"{name}\":")).unwrap() + name.len() + 3;
        m[at..].split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap()
    };
    assert!(number(&traffic, "sent") > 0 && number(&traffic, "received") > 0, "{traffic}");
    assert!(ok(&["endpoints"]).contains("\"sent_bytes\":"));

    // 800-LNK-0016: a replace from a file and one back to the boot image both end; the second stayed running before.
    let done = |key: &str, args: &[&str]| {
        let mut call = vec!["submit"];
        call.extend_from_slice(args);
        call.extend_from_slice(&["--key", key, "--reason", "test: replace"]);
        let id = field(&ok(&call), "id").to_string();
        for _ in 0..100 {
            let status = ok(&["status", &id]);
            if !status.contains("\"status\":\"queued\"") && !status.contains("\"status\":\"running\"") { return status; }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        panic!("{id} did not end");
    };
    let file = format!("file={}", d.join("cli/count.sh").display());
    assert!(done("P-replace", &["replace", "unit=rtc", &file]).contains("\"status\":\"completed\""));
    assert!(done("P-back", &["replace", "unit=rtc", "back=true"]).contains("\"status\":\"completed\""));
    let log = std::fs::read_to_string(d.join("srv.log")).unwrap();
    assert!(log.contains("[SERVE] REPLACE rtc FROM replace/count.sh\n") && log.contains("[SERVE] REPLACE rtc FROM ITS BOOT IMAGE\n"), "{log}");

    // Pages that never read take no time from the cable's work, and a ninth page is refused.
    let silent: Vec<_> = (0..7).map(|_| feed(&at, &own).0).collect();
    let started = std::time::Instant::now();
    for i in 0..10 { ok(&["submit", "hello", "--key", &format!("P-hello-{i}"), "--reason", "test: work beside silent pages", "--wait"]); }
    assert!(started.elapsed() < std::time::Duration::from_secs(20), "{:?}", started.elapsed());
    let (_, head) = feed(&at, &own);
    assert!(head.starts_with("HTTP/1.1 503"), "{head}");
    drop(silent);

    // With a key, the feed asks for it and closes on a wrong one.
    std::fs::write(d.join("key"), "sesame\n").unwrap();
    let (_keyed, at2) = start(EFFECTOR, &["daemon", "/nonexistent/tty", "--listen", "127.0.0.1:0", "--state", d.join("state2").to_str().unwrap(),
                                           "--key-file", d.join("key").to_str().unwrap()]);
    let at2 = at2.split_whitespace().nth(3).unwrap().trim_end_matches(',').to_string();
    let (mut s, _) = feed(&at2, &format!("http://{at2}"));
    assert!(feed_until(&mut s, "\"type\":\"hello\"").unwrap().contains("\"key_needed\":true"));
    page_says(&mut s, "{\"key\":\"wrong\"}");
    assert!(feed_until(&mut s, "\"type\":\"error\"").is_some());
    assert!(feed_until(&mut s, "\"type\":\"links\"").is_none());
    let (mut s, _) = feed(&at2, &format!("http://{at2}"));
    feed_until(&mut s, "\"type\":\"hello\"").unwrap();
    page_says(&mut s, "{\"key\":\"sesame\"}");
    assert!(feed_until(&mut s, "\"type\":\"links\"").unwrap().contains("\"online\":false"));
    std::fs::remove_dir_all(&d).ok();
}

/// A call as a program makes one, with `headers` (each ending in CRLF) and a JSON body; the answer's head and body.
fn send(at: &str, method: &str, path: &str, headers: &str, body: &str) -> (String, String) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(at).unwrap();
    write!(s, "{method} {path} HTTP/1.1\r\nHost: {at}\r\nConnection: close\r\n{headers}Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    let mut answer = String::new();
    s.read_to_string(&mut answer).unwrap();
    let (h, b) = answer.split_once("\r\n\r\n").unwrap();
    (h.to_string(), b.to_string())
}

#[test]
fn a_reservation_holds_the_machine() {
    let d = std::env::temp_dir().join(format!("effector485-reserve-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    for sub in ["srv", "state"] { std::fs::create_dir_all(d.join(sub)).unwrap(); }
    std::fs::write(d.join("srv.log"), "booted\n").unwrap();
    let (_serve, line) = start(MINDLINK, &["pty", "serve", d.join("srv").to_str().unwrap(), "--log", d.join("srv.log").to_str().unwrap(), "--run"]);
    let pty = line.split_whitespace().nth(1).unwrap().to_string();
    let (daemon1, at) = daemon(&pty, &d.join("state"));
    let ok = |args: &[&str]| { let (code, out) = e485(&at, args); assert_eq!(code, 0, "{args:?}: {out}"); out };
    let leases = "/api/v1/terminal-reservations";

    // The lease: SESSION:PIN alone on the first line, and the PIN never shown again.
    let out = Command::new(EFFECTOR).args(["--at", &at, "reserve", "--minutes", "5", "--reason", "test: a slot update", "--owner", "EFF"]).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let lease = String::from_utf8(out.stdout).unwrap().lines().next().unwrap().to_string();
    let (session, pin) = lease.split_once(':').unwrap();
    assert!(session.starts_with("R-") && session.len() == 10 && pin.len() == 12, "{lease}");
    let (head, body) = send(&at, "GET", leases, "", "");
    assert!(head.starts_with("HTTP/1.1 200") && body.contains(session) && body.contains("\"owner\":\"EFF\"") && body.contains("\"reason\":\"test: a slot update\""), "{body}");
    assert!(!body.contains(pin) && !body.contains("\"pin\""), "{body}");
    assert!(ok(&["reservations"]).contains(&format!("{session}\tEFF\ttest: a slot update\tuntil ")));
    let (head, body) = send(&at, "POST", leases, "", r#"{"duration_minutes":5,"reason":"test: another","owner":"DRV"}"#);
    assert!(head.starts_with("HTTP/1.1 409") && body.contains("\"code\":\"terminal_reserved\"") && body.contains(session) && !body.contains(pin), "{head}\n{body}");
    for bad in [r#"{"duration_minutes":61,"reason":"test"}"#, r#"{"duration_minutes":0.5,"reason":"test"}"#, r#"{"duration_minutes":5,"reason":" "}"#] {
        assert!(send(&at, "POST", leases, "", bad).0.starts_with("HTTP/1.1 400"), "{bad}");
    }

    // Another session's change waits, saying for what; a read passes it.
    let theirs = field(&ok(&["submit", "mkdir", "name=theirs", "--key", "L-theirs", "--reason", "test: another session's mkdir"]), "id").to_string();
    assert!(ok(&["submit", "log", "--key", "L-log", "--reason", "test: a read passes", "--wait"]).contains("booted"));
    let status = ok(&["status", &theirs]);
    assert!(status.contains("\"status\":\"queued\"") && status.contains("\"waiting\":{\"code\":\"terminal_reserved\"")
            && status.contains("reserved by EFF (test: a slot update) until ") && status.contains(&format!("UTC ({session})")), "{status}");
    assert!(ok(&["endpoints"]).contains(&format!("\"reservation\":{{\"session_id\":\"{session}\"")));

    // The holder's own run, with the flag or the environment.
    ok(&["submit", "mkdir", "name=mine", "--key", "L-mine", "--reason", "test: the holder's mkdir", "--reservation", &lease, "--wait"]);
    let out = Command::new(EFFECTOR).args(["--at", &at, "submit", "mkdir", "name=mine2", "--key", "L-mine2", "--reason", "test: the holder's, by the environment", "--wait"])
        .env("EFFECTOR485_RESERVATION", &lease).output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(ok(&["status", &theirs]).contains("\"status\":\"queued\""));
    assert!(d.join("srv/mine").is_dir() && d.join("srv/mine2").is_dir() && !d.join("srv/theirs").exists());

    // A wrong PIN neither submits, renews nor releases; the right one renews, by the body or the headers.
    let wrong = format!("{session}:WRONGWRONGWR");
    let (code, out) = e485(&at, &["submit", "hello", "--key", "L-wrong", "--reason", "test: a wrong PIN", "--reservation", &wrong]);
    assert!(code != 0 && out.contains("PIN"), "{out}");
    let (head, body) = send(&at, "PATCH", leases, "", &format!(r#"{{"duration_minutes":10,"reservation_session_id":"{session}","reservation_pin":"WRONGWRONGWR"}}"#));
    assert!(head.starts_with("HTTP/1.1 409") && body.contains("PIN"), "{head}\n{body}");
    let (code, out) = e485(&at, &["release", &wrong]);
    assert!(code != 0 && out.contains("PIN"), "{out}");
    let (head, body) = send(&at, "PATCH", leases, &format!("X-Effector-Reservation-Session: {session}\r\nX-Effector-Reservation-Pin: {pin}\r\n"), r#"{"duration_minutes":10}"#);
    assert!(head.starts_with("HTTP/1.1 200") && body.contains("\"duration_minutes\":10") && body.contains("\"renewal_attempts\":1"), "{body}");
    assert!(ok(&["renew", &lease, "--minutes", "3"]).contains(&format!("{session}: held until ")));

    // The lease outlives a restart: the mkdir still waits, and the page's feed says why.
    drop(daemon1);
    let (_daemon2, at2) = daemon(&pty, &d.join("state"));
    let ok2 = |args: &[&str]| { let (code, out) = e485(&at2, args); assert_eq!(code, 0, "{args:?}: {out}"); out };
    assert!(ok2(&["status", &theirs]).contains("\"code\":\"terminal_reserved\""));
    let (mut s, _) = feed(&at2, &format!("http://{at2}"));
    assert!(feed_until(&mut s, "\"type\":\"links\"").unwrap().contains(&format!("\"reservation\":{{\"session_id\":\"{session}\"")));
    assert!(feed_until(&mut s, "\"type\":\"ops\"").unwrap().contains("\"waiting\":{\"code\":\"terminal_reserved\""));

    // Released, the waiting mkdir runs.
    assert_eq!(ok2(&["release", &lease]).trim(), format!("{session}: released"));
    for _ in 0..100 {
        if ok2(&["status", &theirs]).contains("\"status\":\"completed\"") { break; }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(d.join("srv/theirs").is_dir() && !ok2(&["status", &theirs]).contains("waiting"));
    assert!(ok2(&["endpoints"]).contains("\"reservation\":null"));
    assert_eq!(send(&at2, "GET", leases, "", "").1, "{\"reservations\":[]}");
    assert!(feed_until(&mut s, "\"reservation\":null").is_some());
    let (head, _) = send(&at2, "DELETE", leases, "", &format!(r#"{{"reservation_session_id":"{session}","reservation_pin":"{pin}"}}"#));
    assert!(head.starts_with("HTTP/1.1 409"), "{head}");
    std::fs::remove_dir_all(&d).ok();
}

// 800-LNK-0026: two machines on one cable, at addresses 1 and 2: each operation reaches its own machine alone, and a
// lease on one leaves the other free.
#[test]
fn two_machines_on_one_cable() {
    let d = std::env::temp_dir().join(format!("effector485-bus-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    for sub in ["srv1", "srv2", "cli", "state"] { std::fs::create_dir_all(d.join(sub)).unwrap(); }
    std::fs::write(d.join("cli/a.bin"), vec![7u8; 50_000]).unwrap();
    let line = common::hub();
    let _one = common::serve(&line, &d.join("srv1"), Some(1));
    let _two = common::serve(&line, &d.join("srv2"), Some(2));
    let (_daemon, first) = start(EFFECTOR, &["daemon", &format!("tcp:{line}"), "--addresses", "1,2", "--listen", "127.0.0.1:0",
                                              "--state", d.join("state").to_str().unwrap()]);
    let at = first.split_whitespace().nth(3).unwrap().trim_end_matches(',').to_string();
    let ok = |args: &[&str]| { let (code, out) = e485(&at, args); assert_eq!(code, 0, "{args:?}: {out}"); out };
    let file = format!("file={}", d.join("cli/a.bin").display());

    // Both answer their hellos.
    let mut endpoints = String::new();
    for _ in 0..100 {
        endpoints = ok(&["endpoints"]);
        if endpoints.matches("\"online\":true").count() == 2 { break; }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(endpoints.contains("\"agent_id\":\"link-1\"") && endpoints.contains("\"agent_id\":\"link-2\""), "{endpoints}");
    assert_eq!(endpoints.matches("\"online\":true").count(), 2, "{endpoints}");

    let (code, out) = e485(&at, &["submit", "hello", "--key", "B-none", "--reason", "test: no machine named"]);
    assert!(code != 0 && out.contains("name one with \"agent_id\""), "{out}");
    ok(&["--agent", "link-1", "submit", "put", &file, "name=one.bin", "--key", "B-put-1", "--reason", "test: put to 1", "--wait"]);
    ok(&["--agent", "link-2", "submit", "put", &file, "name=two.bin", "--key", "B-put-2", "--reason", "test: put to 2", "--wait"]);
    assert!(d.join("srv1/one.bin").exists() && !d.join("srv2/one.bin").exists());
    assert!(d.join("srv2/two.bin").exists() && !d.join("srv1/two.bin").exists());
    let listed = ok(&["--agent", "link-2", "submit", "list", "--key", "B-list-2", "--reason", "test: list 2", "--wait"]);
    assert!(listed.contains("two.bin") && !listed.contains("one.bin"), "{listed}");

    // A lease on link-1 holds back link-1's changes alone.
    let lease = ok(&["--agent", "link-1", "reserve", "--minutes", "5", "--owner", "TEST", "--reason", "test: link-1 held"]);
    let lease = lease.lines().next().unwrap().to_string();
    let held = ok(&["--agent", "link-1", "submit", "mkdir", "name=held", "--key", "B-held", "--reason", "test: waits"]);
    let id = field(&held, "id").to_string();
    ok(&["--agent", "link-2", "submit", "mkdir", "name=free", "--key", "B-free", "--reason", "test: passes", "--wait"]);
    assert!(d.join("srv2/free").is_dir());
    let status = ok(&["status", &id]);
    assert!(status.contains("\"status\":\"queued\"") && status.contains("link-1 is reserved by TEST"), "{status}");
    let (code, out) = e485(&at, &["--agent", "link-2", "--reservation", &lease, "submit", "mkdir", "name=x", "--key", "B-wrong", "--reason", "test: wrong machine"]);
    assert!(code != 0 && out.contains("holds link-1, not link-2"), "{out}");
    assert!(ok(&["reservations"]).contains("\tlink-1"));
    ok(&["release", &lease]);
    let (code, out) = e485(&at, &["wait", &id]);
    assert!(code == 0 && out.contains(": completed"), "{out}");
    assert!(d.join("srv1/held").is_dir() && !d.join("srv2/held").exists());
    let ones = ok(&["--agent", "link-1", "list"]);
    assert!(ones.lines().count() == 2 && ones.contains("B-") == false && ones.contains("test: put to 1") && ones.contains("test: waits"), "{ones}");
    std::fs::remove_dir_all(&d).ok();
}

/// A JSON body for `raw`, as a POST carries it.
fn post(at: &str, path: &str, body: &str) -> (String, String) {
    raw(at, &format!("POST {path} HTTP/1.1\r\nHost: {at}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()))
}

/// The console's text from `from`, waited for until it holds `want` (or 10 s).
fn console_until(at: &str, from: u64, want: &str) -> String {
    let mut body = String::new();
    for _ in 0..100 {
        body = raw(at, &format!("GET /api/v1/console?agent_id=link&from={from} HTTP/1.1\r\nHost: {at}")).1;
        if body.contains(want) { break; }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    body
}

#[test]
fn the_console_the_version_and_a_supervised_daemon() {
    let d = std::env::temp_dir().join(format!("effector485-console-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    for sub in ["srv", "state", "state2", "state3"] { std::fs::create_dir_all(d.join(sub)).unwrap(); }
    // The other end's console is `cat`, and it prints two boot lines outside frames after its first answer.
    let (_serve, line) = start(MINDLINK, &["pty", "serve", d.join("srv").to_str().unwrap(), "--console", "cat", "--banner", "U-Boot 2025.10\\nMIND Core loader\\n"]);
    let pty = line.split_whitespace().nth(1).unwrap().to_string();
    let (_daemon, at) = daemon(&pty, &d.join("state"));

    // The version, the kinds and what each version brought.
    let (head, body) = raw(&at, &format!("GET /api/v1/version HTTP/1.1\r\nHost: {at}"));
    assert!(head.starts_with("HTTP/1.1 200") && body.contains("\"api_version\":\"1.2\"") && body.contains("\"kind\":\"shell\",\"required\":[\"line\"]"), "{body}");
    assert!(body.contains("\"version\":\"1.0\"") && body.contains("/api/v1/console") && body.contains("\"version\":\"1.2\""), "{body}");

    // The boot's lines, outside frames, are the console's first text.
    let body = console_until(&at, 0, "MIND Core loader");
    assert!(body.contains("\"text\":\"U-Boot 2025.10\\r\\nMIND Core loader\\r\\n\"") && body.contains("\"gap\":0"), "{body}");
    assert!(body.contains("\"offered\":true") && body.contains("\"pieces\":[{\"offset\":0,"), "{body}");

    // An input, sent twice under one number, is written once; `cat` gives it back.
    let (head, body) = post(&at, "/api/v1/console/input", r#"{"agent_id":"link","seq":1,"text":"hello\n"}"#);
    assert!(head.starts_with("HTTP/1.1 200") && body.contains("\"written\":true"), "{head} {body}");
    let (_, body) = post(&at, "/api/v1/console/input", r#"{"agent_id":"link","seq":1,"text":"hello\n"}"#);
    assert!(body.contains("\"written\":false"), "{body}");
    let (_, body) = post(&at, "/api/v1/console/input", r#"{"agent_id":"link","seq":2,"data":"d29ybGQK"}"#);
    assert!(body.contains("\"written\":true"), "{body}");
    let body = console_until(&at, 0, "world");
    assert!(body.contains("loader\\r\\nhello\\nworld\\n\"") && body.contains("\"input_seq\":2"), "{body}");
    // From an offset: the bytes after it, and nothing past the end.
    let body = console_until(&at, 4, "world");
    assert!(body.contains("\"from\":4") && body.contains("\"text\":\"ot 2025.10") && body.contains("\"pieces\":[{\"offset\":4,"), "{body}");
    let end = body.split("\"next\":").nth(1).unwrap().split(',').next().unwrap().to_string();
    let (_, body) = raw(&at, &format!("GET /api/v1/console?agent_id=link&from={end} HTTP/1.1\r\nHost: {at}"));
    assert!(body.contains("\"text\":\"\""), "{body}");
    let (head, _) = post(&at, "/api/v1/console/input", r#"{"agent_id":"link","seq":3}"#);
    assert!(head.starts_with("HTTP/1.1 400"), "{head}");

    // Supervised: refused on a wide address, or without a key.
    std::fs::write(d.join("key"), "a-test-key\n").unwrap();
    let key = d.join("key");
    let refused = |args: &[&str]| Command::new(EFFECTOR).args(args).output().unwrap();
    let out = refused(&["daemon", "tcp:127.0.0.1:9", "--listen", "0.0.0.0:0", "--state", d.join("state2").to_str().unwrap(), "--key-file", key.to_str().unwrap(), "--supervised"]);
    assert!(!out.status.success() && String::from_utf8_lossy(&out.stderr).contains("--supervised listens only on a loopback address"), "{out:?}");
    let out = refused(&["daemon", "tcp:127.0.0.1:9", "--listen", "127.0.0.1:0", "--state", d.join("state2").to_str().unwrap(), "--supervised"]);
    assert!(!out.status.success(), "{out:?}");
    // On the loopback with a key: no page, the API behind the key, and it ends when its standard input closes.
    let mut child = Command::new(EFFECTOR).args(["daemon", "tcp:127.0.0.1:9", "--listen", "127.0.0.1:0", "--state", d.join("state3").to_str().unwrap(),
                                                 "--key-file", key.to_str().unwrap(), "--supervised"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut first = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut first).unwrap();
    let sup = first.split_whitespace().nth(3).unwrap().trim_end_matches(',').to_string();
    let (head, _) = raw(&sup, &format!("GET / HTTP/1.1\r\nHost: {sup}\r\nX-API-Key: a-test-key"));
    assert!(head.starts_with("HTTP/1.1 404"), "{head}");
    let (head, _) = raw(&sup, &format!("GET /api/v1/version HTTP/1.1\r\nHost: {sup}"));
    assert!(head.starts_with("HTTP/1.1 401"), "{head}");
    let (head, _) = raw(&sup, &format!("GET /api/v1/version HTTP/1.1\r\nHost: {sup}\r\nX-API-Key: a-test-key"));
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    drop(child.stdin.take());
    let ended = (0..50).any(|_| { std::thread::sleep(std::time::Duration::from_millis(100)); child.try_wait().unwrap().is_some() });
    if !ended { child.kill().ok(); }
    child.wait().ok();
    assert!(ended, "a supervised daemon outlived its standard input");
    std::fs::remove_dir_all(&d).ok();
}

/// A fake U-Boot on a pseudo-terminal (800-LNK-0043): it echoes what is printable, runs a line at a carriage return
/// (`version` answers), clears it at Ctrl-C, and prints its prompt `=> `; no `link` answers there.
#[cfg(target_os = "linux")]
mod uboot {
    use std::ffi::{c_char, c_int, c_uint, CStr};
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::OpenOptionsExt;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    #[repr(C)]
    struct Termios { iflag: c_uint, oflag: c_uint, cflag: c_uint, lflag: c_uint, line: u8, cc: [u8; 32], ispeed: c_uint, ospeed: c_uint }

    extern "C" {
        fn posix_openpt(flags: c_int) -> c_int;
        fn grantpt(fd: c_int) -> c_int;
        fn unlockpt(fd: c_int) -> c_int;
        fn ptsname_r(fd: c_int, buf: *mut c_char, len: usize) -> c_int;
        fn tcgetattr(fd: c_int, t: *mut Termios) -> c_int;
        fn tcsetattr(fd: c_int, when: c_int, t: *const Termios) -> c_int;
        fn cfmakeraw(t: *mut Termios);
    }

    const O_RDWR_NOCTTY: c_int = 2 | 0o400;

    /// The board's line for the daemon, held open and raw so it never hangs up; each byte the board got, with its time;
    /// its side of the line, for what the test has it print.
    pub struct Board { pub path: String, _held: File, pub got: Arc<Mutex<Vec<(u8, Instant)>>>, out: Mutex<File> }

    impl Board {
        pub fn say(&self, text: &str) { self.out.lock().unwrap().write_all(text.as_bytes()).unwrap(); }
    }

    pub fn board() -> Board {
        let fd = unsafe { posix_openpt(O_RDWR_NOCTTY) };
        assert!(fd >= 0 && unsafe { grantpt(fd) } == 0 && unsafe { unlockpt(fd) } == 0, "no pseudo-terminal");
        let mut master = unsafe { File::from_raw_fd(fd) };
        let mut name = [0 as c_char; 128];
        assert_eq!(unsafe { ptsname_r(fd, name.as_mut_ptr(), name.len()) }, 0);
        let path = unsafe { CStr::from_ptr(name.as_ptr()) }.to_string_lossy().into_owned();
        let held = OpenOptions::new().read(true).write(true).custom_flags(0o400).open(&path).unwrap();
        let mut t: Termios = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { tcgetattr(held.as_raw_fd(), &mut t) }, 0);
        unsafe { cfmakeraw(&mut t) };
        assert_eq!(unsafe { tcsetattr(held.as_raw_fd(), 0, &t) }, 0);
        let got = Arc::new(Mutex::new(Vec::new()));
        let seen = got.clone();
        let mut out = master.try_clone().unwrap();
        let say = Mutex::new(master.try_clone().unwrap());
        std::thread::spawn(move || {
            let (mut line, mut buf) = (String::new(), [0u8; 256]);
            while let Ok(n) = master.read(&mut buf) {
                if n == 0 { break; }
                for &b in &buf[..n] {
                    seen.lock().unwrap().push((b, Instant::now()));
                    let say = match b {
                        b'\r' | b'\n' => {
                            let answer = match line.as_str() {
                                "" => String::new(),
                                "version" => "U-Boot 2025.10-fake, built for the test\r\n".to_string(),
                                other => format!("Unknown command '{other}' - try 'help'\r\n"),
                            };
                            line.clear();
                            format!("\r\n{answer}=> ")
                        }
                        3 => { line.clear(); "<INTERRUPT>\r\n=> ".to_string() }
                        0x20..=0x7e => { line.push(b as char); (b as char).to_string() }
                        _ => String::new(),
                    };
                    if out.write_all(say.as_bytes()).is_err() { return; }
                }
            }
        });
        Board { path, _held: held, got, out: say }
    }
}

// 800-LNK-0043: a line on which no machine answers takes its input as it is, paced and written once, from the lease's
// holder alone, and a break; while `mindlink serve` answers, a raw write and a break are refused.
#[cfg(target_os = "linux")]
#[test]
fn a_line_without_link_as_a_console() {
    let d = std::env::temp_dir().join(format!("effector485-uboot-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    for sub in ["srv", "state", "state2", "state3"] { std::fs::create_dir_all(d.join(sub)).unwrap(); }
    let board = uboot::board();
    let (_daemon, at) = daemon(&board.path, &d.join("state"));
    let input = "/api/v1/console/input";

    // Nobody answers the link: the line is in console mode.
    assert!(raw(&at, &format!("GET /api/v1/endpoints HTTP/1.1\r\nHost: {at}")).1.contains("\"online\":false,\"mode\":\"console\""));
    assert!(raw(&at, &format!("GET /api/v1/console?agent_id=link HTTP/1.1\r\nHost: {at}")).1.contains("\"mode\":\"console\""));

    // Ctrl-C clears what the daemon's hellos left at the prompt; `version`, sent twice under one number, runs once.
    let (head, body) = post(&at, input, r#"{"agent_id":"link","seq":1,"data":"Aw=="}"#);
    assert!(head.starts_with("HTTP/1.1 200") && body.contains("\"written\":true") && body.contains("\"mode\":\"console\""), "{head} {body}");
    assert!(console_until(&at, 0, "<INTERRUPT>").contains("<INTERRUPT>"));
    assert!(post(&at, input, r#"{"seq":2,"text":"version\r"}"#).1.contains("\"written\":true"));
    assert!(post(&at, input, r#"{"seq":2,"text":"version\r"}"#).1.contains("\"written\":false"));
    assert!(post(&at, input, r#"{"seq":3,"text":"x","pace_ms":1001}"#).0.starts_with("HTTP/1.1 400"));
    let body = console_until(&at, 0, "built for the test\\r\\n=> ");
    assert!(body.contains("version\\r\\nU-Boot 2025.10-fake, built for the test\\r\\n=> "), "{body}");
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert_eq!(console_until(&at, 0, "built for the test").matches("built for the test").count(), 1);
    // The board got the line after Ctrl-C, a character every 20 ms, and nothing of the daemon's since.
    let got = board.got.lock().unwrap().clone();
    let after = got.iter().rposition(|&(b, _)| b == 3).unwrap() + 1;
    let typed: Vec<u8> = got[after..].iter().map(|&(b, _)| b).collect();
    assert_eq!(typed, b"version\r", "{:?}", String::from_utf8_lossy(&typed));
    let span = got[got.len() - 1].1 - got[after].1;
    assert!(span >= std::time::Duration::from_millis(7 * 15), "{span:?}");

    // A lease: the line takes its holder's input and break alone.
    let lease = e485(&at, &["reserve", "--minutes", "5", "--owner", "TEST", "--reason", "test: U-Boot's prompt"]).1;
    let (session, pin) = lease.lines().next().unwrap().split_once(':').unwrap();
    let held = format!("X-Effector-Reservation-Session: {session}\r\nX-Effector-Reservation-Pin: {pin}\r\n");
    let (head, body) = send(&at, "POST", input, "", r#"{"seq":4,"text":"\r"}"#);
    assert!(head.starts_with("HTTP/1.1 409") && body.contains("\"code\":\"reservation_not_held\"") && body.contains("takes the holder's input only"), "{head} {body}");
    let (head, body) = send(&at, "POST", input, &held, r#"{"seq":4,"text":"\r"}"#);
    assert!(head.starts_with("HTTP/1.1 200") && body.contains("\"written\":true"), "{head} {body}");
    let brk = "/api/v1/console/break";
    assert!(send(&at, "POST", brk, "", r#"{"ms":100}"#).0.starts_with("HTTP/1.1 409"));
    assert!(send(&at, "POST", brk, &held, r#"{"ms":5}"#).0.starts_with("HTTP/1.1 400"));
    let (head, body) = send(&at, "POST", brk, &held, r#"{"ms":100}"#);
    assert!(head.starts_with("HTTP/1.1 200") && body.contains("\"sent\":true") && body.contains("\"ms\":100"), "{head} {body}");
    assert!(e485(&at, &["release", lease.lines().next().unwrap()]).0 == 0);

    // `mindlink serve` answers: the line is the link's, and a break is refused.
    let (_serve, line) = start(MINDLINK, &["pty", "serve", d.join("srv").to_str().unwrap()]);
    let pty = line.split_whitespace().nth(1).unwrap().to_string();
    let (_daemon2, at2) = daemon(&pty, &d.join("state2"));
    let mut endpoints = String::new();
    for _ in 0..100 {
        endpoints = e485(&at2, &["endpoints"]).1;
        if endpoints.contains("\"online\":true") { break; }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(endpoints.contains("\"online\":true,\"mode\":\"link\""), "{endpoints}");
    let (head, body) = post(&at2, brk, "{}");
    assert!(head.starts_with("HTTP/1.1 409") && body.contains("\"code\":\"link_mode\"") && body.contains("the link's frames share this line"), "{head} {body}");

    // Two machines on a cable, one answering: `line` takes no raw input. With none answering, it does, and a TCP line
    // has no break.
    let hub = common::hub();
    let _one = common::serve(&hub, &d.join("srv"), Some(1));
    let (_daemon3, first) = start(EFFECTOR, &["daemon", &format!("tcp:{hub}"), "--addresses", "1,2", "--listen", "127.0.0.1:0", "--state", d.join("state3").to_str().unwrap()]);
    let at3 = first.split_whitespace().nth(3).unwrap().trim_end_matches(',').to_string();
    for _ in 0..100 {
        if e485(&at3, &["endpoints"]).1.contains("\"online\":true") { break; }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let (head, body) = post(&at3, input, r#"{"agent_id":"line","seq":1,"text":"boot\r"}"#);
    assert!(head.starts_with("HTTP/1.1 409") && body.contains("link-1 answers the link"), "{head} {body}");
    assert!(post(&at3, brk, r#"{"agent_id":"line"}"#).1.contains("\"code\":\"link_mode\""));
    assert!(raw(&at3, &format!("GET /api/v1/console?agent_id=line HTTP/1.1\r\nHost: {at3}")).1.contains("\"mode\":\"link\""));
    let empty = common::hub();
    std::fs::create_dir_all(d.join("state4")).unwrap();
    let (_daemon4, first) = start(EFFECTOR, &["daemon", &format!("tcp:{empty}"), "--addresses", "1,2", "--listen", "127.0.0.1:0", "--state", d.join("state4").to_str().unwrap()]);
    let at4 = first.split_whitespace().nth(3).unwrap().trim_end_matches(',').to_string();
    let (head, body) = post(&at4, input, r#"{"agent_id":"line","seq":1,"text":"boot\r"}"#);
    assert!(head.starts_with("HTTP/1.1 200") && body.contains("\"written\":true") && body.contains("\"mode\":\"console\""), "{head} {body}");
    let (head, body) = post(&at4, brk, r#"{"agent_id":"line"}"#);
    assert!(head.starts_with("HTTP/1.1 409") && body.contains("\"code\":\"unsupported\""), "{head} {body}");
    assert!(post(&at4, brk, r#"{"agent_id":"link-1"}"#).0.starts_with("HTTP/1.1 400"));
    std::fs::remove_dir_all(&d).ok();
}

// 800-LNK-0043: while the line prints, as U-Boot's countdown does each second, no hello reaches it; once it has been
// quiet for 3 s, one does within a round.
#[cfg(target_os = "linux")]
#[test]
fn no_hello_while_the_line_prints() {
    use std::time::{Duration, Instant};
    let d = std::env::temp_dir().join(format!("effector485-quiet-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(d.join("state")).unwrap();
    let board = uboot::board();
    let (_daemon, at) = daemon(&board.path, &d.join("state"));
    let got = || board.got.lock().unwrap().len();
    let wait = |until: Duration, done: &dyn Fn() -> bool| { let start = Instant::now(); while !done() && start.elapsed() < until { std::thread::sleep(Duration::from_millis(20)); } };

    // The first hello, on a quiet line, then the countdown: a line every 500 ms for 5 s, and no frame meanwhile.
    wait(Duration::from_secs(10), &|| got() > 0);
    std::thread::sleep(Duration::from_millis(200));
    let before = got();
    assert!(before > 0, "no hello on a quiet line");
    let mut last = Instant::now();
    for left in (1..=10).rev() {
        board.say(&format!("\rHit any key to stop autoboot: {left:2} "));
        last = Instant::now();
        std::thread::sleep(Duration::from_millis(500));
    }
    assert_eq!(got(), before, "a frame reached the board while it printed");
    let endpoints = raw(&at, &format!("GET /api/v1/endpoints HTTP/1.1\r\nHost: {at}")).1;
    assert!(endpoints.contains("\"hello_held\":\"printing\""), "{endpoints}");
    assert!(console_until(&at, 0, "autoboot:  1").contains("autoboot:  1"));

    // Quiet: no hello before 3 s, and one within 3 s and a round of about 3.6 s.
    wait(Duration::from_millis(2500).saturating_sub(last.elapsed()), &|| got() > before);
    assert_eq!(got(), before, "a hello {:?} after the last line", last.elapsed());
    wait(Duration::from_millis(3000 + 3600 + 1000), &|| got() > before);
    assert!(got() > before, "no hello {:?} after the line went quiet", last.elapsed());
    std::fs::remove_dir_all(&d).ok();
}
