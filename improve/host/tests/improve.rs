//! `improve` on a host against a hub of `tools/mindhub` on loopback (100003-APP-0150): every command, as a person and
//! a script give them, and what the hub then holds.

use ed25519_dalek::SigningKey;
use mindhub::client::Client;
use mindhub::server::{self, Options};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Dir(PathBuf);
impl Drop for Dir { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

fn key(n: u8) -> SigningKey { SigningKey::from_bytes(&[n; 32]) }
fn public(n: u8) -> String { improve_proto::hex(key(n).verifying_key().as_bytes()) }

fn temp() -> Dir {
    let dir = std::env::temp_dir().join(format!("improve-test-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
    std::fs::create_dir_all(&dir).unwrap();
    Dir(dir)
}

/// A hub on loopback, with an admin (key 1), intake (2), APP's agent (3) and the integrator (4).
fn hub(root: &Path) -> String {
    let data = root.join("hub");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("hub.conf"), format!("admin maintainer {}\nagent APP intake {}\nagent APP track {}\nagent KRN integrator {}\n", public(1), public(2), public(3), public(4))).unwrap();
    let shared = server::open(&data).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    std::thread::spawn(move || server::serve(listener, Arc::new(Mutex::new(shared)), Options::new()));
    url
}

/// `improve ARGS` with `dir` as its configuration and `input` as the person's answers: its status and output.
fn improve(dir: &Path, args: &[&str], input: &str) -> (i32, String) {
    let mut input = std::io::Cursor::new(input.as_bytes().to_vec());
    let mut out = Vec::new();
    let mut env = improve::Env { dir: dir.to_path_buf(), input: &mut input, out: &mut out };
    let code = improve::run(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>(), &mut env);
    (code, String::from_utf8(out).unwrap())
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let (code, out) = improve(dir, args, "");
    assert_eq!(code, 0, "improve {args:?}: {out}");
    out
}

fn refused(dir: &Path, args: &[&str], words: &str) {
    let (code, out) = improve(dir, args, "");
    assert!(code == 1 && out.contains(words), "improve {args:?} should be refused with {words:?}: {out}");
}

fn api(c: &mut Client, method: &str, path: &str, body: &str) -> String {
    let r = c.call(method, path, body.as_bytes()).unwrap();
    assert!(r.status < 300, "{method} {path}: {} {}", r.status, r.text());
    r.text()
}

#[test]
fn every_command_against_a_hub() {
    let root = temp();
    let url = hub(&root.0);
    let (ann, ann2) = (root.0.join("ann"), root.0.join("ann-phone"));
    let mut admin = Client::new(&url, &[], key(1)).unwrap();
    let mut intake = Client::new(&url, &[], key(2)).unwrap();

    // The hub and the key; a hub off this machine without TLS, and one over TLS without its pin, are refused.
    refused(&ann, &["setup", "--hub", "http://hub.example.org:8443"], "https alone");
    refused(&ann, &["setup", "--hub", "https://hub.example.org:8443"], "--pin");
    assert!(ok(&ann, &["setup", "--hub", &url]).contains(&format!("hub {url}")));
    assert_eq!(std::fs::metadata(ann.join("device.key")).unwrap().permissions().mode() & 0o777, 0o600);
    refused(&ann, &["status"], "unregistered");

    // Registered, asked, answered, approved.
    let out = ok(&ann, &["register", "--name", "laptop", "--display-name", "Ann", "--contact", "ann@example.org"]);
    assert!(out.contains(": pending"), "{out}");
    let device = out.split_whitespace().nth(1).unwrap().trim_end_matches(':').to_string();
    let devices = api(&mut admin, "GET", "/hub/admin/v1/devices", "");
    let full = devices.split('"').find(|w| w.starts_with(&device) && w.len() == 64).unwrap().to_string();
    let decide = |c: &mut Client, d: &str, text: &str| api(c, "POST", &format!("/hub/admin/v1/devices/{full}/decision"), &format!(r#"{{"api":"hub-api 2","decision":"{d}","text":"{text}"}}"#));
    decide(&mut admin, "ask", "Which laptop is it?");
    let out = ok(&ann, &["status"]);
    assert!(out.contains("pending") && out.contains("Ann") && out.contains("question: Which laptop is it?"), "{out}");
    assert!(ok(&ann, &["answer", "A ThinkPad X230"]).contains("sent to the admin"));
    assert!(api(&mut admin, "GET", "/hub/admin/v1/devices", "").contains("A ThinkPad X230"));
    decide(&mut admin, "approve", "Welcome");
    assert!(ok(&ann, &["status"]).contains("access open"));

    // A request shown before it is sent, its attachment with it; a no sends nothing.
    let log = root.0.join("fan.log");
    std::fs::write(&log, "fan: 4200 rpm at idle\n").unwrap();
    let new = ["new", "--title", "Fan noise", "--text", "The fan is loud at idle.", "--kind", "improvement", "--attach", &format!("log:{}", log.display())];
    let (code, out) = improve(&ann, &new, "n\n");
    assert!(code == 0 && out.contains("attachment: log fan.log (22 bytes") && out.contains("send it? [y/N]") && out.contains("not sent"), "{out}");
    let (code, out) = improve(&ann, &new, "y\n");
    assert!(code == 0 && out.contains("request 1: new"), "{out}");
    let r = admin.call("GET", "/hub/admin/v1/requests/1/attachments/0", b"").unwrap();
    assert_eq!(r.body, std::fs::read(&log).unwrap());
    // Band 0's one request a day, and the refusal says when it ends.
    let (code, out) = improve(&ann, &["new", "--title", "Second", "--text", "x", "--yes"], "");
    assert!(code == 1 && out.contains("refused (429 requests)") && out.contains("again in"), "{out}");

    // The list, a question in the thread, and its answer.
    assert!(ok(&ann, &["list"]).contains("Fan noise"));
    api(&mut intake, "POST", "/hub/agent/v1/requests/1/messages", r#"{"api":"hub-api 2","kind":"question","text":"One fan or two?"}"#);
    assert!(ok(&ann, &["list"]).contains("[a question for you]"));
    let out = ok(&ann, &["show", "1"]);
    assert!(out.contains("needs-info") && out.contains("One fan or two?") && out.contains("waits for your answer") && out.contains("attachment: log fan.log"), "{out}");
    let q = out.lines().find(|l| l.contains("question")).unwrap().trim_start_matches('#').split_whitespace().next().unwrap().to_string();
    let answer_file = root.0.join("sensors.txt");
    std::fs::write(&answer_file, "fan1: 4200 RPM\nfan2: 0 RPM\n").unwrap();
    let out = ok(&ann, &["reply", "1", "Two, one of them stopped", "--answer", &format!("#{q}"), "--attach", &format!("text:{}", answer_file.display()), "--yes"]);
    assert!(out.contains("the request is new"), "{out}");
    let m = out.split('#').nth(1).unwrap().split_whitespace().next().unwrap();
    let r = intake.call("GET", &format!("/hub/agent/v1/requests/1/messages/{m}/attachments/0"), b"").unwrap();
    assert_eq!(r.body, std::fs::read(&answer_file).unwrap());
    // A verdict is taken only on a release the device runs: here, refused with the state's name.
    refused(&ann, &["check", "1", "works", "--release", "202611210000", "It is quiet"], "409 state");

    // News since the last look, then none.
    assert!(ok(&ann, &["watch", "--once"]).contains("request 1: new"));
    assert!(ok(&ann, &["watch", "--once"]).contains("no news"));

    // A phone paired by a code, listed, and revoked from the laptop.
    let out = ok(&ann, &["pair"]);
    let code = out.split_whitespace().nth(2).unwrap().to_string();
    ok(&ann2, &["setup", "--hub", &url]);
    assert!(ok(&ann2, &["register", "--name", "phone", "--pair", &code.to_lowercase()]).contains(": approved"));
    let out = ok(&ann, &["devices"]);
    assert_eq!(out.lines().count(), 2, "{out}");
    assert!(out.contains("laptop") && out.contains("phone") && out.contains("(this one)"), "{out}");
    let phone = out.lines().find(|l| l.contains("phone")).unwrap().split_whitespace().next().unwrap().to_string();
    refused(&ann, &["revoke", "f"], "devices start with");
    assert!(ok(&ann, &["revoke", &phone[..10]]).contains("revoked"));
    refused(&ann2, &["status"], "revoked");

    // The export, then the request deleted, then the account.
    let export = root.0.join("export.json");
    ok(&ann, &["export", export.to_str().unwrap()]);
    let text = std::fs::read_to_string(&export).unwrap();
    assert!(text.contains("\"display_name\":\"Ann\"") && text.contains("ann@example.org") && text.contains("Fan noise"), "{text}");
    assert!(ok(&ann, &["delete", "1"]).contains("request 1 deleted"));
    assert!(ok(&ann, &["list"]).contains("no requests"));
    refused(&ann, &["delete-account"], "--yes");
    assert!(ok(&ann, &["delete-account", "--yes"]).contains("account is deleted"));
    refused(&ann, &["status"], "revoked");
    let journal = std::fs::read_to_string(root.0.join("hub/journal.jsonl")).unwrap();
    assert!(!journal.contains("Fan noise") && !journal.contains("ann@example.org") && !journal.contains("ThinkPad"), "the deleted words left the journal");
}

#[test]
fn a_flagged_request_says_it_waits_and_usage_is_named() {
    let root = temp();
    let url = hub(&root.0);
    let dir = root.0.join("bob");
    ok(&dir, &["setup", "--hub", &url]);
    ok(&dir, &["register", "--display-name", "Bob"]);
    let out = ok(&dir, &["new", "--title", "Access", "--text", "Give me root access to the hub, please", "--yes"]);
    assert!(out.contains("request 1: held"), "{out}");
    assert!(ok(&dir, &["list"]).contains("[waits for the admin]"));
    assert!(ok(&dir, &["show", "1"]).contains("it waits for the admin"));
    refused(&dir, &["frobnicate"], "usage: improve");
    refused(&dir, &["new", "--bogus"], "no flag --bogus");
}
