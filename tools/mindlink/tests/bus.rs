//! Several `serve`s on one line (800-LNK-0026): a hub repeats every byte one end sends to every other end, as an RS-485
//! pair carries it. Three slaves with addresses and one from before addresses; the master asks each by its address.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const BIN: &str = env!("CARGO_BIN_EXE_mindlink");

/// The shared line: every connection hears what every other one sends.
fn hub() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let ends: Arc<Mutex<Vec<(usize, TcpStream)>>> = Arc::default();
    std::thread::spawn(move || {
        for (id, stream) in listener.incoming().flatten().enumerate() {
            ends.lock().unwrap().push((id, stream.try_clone().unwrap()));
            let ends = ends.clone();
            std::thread::spawn(move || {
                let (mut stream, mut buf) = (stream, [0u8; 4096]);
                while let Ok(n) = stream.read(&mut buf) {
                    if n == 0 { break; }
                    for (other, w) in ends.lock().unwrap().iter_mut() { if *other != id { w.write_all(&buf[..n]).ok(); } }
                }
                ends.lock().unwrap().retain(|(other, _)| *other != id);
            });
        }
    });
    addr
}

struct Kill(Child);
impl Drop for Kill { fn drop(&mut self) { self.0.kill().ok(); self.0.wait().ok(); } }

/// `serve` of `dir` on the line, with an address or none; back once it says it serves.
fn serve(line: &str, dir: &Path, address: Option<u8>) -> Kill {
    let mut cmd = Command::new(BIN);
    if let Some(a) = address { cmd.args(["--address", &a.to_string()]); }
    let mut child = cmd.args([&format!("tcp:{line}"), "serve"]).arg(dir).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let mut said = String::new();
    let mut err = BufReader::new(child.stderr.take().unwrap());
    err.read_line(&mut said).unwrap();
    assert!(said.starts_with("serving"), "{said}");
    if let Some(a) = address { assert!(said.contains(&format!("address {a} on a shared line")), "{said}"); }
    // The rest of what it says is read, so that it never writes into a closed pipe.
    std::thread::spawn(move || { let mut sink = Vec::new(); err.read_to_end(&mut sink).ok(); });
    Kill(child)
}

fn run(line: &str, address: Option<u8>, args: &[&str]) -> (bool, String) {
    let mut cmd = Command::new(BIN);
    cmd.args(["--tries", "2", "--timeout", "800"]);
    if let Some(a) = address { cmd.args(["--address", &a.to_string()]); }
    let out = cmd.arg(format!("tcp:{line}")).args(args).output().unwrap();
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn three_addressed_slaves_and_an_old_one_on_one_line() {
    let d = std::env::temp_dir().join(format!("mindlink-bus-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    let dirs: Vec<PathBuf> = (0..4).map(|i| { let p = d.join(format!("srv{i}")); std::fs::create_dir_all(&p).unwrap(); p }).collect();
    let line = hub();
    // srv1..srv3 have addresses 1..3; srv0 is a slave from before addresses.
    let _slaves: Vec<Kill> = (0..4).map(|i| serve(&line, &dirs[i], (i > 0).then_some(i as u8))).collect();
    let file = d.join("payload.bin");
    std::fs::write(&file, (0..70_000u32).map(|i| (i * 7 % 251) as u8).collect::<Vec<u8>>()).unwrap();

    for a in 1u8..=3 {
        let (ok, out) = run(&line, Some(a), &["put", file.to_str().unwrap(), &format!("for{a}.bin")]);
        assert!(ok, "put to {a}: {out}");
        let (ok, out) = run(&line, Some(a), &["ls"]);
        assert!(ok && out.contains(&format!("for{a}.bin")), "ls of {a}: {out}");
    }
    for a in 1usize..=3 { assert_eq!(names(&dirs[a]), vec![format!("for{a}.bin")], "slave {a} holds only its own"); }
    // The old slave heard every version 2 frame and dropped it.
    assert!(names(&dirs[0]).is_empty());
    // Without --address, version 1: the old slave alone answers.
    let (ok, out) = run(&line, None, &["put", file.to_str().unwrap(), "old.bin"]);
    assert!(ok, "put without an address: {out}");
    assert_eq!(names(&dirs[0]), vec!["old.bin".to_string()]);
    for a in 1usize..=3 { assert_eq!(names(&dirs[a]).len(), 1); }
    // An address no slave has: no answer, from anyone.
    let (ok, out) = run(&line, Some(9), &["hello"]);
    assert!(!ok, "address 9 answered: {out}");
    std::fs::remove_dir_all(&d).ok();
}
