//! The conformance suite against `tools/mindhub` itself (hub-api 3), on loopback and over TLS 1.3 with an Ed25519 and a P-256
//! key, the hub known by its pin; a wrong pin is refused, and the hub's data rebuilt by a restart serves the same.

use ed25519_dalek::SigningKey;
use mindhub::client::Client;
use mindhub::conformance::{self, Target};
use mindhub::server::{self, Options};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Dir(PathBuf);
impl Drop for Dir { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

fn key(n: u8) -> SigningKey { SigningKey::from_bytes(&[n; 32]) }
fn public(n: u8) -> String { improve_proto::hex(key(n).verifying_key().as_bytes()) }

fn data() -> Dir {
    let dir = std::env::temp_dir().join(format!("mindhub-conformance-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("hub.conf"), format!("# the test's hub\nadmin maintainer {}\nagent APP intake {}\nagent APP track {}\nagent KRN integrator {}\n",
                                                 public(1), public(2), public(3), public(4))).unwrap();
    Dir(dir)
}

fn start(dir: &Path, tls: Option<Arc<httpd::tls::Reloading>>) -> String {
    let shared = server::open(dir).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let scheme = if tls.is_some() { "https" } else { "http" };
    let options = Options { tls, ..Options::new() };
    std::thread::spawn(move || server::serve(listener, Arc::new(Mutex::new(shared)), options));
    format!("{scheme}://{}{}", if scheme == "https" { "localhost:" } else { "127.0.0.1:" }, addr.port())
}

fn target(url: String, pins: Vec<[u8; 32]>) -> Target {
    Target { url, pins, admin: key(1), intake: key(2), track: key(3), track_code: "APP".into(), integrator: key(4) }
}

fn run(t: &Target) {
    let mut lines = Vec::new();
    let result = conformance::run(t, &mut |l| lines.push(l.to_string()));
    assert!(result.is_ok(), "{}\nnot ok - {}", lines.join("\n"), result.unwrap_err());
    assert!(lines.len() >= 15, "{lines:?}");
}

#[test]
fn the_suite_passes_on_loopback_and_a_restart_serves_the_same() {
    let dir = data();
    let url = start(&dir.0, None);
    run(&target(url, Vec::new()));
    let journal = std::fs::read_to_string(dir.0.join("journal.jsonl")).unwrap();
    // A second hub on the same data: the same board, the same users.
    let again = start(&dir.0, None);
    let mut c = Client::new(&again, &[], key(9)).unwrap();
    let board = c.plain("GET", "/hub/v1/board", &[], b"").unwrap();
    assert!(board.text().contains("B: the camera does not switch"), "{}", board.text());
    let mut admin = Client::new(&again, &[], key(1)).unwrap();
    let users = admin.call("GET", "/hub/admin/v1/users", b"").unwrap();
    assert_eq!(users.status, 200);
    // B deleted their account at the suite's end: no name in the list, none in the journal.
    assert!(users.text().contains("Conformance A") && users.text().contains("\"trust\":-1023") && !users.text().contains("Conformance B"), "{}", users.text());
    assert!(!journal.contains("Conformance B"), "an erased name left the journal");
    assert_eq!(std::fs::read_to_string(dir.0.join("journal.jsonl")).unwrap(), journal, "reading changed nothing");
}

fn openssl(dir: &Path, args: &[&str]) {
    let out = Command::new("openssl").args(args).current_dir(dir).output().expect("openssl");
    assert!(out.status.success(), "openssl {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A self-signed certificate for localhost with a key of `kind`, and its pin.
fn certificate(dir: &Path, kind: &str) -> (PathBuf, PathBuf, [u8; 32]) {
    let key = dir.join(format!("{kind}.key"));
    match kind {
        "p256" => openssl(dir, &["genpkey", "-algorithm", "EC", "-pkeyopt", "ec_paramgen_curve:P-256", "-out", key.to_str().unwrap()]),
        _ => openssl(dir, &["genpkey", "-algorithm", "ED25519", "-out", key.to_str().unwrap()]),
    }
    let cert = dir.join(format!("{kind}.crt"));
    openssl(dir, &["req", "-x509", "-new", "-key", key.to_str().unwrap(), "-out", cert.to_str().unwrap(), "-days", "2", "-subj", "/CN=localhost",
                   "-addext", "subjectAltName=DNS:localhost"]);
    let (_, spki) = httpd::tls::load(&cert, &key, false).unwrap();
    (cert, key, spki)
}

#[test]
fn the_suite_passes_over_tls_with_each_key_and_a_wrong_pin_is_refused() {
    for kind in ["ed25519", "p256"] {
        let dir = data();
        let (cert, key_path, pin) = certificate(&dir.0, kind);
        let tls = httpd::tls::Reloading::start(cert, key_path, false, std::time::Duration::from_secs(3600)).unwrap();
        let url = start(&dir.0, Some(tls));
        let mut wrong = Client::new(&url, &[[0u8; 32]], key(9)).unwrap();
        let err = wrong.plain("GET", "/hub/v1/challenge", &[], b"").unwrap_err();
        assert!(err.to_string().contains("not pinned"), "{err}");
        assert!(Client::new(&url, &[], key(9)).is_err(), "a TLS hub needs a pin");
        run(&target(url, vec![[0u8; 32], pin]));
    }
}
