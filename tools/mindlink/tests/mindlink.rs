//! mindlink against its own `serve`, over a pseudo-terminal (the serial path, termios) and over TCP (800-LNK-0004).
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_mindlink");

struct Server { child: Child, port: String }
impl Drop for Server { fn drop(&mut self) { self.child.kill().ok(); self.child.wait().ok(); } }

/// `serve` on `spec`, and the port its first line names.
fn serve(spec: &str, dir: &Path, log: &Path) -> Server { serve_with(spec, dir, log, &[]) }

fn serve_with(spec: &str, dir: &Path, log: &Path, extra: &[&str]) -> Server {
    let mut child = Command::new(BIN).args([spec, "serve"]).arg(dir).arg("--log").arg(log).args(["--console", "sh"]).args(extra)
        .stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let port = match line.split_whitespace().collect::<Vec<_>>()[..] {
        ["pty", path] => path.to_string(),
        ["listening", "on", addr] => format!("tcp:{addr}"),
        _ => panic!("serve said {line:?}"),
    };
    Server { child, port }
}

fn run(port: &str, args: &[&str], input: &[u8]) -> (bool, String) {
    let mut child = Command::new(BIN).arg(port).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mindlink-test-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(d.join("srv")).unwrap();
    std::fs::create_dir_all(d.join("cli")).unwrap();
    d
}

