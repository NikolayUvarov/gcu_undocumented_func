//! mindlink against its own `serve`, over a pseudo-terminal (the serial path, termios) and over TCP (800-LNK-0004).
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_mindlink");

struct Server { child: Child, port: String }
impl Drop for Server { fn drop(&mut self) { self.child.kill().ok(); self.child.wait().ok(); } }

/// `serve` on `spec`, and the port its first line names.
fn serve(spec: &str, dir: &Path, log: &Path) -> Server {
    let mut child = Command::new(BIN).args([spec, "serve"]).arg(dir).arg("--log").arg(log).args(["--console", "sh"])
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
    let (ok, out) = run(p, &["put", big.to_str().unwrap(), "../escape"], b"");
    assert!(!ok && out.contains("not allowed"), "{out}");
    drop(s);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn over_a_pseudo_terminal() { session("pty", "pty"); }

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
