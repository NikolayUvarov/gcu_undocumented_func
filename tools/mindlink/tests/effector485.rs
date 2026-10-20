//! effector485 against `mindlink serve --run` over a pseudo-terminal (800-LNK-0011): a batch of put, mkdir, move, list,
//! run with output and log; an idempotency key that repeats does the work once; cancel stops a running program; the
//! queue and the results outlive the daemon.
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
    let listed = e485(&at2, &["list"]).1;
    assert_eq!(listed.lines().count(), 8, "{listed}");
    std::fs::remove_dir_all(&d).ok();
}