fn session(spec: &str, tag: &str) {
    let d = scratch(tag);
    let data: Vec<u8> = (0..1_500_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8 & if i % 211 < 9 { 0 } else { 0xFF }).collect();
    std::fs::write(d.join("cli/big.bin"), &data).unwrap();
    std::fs::write(d.join("srv.log"), "booted\nready\n").unwrap();
    let s = serve(spec, &d.join("srv"), &d.join("srv.log"));
    let p = s.port.as_str();
    let big = d.join("cli/big.bin");
    let back = d.join("cli/back.bin");

    let (ok, out) = run(p, &["hello"], b"");
    assert!(ok && out.contains("mindlink serve on"), "{out}");
    let (ok, out) = run(p, &["put", big.to_str().unwrap()], b"");
    assert!(ok && out.contains("put ") && out.contains("1500000 bytes"), "{out}");
    assert_eq!(std::fs::read(d.join("srv/big.bin")).unwrap(), data);
    let (ok, out) = run(p, &["put", big.to_str().unwrap()], b"");
    assert!(ok && out.contains("not sent"), "{out}");
    let (ok, out) = run(p, &["put", big.to_str().unwrap(), "copy.bin", "--force"], b"");
    assert!(ok && out.contains("as copy.bin"), "{out}");
    let (ok, out) = run(p, &["ls"], b"");
    assert!(ok && out.contains("1500000  big.bin") && out.contains("copy.bin") && !out.contains(".part"), "{out}");
    let (ok, out) = run(p, &["get", "big.bin", back.to_str().unwrap()], b"");
    assert!(ok && out.contains("got big.bin"), "{out}");
    assert_eq!(std::fs::read(&back).unwrap(), data);
    let (ok, out) = run(p, &["stat", "big.bin"], b"");
    assert!(ok && out.starts_with("big.bin\t1500000\t"), "{out}");
    let (ok, out) = run(p, &["log"], b"");
    assert!(ok && out == "booted\nready\n", "{out}");
    let (ok, out) = run(p, &["console"], b"echo sum $((40+2))\n");
    assert!(ok && out.contains("sum 42"), "{out}");
    let (ok, out) = run(p, &["rm", "copy.bin"], b"");
    assert!(ok && !d.join("srv/copy.bin").exists(), "{out}");
    let (ok, out) = run(p, &["get", "copy.bin"], b"");
    assert!(!ok && out.contains("no such file"), "{out}");
    // `delete`, as effector485 names it, is `rm`.
    let small = d.join("small.txt");
    std::fs::write(&small, b"to be deleted").unwrap();
    let (ok, out) = run(p, &["put", small.to_str().unwrap(), "small.txt"], b"");
    assert!(ok && d.join("srv/small.txt").exists(), "{out}");
    let (ok, out) = run(p, &["delete", "small.txt"], b"");
    assert!(ok && !d.join("srv/small.txt").exists(), "{out}");
    let (ok, out) = run(p, &["put", big.to_str().unwrap(), "../escape"], b"");
    assert!(!ok && out.contains("not allowed"), "{out}");
    drop(s);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn over_a_pseudo_terminal() { session("pty", "pty"); }

/// 800-LNK-0010 and 0009 between two mindlinks: a tree made, filled, moved and removed, `ls` showing each step; programs
/// started with `serve --run`, their output and exit code, a stop; none started without `--run`.
#[test]
fn directories_and_programs() {
    let d = scratch("dirs");
    std::fs::write(d.join("cli/f.txt"), b"in a tree\n").unwrap();
    std::fs::write(d.join("cli/hello.sh"), b"#!/bin/sh\necho hello \"$@\"\necho to stderr >&2\nexit 3\n").unwrap();
    std::fs::write(d.join("cli/slow.sh"), b"#!/bin/sh\nwhile true; do echo tick; sleep 0.2; done\n").unwrap();
    std::fs::write(d.join("srv.log"), "").unwrap();
    let s = serve_with("pty", &d.join("srv"), &d.join("srv.log"), &["--run"]);
    let p = s.port.as_str();
    let ok = |args: &[&str]| { let (ok, out) = run(p, args, b""); assert!(ok, "{args:?}: {out}"); out };
    let fails = |args: &[&str], why: &str| { let (ok, out) = run(p, args, b""); assert!(!ok && out.contains(why), "{args:?}: {out}"); };
    let f = d.join("cli/f.txt");
    fails(&["put", f.to_str().unwrap(), "a/b/f.txt"], "no such file or directory");
    ok(&["mkdir", "a/b"]);
    assert!(ok(&["ls"]).contains("-  a/"));
    ok(&["put", f.to_str().unwrap(), "a/b/f.txt"]);
    assert!(ok(&["ls", "a/b"]).contains("10  f.txt"));
    fails(&["mv", "a/b/f.txt", "c/f.txt"], "no such file");
    ok(&["mkdir", "c"]);
    ok(&["mv", "a/b/f.txt", "c/f.txt"]);
    assert!(!ok(&["ls", "a/b"]).contains("f.txt"));
    assert_eq!(std::fs::read(d.join("srv/c/f.txt")).unwrap(), b"in a tree\n");
    fails(&["rmdir", "a"], "not empty");
    ok(&["rmdir", "a", "-r"]);
    let root = ok(&["ls"]);
    assert!(root.contains("c/") && !root.contains("a/"), "{root}");
    for bad in ["../x", "a/../../x", "/x"] { fails(&["mkdir", bad], "not allowed"); }

    ok(&["mkdir", "apps"]);
    ok(&["put", d.join("cli/hello.sh").to_str().unwrap(), "apps/hello.sh"]);
    ok(&["put", d.join("cli/slow.sh").to_str().unwrap(), "apps/slow.sh"]);
    let (done, out) = run(p, &["run", "apps/hello.sh", "--payload", "x"], b"");
    assert!(!done && out.contains("hello --payload x") && out.contains("to stderr") && out.contains("exit code 3"), "{out}");
    let started = ok(&["start", "apps/slow.sh"]);
    let id = started.split('\t').next().unwrap().to_string();
    std::thread::sleep(std::time::Duration::from_millis(700));
    assert!(ok(&["runs"]).contains(&format!("{id}\t")), "the run is listed");
    assert!(ok(&["out", &id]).contains("tick\ntick\n"));
    ok(&["stop", &id]);
    assert!(ok(&["runs"]).contains("stopped\tapps/slow.sh"));
    fails(&["out", "12345"], "no such file");
    // 800-LNK-0015: a line for the other end's shell, its words joined; its exit code is mindlink's.
    assert!(ok(&["sh", "echo", "sum", "$((40+2))", "&&", "ls"]).contains("sum 42\napps\nc\n"));
    let (done, out) = run(p, &["sh", "echo refused >&2; exit 3"], b"");
    assert!(!done && out.contains("refused"), "{out}");
    assert!(ok(&["runs"]).contains("ended\tsh: echo"), "a line's run is listed by its command's word");
    drop(s);

    let s = serve("pty", &d.join("srv"), &d.join("srv.log"));
    let (done, out) = run(&s.port, &["run", "apps/hello.sh"], b"");
    assert!(!done && out.contains("not allowed"), "{out}");
    let (done, out) = run(&s.port, &["sh", "echo", "no"], b"");
    assert!(!done && out.contains("not allowed"), "{out}");
    drop(s);
    std::fs::remove_dir_all(&d).ok();
}

/// TCP serves one connection, so this one runs its commands against a fresh server each time it is needed.
#[test]
fn over_tcp() {
    let d = scratch("tcp");
    std::fs::write(d.join("cli/f.txt"), b"over tcp\n").unwrap();
    std::fs::write(d.join("srv.log"), "").unwrap();
    let s = serve("listen:0", &d.join("srv"), &d.join("srv.log"));
    let (ok, out) = run(&s.port, &["put", d.join("cli/f.txt").to_str().unwrap()], b"");
    assert!(ok && out.contains("9 bytes"), "{out}");
    assert_eq!(std::fs::read(d.join("srv/f.txt")).unwrap(), b"over tcp\n");
    drop(s);
    std::fs::remove_dir_all(&d).ok();
}

/// No one answers at the other end (its server stopped): the tool says so and fails, in bounded time.
#[test]
fn nobody_answers() {
    let d = scratch("dead");
    let s = serve("pty", &d.join("srv"), &d.join("none.log"));
    let pid = s.child.id().to_string();
    Command::new("kill").args(["-STOP", &pid]).status().unwrap();
    let start = std::time::Instant::now();
    let (ok, out) = run(&s.port, &["--timeout", "100", "--tries", "3", "hello"], b"");
    Command::new("kill").args(["-CONT", &pid]).status().unwrap();
    assert!(!ok && out.contains("no answer"), "{out}");
    assert!(start.elapsed().as_secs() < 5);
    drop(s);
    std::fs::remove_dir_all(&d).ok();
}
