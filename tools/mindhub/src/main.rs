//! `mindhub`: MIND Hub's separate server and its key tools (docs/hub/README.md, 100003-APP-0146).

use mindhub::{client, conformance, server};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process::exit;
use std::sync::{Arc, Mutex};

const USAGE: &str = "usage:
  mindhub serve DATA [--listen 127.0.0.1:8443] [--cert CHAIN.pem --key KEY.pem]
      DATA holds hub.conf, journal.jsonl and attachments/; without a certificate it listens on loopback alone
  mindhub key FILE                 make an Ed25519 key in FILE (0600) and print its public half for hub.conf
  mindhub public FILE              print a key file's public half
  mindhub admin-code --key FILE --hub URL [--pin SPKI-SHA256]...
                                   a one-time code for the admin pages, signed with an admin key
  mindhub conformance --hub URL [--pin SPKI-SHA256]... --admin-key F --intake-key F --track-key F --track TRK --integrator-key F
                                   the conformance suite against a hub, from a network that registered no key there today";

fn fail(why: &str) -> ! { eprintln!("mindhub: {why}"); exit(1) }

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).map(|i| args.get(i + 1).cloned().unwrap_or_else(|| fail(&format!("{name} takes a value"))));
    match args.first().map(String::as_str) {
        Some("serve") => serve(args.get(1).cloned().unwrap_or_else(|| fail(USAGE)).into(), flag("--listen"), flag("--cert"), flag("--key")),
        Some("key") => {
            let path = PathBuf::from(args.get(1).unwrap_or_else(|| fail(USAGE)));
            let key = client::write_key(&path).unwrap_or_else(|e| fail(&e));
            println!("{}", improve_proto::hex(key.verifying_key().as_bytes()));
        }
        Some("public") => {
            let key = client::read_key(&PathBuf::from(args.get(1).unwrap_or_else(|| fail(USAGE)))).unwrap_or_else(|e| fail(&e));
            println!("{}", improve_proto::hex(key.verifying_key().as_bytes()));
        }
        Some("admin-code") => {
            let key = client::read_key(&PathBuf::from(flag("--key").unwrap_or_else(|| fail(USAGE)))).unwrap_or_else(|e| fail(&e));
            let pins: Vec<[u8; 32]> = args.windows(2).filter(|w| w[0] == "--pin").map(|w| improve_proto::parse_hex32(&w[1]).unwrap_or_else(|| fail("--pin takes 64 hex"))).collect();
            let mut c = client::Client::new(&flag("--hub").unwrap_or_else(|| fail(USAGE)), &pins, key).unwrap_or_else(|e| fail(&e));
            let reply = c.call("POST", "/hub/admin/v1/sign-in-code", b"").unwrap_or_else(|e| fail(&e.to_string()));
            match reply.json().get("code").and_then(|v| v.as_str()) {
                Some(code) if reply.status == 201 => println!("{code}  (for the admin pages, once, within 10 minutes)"),
                _ => fail(&format!("the hub answered {}: {}", reply.status, reply.text())),
            }
        }
        Some("conformance") => {
            let key = |name: &str| client::read_key(&PathBuf::from(flag(name).unwrap_or_else(|| fail(USAGE)))).unwrap_or_else(|e| fail(&e));
            let target = conformance::Target {
                url: flag("--hub").unwrap_or_else(|| fail(USAGE)),
                pins: args.windows(2).filter(|w| w[0] == "--pin").map(|w| improve_proto::parse_hex32(&w[1]).unwrap_or_else(|| fail("--pin takes 64 hex"))).collect(),
                admin: key("--admin-key"), intake: key("--intake-key"), track: key("--track-key"),
                track_code: flag("--track").unwrap_or_else(|| fail(USAGE)), integrator: key("--integrator-key"),
            };
            match conformance::run(&target, &mut |line| println!("{line}")) {
                Ok(()) => println!("CONFORMANCE: PASSED"),
                Err(why) => { println!("not ok - {why}"); fail("CONFORMANCE: FAILED") }
            }
        }
        _ => fail(USAGE),
    }
}

fn serve(data: PathBuf, listen: Option<String>, cert: Option<String>, key: Option<String>) {
    let listen = listen.unwrap_or_else(|| "127.0.0.1:8443".into());
    let addr: SocketAddr = listen.parse().unwrap_or_else(|_| fail(&format!("--listen {listen}: an address and a port")));
    let tls = match (cert, key) {
        (Some(cert), Some(key)) => {
            let (_, spki) = httpd::tls::load(cert.as_ref(), key.as_ref(), false).unwrap_or_else(|e| fail(&e));
            eprintln!("MINDHUB: TLS 1.3, THE KEY'S SPKI SHA-256 {}: DEVICES PIN IT", httpd::tls::hex(&spki));
            Some(httpd::tls::Reloading::start(cert.into(), key.into(), false, std::time::Duration::from_secs(10)).unwrap_or_else(|e| fail(&e)))
        }
        (None, None) if addr.ip().is_loopback() => None,
        (None, None) => fail("a hub off loopback speaks TLS alone: --cert and --key"),
        _ => fail("--cert and --key go together"),
    };
    let shared = server::open(&data).unwrap_or_else(|e| fail(&e));
    let listener = TcpListener::bind(addr).unwrap_or_else(|e| fail(&format!("{listen}: {e}")));
    eprintln!("MINDHUB: SERVING {} ON {}, {} EVENTS, {} USERS, {} REQUESTS", improve_proto::API, listener.local_addr().map(|a| a.to_string()).unwrap_or(listen),
              shared.hub.events, shared.hub.users().len(), shared.hub.requests().len());
    let shared = Arc::new(Mutex::new(shared));
    let options = server::Options { tls, ..server::Options::new() };
    if let Err(e) = server::serve(listener, shared, options) { fail(&e.to_string()); }
}
