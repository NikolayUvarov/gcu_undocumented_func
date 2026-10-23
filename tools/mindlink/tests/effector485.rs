//! effector485 against `mindlink serve --run` over a pseudo-terminal (800-LNK-0011): a batch of put, mkdir, move, list,
//! run with output and log; an idempotency key that repeats does the work once; cancel stops a running program; the
//! queue and the results outlive the daemon; a lease holds the machine for one session (800-LNK-0021).
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
    assert!(feed_until(&mut s, "\"type\":\"link\"").unwrap().contains("\"online\":true"));
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
    assert!(feed_until(&mut s, "\"type\":\"link\"").is_none());
    let (mut s, _) = feed(&at2, &format!("http://{at2}"));
    feed_until(&mut s, "\"type\":\"hello\"").unwrap();
    page_says(&mut s, "{\"key\":\"sesame\"}");
    assert!(feed_until(&mut s, "\"type\":\"link\"").unwrap().contains("\"online\":false"));
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
    assert!(feed_until(&mut s, "\"type\":\"link\"").unwrap().contains(&format!("\"reservation\":{{\"session_id\":\"{session}\"")));
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
