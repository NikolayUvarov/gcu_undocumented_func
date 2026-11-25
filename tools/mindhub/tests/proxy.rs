//! The hub reached through an HTTP proxy's CONNECT and known by a CA bundle (100003-APP-0160), as an agent in a cloud
//! session reaches it: the chain checked against the bundle and named for the URL's host, every call still signed over
//! the hub's challenge; the conformance suite through the proxy; what is refused.

use ed25519_dalek::SigningKey;
use mindhub::client::{self, Client, Trust};
use mindhub::conformance::{self, Target};
use mindhub::server::{self, Options};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn key(n: u8) -> SigningKey { SigningKey::from_bytes(&[n; 32]) }
fn public(n: u8) -> String { improve_proto::hex(key(n).verifying_key().as_bytes()) }

fn dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mindhub-proxy-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("hub.conf"), format!("admin maintainer {}\nagent APP intake {}\nagent APP track {}\nagent KRN integrator {}\n",
                                                 public(1), public(2), public(3), public(4))).unwrap();
    dir
}

fn openssl(dir: &Path, args: &[&str]) {
    let out = Command::new("openssl").args(args).current_dir(dir).output().expect("openssl");
    assert!(out.status.success(), "openssl {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

// A CA of RSA, as the cloud's proxy has, and the hub's certificate for localhost from it with a key of `kind`.
fn issued(dir: &Path, ca: &str, kind: &str) -> (PathBuf, PathBuf) {
    if !dir.join(format!("{ca}.pem")).exists() {
        openssl(dir, &["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", &format!("{ca}.key"), "-out", &format!("{ca}.pem"), "-days", "2",
                       "-subj", &format!("/CN=test {ca}"), "-addext", "basicConstraints=critical,CA:TRUE", "-addext", "keyUsage=critical,keyCertSign"]);
    }
    let key = format!("{kind}.key");
    match kind {
        "rsa" => openssl(dir, &["genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048", "-out", &key]),
        _ => openssl(dir, &["genpkey", "-algorithm", "EC", "-pkeyopt", "ec_paramgen_curve:P-256", "-out", &key]),
    }
    std::fs::write(dir.join("names.cnf"), "subjectAltName=DNS:localhost\nbasicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\n").unwrap();
    openssl(dir, &["req", "-new", "-key", &key, "-out", &format!("{kind}.csr"), "-subj", "/CN=localhost"]);
    openssl(dir, &["x509", "-req", "-in", &format!("{kind}.csr"), "-CA", &format!("{ca}.pem"), "-CAkey", &format!("{ca}.key"), "-CAcreateserial",
                   "-out", &format!("{kind}.crt"), "-days", "2", "-extfile", "names.cnf"]);
    (dir.join(format!("{kind}.crt")), dir.join(key))
}

// The hub over TLS with that certificate (an RSA key taken, as the tests' certificates may be); its port.
fn hub(dir: &Path, cert: PathBuf, key: PathBuf) -> u16 {
    let tls = httpd::tls::Reloading::start(cert, key, true, std::time::Duration::from_secs(3600)).unwrap();
    let shared = server::open(dir).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || server::serve(listener, Arc::new(Mutex::new(shared)), Options { tls: Some(tls), ..Options::new() }));
    port
}

// A CONNECT proxy on loopback, as the cloud's is to the agent (without its interception): the targets asked, and an
// answer of `refuse` in place of the tunnel when one is given.
fn proxy(refuse: Option<&'static str>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = asked.clone();
    std::thread::spawn(move || {
        for client in listener.incoming() {
            let Ok(mut client) = client else { continue };
            let seen = seen.clone();
            std::thread::spawn(move || {
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") { if client.read(&mut byte).unwrap_or(0) == 0 { return; } head.push(byte[0]); }
                let line = String::from_utf8_lossy(&head).lines().next().unwrap_or("").to_string();
                let target = line.strip_prefix("CONNECT ").and_then(|r| r.split(' ').next()).unwrap_or("").to_string();
                seen.lock().unwrap().push(target.clone());
                if let Some(answer) = refuse { let _ = client.write_all(answer.as_bytes()); return; }
                let Ok(upstream) = TcpStream::connect(&target) else { let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n"); return; };
                client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").unwrap();
                let (mut a, mut b) = (client.try_clone().unwrap(), upstream.try_clone().unwrap());
                let (mut c, mut d) = (client, upstream);
                std::thread::spawn(move || { let _ = std::io::copy(&mut a, &mut b); let _ = b.shutdown(Shutdown::Write); });
                let _ = std::io::copy(&mut d, &mut c);
                let _ = c.shutdown(Shutdown::Write);
            });
        }
    });
    (addr, asked)
}

#[test]
fn a_hub_known_by_its_ca_is_reached_through_a_connect_proxy() {
    for kind in ["p256", "rsa"] {
        let dir = dir();
        let (cert, key_path) = issued(&dir, "ca", kind);
        let port = hub(&dir, cert, key_path);
        let roots = client::roots(&dir.join("ca.pem")).unwrap();
        let (through, asked) = proxy(None);
        let url = format!("https://localhost:{port}");
        // An unsigned call, then one the admin signs over the challenge the hub gave through the tunnel.
        let mut admin = Client::connect(&url, Trust::Roots(roots.clone()), Some(through.clone()), key(1)).unwrap();
        assert_eq!(admin.plain("GET", "/hub/v1/challenge", &[], b"").unwrap().status, 200, "{kind}");
        let users = admin.call("GET", "/hub/admin/v1/users", b"").unwrap();
        assert_eq!(users.status, 200, "{kind}: {}", users.text());
        // A key that is no agent's is refused by the hub (403), whoever carries the call.
        let mut stranger = Client::connect(&url, Trust::Roots(roots.clone()), Some(through.clone()), key(9)).unwrap();
        assert_eq!(stranger.call("GET", "/hub/agent/v1/queue", b"").unwrap().status, 403, "{kind}");
        assert!(asked.lock().unwrap().iter().all(|t| *t == format!("localhost:{port}")), "{:?}", asked.lock().unwrap());
        if kind == "p256" {
            // The conformance suite through the proxy, the hub known by the bundle.
            let target = Target { url: url.clone(), trust: Trust::Roots(roots), proxy: Some(through), admin: key(1), intake: key(2), track: key(3),
                                  track_code: "APP".into(), integrator: key(4) };
            let mut lines = Vec::new();
            let result = conformance::run(&target, &mut |l| lines.push(l.to_string()));
            assert!(result.is_ok(), "{}\nnot ok - {}", lines.join("\n"), result.unwrap_err());
            assert!(lines.len() >= 15, "{lines:?}");
        }
    }
}

#[test]
fn what_is_refused_on_the_way() {
    let dir = dir();
    let (cert, key_path) = issued(&dir, "ca", "p256");
    let port = hub(&dir, cert, key_path);
    issued(&dir, "other", "p256-other");
    let (through, _) = proxy(None);
    let url = format!("https://localhost:{port}");
    // A chain to another CA, and a name the certificate does not carry.
    let other = client::roots(&dir.join("other.pem")).unwrap();
    let err = Client::connect(&url, Trust::Roots(other), Some(through.clone()), key(1)).unwrap().plain("GET", "/hub/v1/challenge", &[], b"").unwrap_err();
    assert!(err.to_string().to_lowercase().contains("unknownissuer") || err.to_string().contains("issuer"), "{err}");
    let roots = client::roots(&dir.join("ca.pem")).unwrap();
    let err = Client::connect(&format!("https://127.0.0.1:{port}"), Trust::Roots(roots.clone()), Some(through), key(1)).unwrap()
        .plain("GET", "/hub/v1/challenge", &[], b"").unwrap_err();
    assert!(err.to_string().contains("NotValidForName") || err.to_string().to_lowercase().contains("name"), "{err}");
    // A proxy that refuses the tunnel is named with its answer.
    let (refusing, _) = proxy(Some("HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n"));
    let err = Client::connect(&url, Trust::Roots(roots.clone()), Some(refusing.clone()), key(1)).unwrap().plain("GET", "/hub/v1/challenge", &[], b"").unwrap_err();
    assert!(err.to_string().contains(&format!("the proxy {refusing} refused CONNECT localhost:{port}: HTTP/1.1 403 Forbidden")), "{err}");
    // No pin and no bundle; a plain hub through a proxy; a bundle with no CA.
    assert!(Client::connect(&url, Trust::Pins(vec![]), None, key(1)).is_err());
    assert!(Client::connect("http://hub.example:80", Trust::Pins(vec![]), Some(refusing), key(1)).is_err());
    std::fs::write(dir.join("empty.pem"), "not a certificate\n").unwrap();
    assert!(client::roots(&dir.join("empty.pem")).is_err());
}

#[test]
fn the_proxy_is_taken_from_https_proxy_unless_no_proxy_names_the_host() {
    let no = Some("localhost,127.0.0.1,::1,127.0.0.0/8,169.254.0.0/16,.internal,example.org");
    let via = Some("http://proxy.local:3128");
    assert_eq!(client::proxy_for("hub.example.com", via, no), Ok(Some("proxy.local:3128".into())));
    assert_eq!(client::proxy_for("hub.example.com", None, no), Ok(None), "no proxy set");
    for host in ["localhost", "127.0.0.1", "127.9.9.9", "::1", "169.254.1.2", "db.internal", "example.org", "hub.example.org"] {
        assert_eq!(client::proxy_for(host, via, no), Ok(None), "{host}");
    }
    assert_eq!(client::proxy_for("10.0.0.1", via, Some("*")), Ok(None));
    assert_eq!(client::proxy_for("127.0.0.1", via, None), Ok(None), "loopback never goes through a proxy");
    assert_eq!(client::proxy_for("notexample.org", via, no), Ok(Some("proxy.local:3128".into())), "a suffix of another name is not the domain");
    assert_eq!(client::proxy("proxy.local:3128/"), Ok("proxy.local:3128".into()));
    assert!(client::proxy("https://proxy.local:3128").is_err());
    assert!(client::proxy("http://user:secret@proxy.local:3128").is_err());
    assert!(client::proxy("http://proxy.local").is_err());
    // A URL with no port asks the proxy for the scheme's.
    let (through, asked) = proxy(Some("HTTP/1.1 403 Forbidden\r\n\r\n"));
    let roots = { let dir = dir(); issued(&dir, "ca", "p256"); client::roots(&dir.join("ca.pem")).unwrap() };
    let _ = Client::connect("https://hub.example", Trust::Roots(roots), Some(through), key(1)).unwrap().plain("GET", "/hub/v1/challenge", &[], b"");
    assert_eq!(asked.lock().unwrap().as_slice(), ["hub.example:443"]);
}
