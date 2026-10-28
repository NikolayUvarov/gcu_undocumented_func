//! A shared line for the tests of several machines on one cable (800-LNK-0026): a hub repeats every byte one end sends
//! to every other end, as an RS-485 pair carries it, and `mindlink serve` ends on it.
#![allow(dead_code)]
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

pub const MINDLINK: &str = env!("CARGO_BIN_EXE_mindlink");

/// The shared line's address: every connection hears what every other one sends.
pub fn hub() -> String {
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

pub struct Kill(pub Child);
impl Drop for Kill { fn drop(&mut self) { self.0.kill().ok(); self.0.wait().ok(); } }

/// `serve` of `dir` on the line, with an address or none; back once it says it serves.
pub fn serve(line: &str, dir: &Path, address: Option<u8>) -> Kill {
    let mut cmd = Command::new(MINDLINK);
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
