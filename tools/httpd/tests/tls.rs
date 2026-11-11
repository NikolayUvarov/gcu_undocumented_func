//! httpd over TLS 1.3, with certificates made by `openssl` and Python's `ssl` as the client: each key kind, TLS 1.2
//! refused, the SPKI's hash as the updater pins it, and a certificate reloaded (110-NET-0067).

use httpd::log::Log;
use httpd::server::{self, Config};
use httpd::tls::{self, Reloading};
use std::fs;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn openssl(dir: &Path, args: &[&str]) {
    let out = Command::new("openssl").args(args).current_dir(dir).output().expect("openssl");
    assert!(out.status.success(), "openssl {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

struct Dir(PathBuf);
impl Drop for Dir { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

// A CA, and under it a certificate for `localhost` with a key of `kind` (rsa, p256, ed25519) in `name`.{crt,key}.
fn pki() -> Dir {
    let dir = std::env::temp_dir().join(format!("httpd-tls-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
    fs::create_dir_all(dir.join("root/blobs")).unwrap();
    fs::write(dir.join("root/blobs").join(DIGEST), (0..70_000u32).map(|i| (i % 249) as u8).collect::<Vec<u8>>()).unwrap();
    fs::write(dir.join("san.cnf"), "subjectAltName=DNS:localhost\n").unwrap();
    openssl(&dir, &["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", "ca.key", "-out", "ca.pem", "-days", "2", "-subj", "/CN=httpd test CA"]);
    Dir(dir)
}

fn leaf(dir: &Path, name: &str, kind: &str) {
    match kind {
        "rsa" => openssl(dir, &["genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048", "-out", &format!("{name}.key")]),
        "p256" => openssl(dir, &["genpkey", "-algorithm", "EC", "-pkeyopt", "ec_paramgen_curve:P-256", "-out", &format!("{name}.key")]),
        _ => openssl(dir, &["genpkey", "-algorithm", "ED25519", "-out", &format!("{name}.key")]),
    }
    sign(dir, name);
}

fn sign(dir: &Path, name: &str) {
    openssl(dir, &["req", "-new", "-key", &format!("{name}.key"), "-subj", "/CN=localhost", "-out", &format!("{name}.csr")]);
    openssl(dir, &["x509", "-req", "-in", &format!("{name}.csr"), "-CA", "ca.pem", "-CAkey", "ca.key", "-CAcreateserial", "-days", "2",
                   "-extfile", "san.cnf", "-out", &format!("{name}.crt")]);
}

fn serve(dir: &Path, cert: &str, key: &str, every: Duration) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let mut config = Config::new(dir.join("root").canonicalize().unwrap());
    config.tls = Some(Reloading::start(dir.join(cert), dir.join(key), true, every).unwrap());
    fs::create_dir_all(dir.join("logs")).unwrap();
    let log = Log::new(Some(dir.join("logs")), 30);
    std::thread::spawn(move || server::run(listener, config, log));
    addr
}

// One GET through Python's ssl: the protocol, the status line, the body's length and the leaf's SHA-256, or the error.
fn fetch(dir: &Path, addr: SocketAddr, range: &str, max: &str) -> String {
    let script = r#"
import hashlib, socket, ssl, sys
port, ca, rng, top = int(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
ctx = ssl.create_default_context(cafile=ca)
if top == "1.2": ctx.maximum_version = ssl.TLSVersion.TLSv1_2
try:
    t = ctx.wrap_socket(socket.create_connection(("127.0.0.1", port), timeout=10), server_hostname="localhost")
except Exception as e:
    print("ERROR", type(e).__name__); sys.exit(0)
head = "GET /blobs/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef HTTP/1.1\r\nHost: localhost\r\n"
t.sendall((head + (f"Range: {rng}\r\n" if rng else "") + "Connection: close\r\n\r\n").encode())
data = b""
while True:
    chunk = t.recv(65536)
    if not chunk: break
    data += chunk
status, _, body = data.partition(b"\r\n\r\n")
print(t.version(), status.split(b"\r\n")[0].decode(), len(body), hashlib.sha256(t.getpeercert(binary_form=True)).hexdigest())
"#;
    let out = Command::new("python3").arg("-c").arg(script).arg(addr.port().to_string()).arg(dir.join("ca.pem")).arg(range).arg(max).output().expect("python3");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn der_sha256(dir: &Path, cert: &str) -> String {
    let out = Command::new("openssl").args(["x509", "-in", cert, "-outform", "der"]).current_dir(dir).output().unwrap();
    use sha2::Digest;
    tls::hex(&sha2::Sha256::digest(&out.stdout))
}

#[test]
fn each_key_kind_over_tls_1_3() {
    let pki = pki();
    let dir = &pki.0;
    for kind in ["rsa", "p256", "ed25519"] {
        leaf(dir, kind, kind);
        let addr = serve(dir, &format!("{kind}.crt"), &format!("{kind}.key"), Duration::from_secs(60));
        let got = fetch(dir, addr, "bytes=1000-", "");
        assert_eq!(got, format!("TLSv1.3 HTTP/1.1 206 Partial Content 69000 {}", der_sha256(dir, &format!("{kind}.crt"))), "{kind}");
    }
    // The same keys in PKCS#1 (RSA) and SEC1 (P-256) files.
    openssl(dir, &["rsa", "-in", "rsa.key", "-traditional", "-out", "rsa1.key"]);
    openssl(dir, &["ec", "-in", "p256.key", "-out", "sec1.key"]);
    assert!(fs::read_to_string(dir.join("rsa1.key")).unwrap().contains("BEGIN RSA PRIVATE KEY"));
    assert!(fs::read_to_string(dir.join("sec1.key")).unwrap().contains("BEGIN EC PRIVATE KEY"));
    for (cert, key) in [("rsa.crt", "rsa1.key"), ("p256.crt", "sec1.key")] {
        let addr = serve(dir, cert, key, Duration::from_secs(60));
        assert!(fetch(dir, addr, "", "").starts_with("TLSv1.3 HTTP/1.1 200 OK 70000 "), "{key}");
    }
}

#[test]
fn tls_1_2_and_plain_http_refused() {
    let pki = pki();
    let dir = &pki.0;
    leaf(dir, "p256", "p256");
    let addr = serve(dir, "p256.crt", "p256.key", Duration::from_secs(60));
    assert!(fetch(dir, addr, "", "1.2").starts_with("ERROR "), "TLS 1.2 is not offered");
    use std::io::{Read, Write};
    let mut plain = std::net::TcpStream::connect(addr).unwrap();
    plain.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    plain.write_all(b"GET /blobs/x HTTP/1.1\r\nHost: t\r\n\r\n").unwrap();
    let mut rest = Vec::new();
    let _ = plain.read_to_end(&mut rest);
    assert!(!String::from_utf8_lossy(&rest).contains("HTTP/1.1 200"), "no answer in clear");
    std::thread::sleep(Duration::from_millis(300));
    let day = httpd::date::day(std::time::SystemTime::now());
    let log = fs::read_to_string(dir.join(format!("logs/httpd-{day}.log"))).unwrap();
    assert_eq!(log.matches("(the TLS handshake failed: ").count(), 2, "{log}");
}

#[test]
fn the_spki_as_the_updater_pins_it() {
    let pki = pki();
    let dir = &pki.0;
    leaf(dir, "rsa", "rsa");
    // openssl's view: the certificate's public key as DER (the SubjectPublicKeyInfo), its SHA-256.
    let pubkey = Command::new("sh").arg("-c").arg("openssl x509 -in rsa.crt -pubkey -noout | openssl pkey -pubin -outform der | sha256sum").current_dir(dir).output().unwrap();
    let expected = String::from_utf8_lossy(&pubkey.stdout).split_whitespace().next().unwrap().to_string();
    let (_, spki) = tls::load(&dir.join("rsa.crt"), &dir.join("rsa.key"), true).unwrap();
    // Without --allow-rsa an RSA key is refused, saying why.
    let refused = tls::load(&dir.join("rsa.crt"), &dir.join("rsa.key"), false).unwrap_err();
    assert!(refused.contains("RUSTSEC-2023-0071"), "{refused}");
    assert_eq!(tls::hex(&spki), expected);
    // A key that is not the certificate's is refused with the pair, as a missing one is.
    leaf(dir, "other", "rsa");
    let wrong = tls::load(&dir.join("rsa.crt"), &dir.join("other.key"), true).unwrap_err();
    assert!(wrong.contains("the certificate and key"), "{wrong}");
    assert!(tls::load(&dir.join("rsa.crt"), &dir.join("missing.key"), true).is_err());
    for kind in ["p256", "ed25519"] {
        leaf(dir, kind, kind);
        assert!(tls::load(&dir.join(format!("{kind}.crt")), &dir.join("other.key"), true).is_err(), "{kind}");
        assert!(tls::load(&dir.join(format!("{kind}.crt")), &dir.join(format!("{kind}.key")), false).is_ok(), "{kind}");
    }
}

#[test]
fn a_certificate_reloaded_keeps_its_connections_and_its_key() {
    let pki = pki();
    let dir = &pki.0;
    leaf(dir, "live", "p256");
    let addr = serve(dir, "live.crt", "live.key", Duration::from_millis(200));
    let first = der_sha256(dir, "live.crt");
    assert!(fetch(dir, addr, "", "").ends_with(&first));
    // A renewal: the same key, a new certificate over the old file.
    std::thread::sleep(Duration::from_millis(1100)); // a new mtime on file systems that count whole seconds
    sign(dir, "live");
    let second = der_sha256(dir, "live.crt");
    assert_ne!(first, second);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let got = fetch(dir, addr, "", "");
        if got.ends_with(&second) { break; }
        assert!(std::time::Instant::now() < deadline, "not reloaded: {got}");
        std::thread::sleep(Duration::from_millis(200));
    }
    // A certificate that does not load leaves the last one serving.
    fs::write(dir.join("live.crt"), "not a certificate\n").unwrap();
    std::thread::sleep(Duration::from_millis(600));
    assert!(fetch(dir, addr, "", "").ends_with(&second));
}
