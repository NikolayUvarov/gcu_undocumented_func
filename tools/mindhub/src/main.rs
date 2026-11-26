//! `mindhub`: MIND Hub's separate server and its key tools (docs/hub/README.md, 100003-APP-0146).

use mindhub::{client, conformance, server};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process::exit;
use std::sync::{Arc, Mutex};

const USAGE: &str = "usage:
  mindhub serve DATA [--listen 127.0.0.1:8443] [--cert CHAIN.pem --key KEY.pem]
                [--releases DIR [--public CHANNEL]... [--updates-per-minute 600]]
      DATA holds hub.conf, journal.jsonl and attachments/; without a certificate it listens on loopback alone;
      DIR is a release tree as scripts/release.py publish writes it, served under /updates/, its public channels
      alone (stable when none is named)
  mindhub key FILE                 make an Ed25519 key in FILE (0600) and print its public half for hub.conf
  mindhub public FILE              print a key file's public half
  mindhub admin-code --key FILE --hub URL [TRUST]
                                   a one-time code for the admin pages, signed with an admin key
  mindhub call --key FILE --hub URL [TRUST] METHOD PATH [BODY]
                                   one call signed with a key (an admin's, an agent's), its status and answer printed
  mindhub conformance --hub URL [TRUST] --admin-key F --intake-key F --track-key F --track TRK --integrator-key F
                                   the conformance suite against a hub, from a network that registered no key there today
TRUST, for an https:// hub: --pin SPKI-SHA256 (repeatable), the hub's key as devices pin it; or --ca BUNDLE.pem, a chain
to one of its CAs named for the URL's host (an agent behind a proxy that ends TLS). HTTPS_PROXY is used, CONNECT, unless
NO_PROXY names the host or it is a loopback one; --proxy http://HOST:PORT names another, --proxy none uses none.";

fn fail(why: &str) -> ! { eprintln!("mindhub: {why}"); exit(1) }

// How the hub of `url` is known and reached, from --pin, --ca and --proxy, and HTTPS_PROXY otherwise (100003-APP-0160).
fn route(args: &[String], url: &str) -> (client::Trust, Option<String>) {
    let flag = |name: &str| args.iter().position(|a| a == name).map(|i| args.get(i + 1).cloned().unwrap_or_else(|| fail(&format!("{name} takes a value"))));
    let pins: Vec<[u8; 32]> = args.windows(2).filter(|w| w[0] == "--pin").map(|w| improve_proto::parse_hex32(&w[1]).unwrap_or_else(|| fail("--pin takes 64 hex"))).collect();
    let trust = match (flag("--ca"), pins.is_empty()) {
        (Some(_), false) => fail("--pin or --ca, not both"),
        (Some(ca), true) => client::Trust::Roots(client::roots(ca.as_ref()).unwrap_or_else(|e| fail(&e))),
        (None, _) => client::Trust::Pins(pins),
    };
    let host = url.split_once("://").map_or(url, |(_, r)| r).trim_end_matches('/');
    let host = host.rsplit_once(':').map_or(host, |(h, _)| h).trim_start_matches('[').trim_end_matches(']');
    let proxy = match flag("--proxy").as_deref() {
        Some("none") => None,
        Some(p) => Some(client::proxy(p).unwrap_or_else(|e| fail(&e))),
        None if url.starts_with("https://") => client::proxy_from_env(host).unwrap_or_else(|e| fail(&format!("HTTPS_PROXY: {e}"))),
        None => None,
    };
    (trust, proxy)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).map(|i| args.get(i + 1).cloned().unwrap_or_else(|| fail(&format!("{name} takes a value"))));
    match args.first().map(String::as_str) {
        Some("serve") => {
            let updates = flag("--releases").map(|dir| {
                let public: Vec<String> = args.windows(2).filter(|w| w[0] == "--public").map(|w| w[1].clone()).collect();
                let per_minute = flag("--updates-per-minute").map_or(600, |n| n.parse().ok().filter(|&n| n > 0).unwrap_or_else(|| fail("--updates-per-minute takes a number above 0")));
                Arc::new(mindhub::updates::Updates::new(dir.into(), public, per_minute).unwrap_or_else(|e| fail(&e)))
            });
            serve(args.get(1).cloned().unwrap_or_else(|| fail(USAGE)).into(), flag("--listen"), flag("--cert"), flag("--key"), updates)
        }
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
            let url = flag("--hub").unwrap_or_else(|| fail(USAGE));
            let (trust, proxy) = route(&args, &url);
            let mut c = client::Client::connect(&url, trust, proxy, key).unwrap_or_else(|e| fail(&e));
            let reply = c.call("POST", "/hub/admin/v1/sign-in-code", b"").unwrap_or_else(|e| fail(&e.to_string()));
            match reply.json().get("code").and_then(|v| v.as_str()) {
                Some(code) if reply.status == 201 => println!("{code}  (for the admin pages, once, within 10 minutes)"),
                _ => fail(&format!("the hub answered {}: {}", reply.status, reply.text())),
            }
        }
        Some("call") => {
            let key = client::read_key(&PathBuf::from(flag("--key").unwrap_or_else(|| fail(USAGE)))).unwrap_or_else(|e| fail(&e));
            // The words after the flags and their values: METHOD PATH [BODY].
            let mut rest = Vec::new();
            let mut i = 1;
            while i < args.len() { if args[i].starts_with("--") { i += 2; } else { rest.push(args[i].clone()); i += 1; } }
            let (Some(method), Some(path)) = (rest.first(), rest.get(1)) else { fail(USAGE) };
            let url = flag("--hub").unwrap_or_else(|| fail(USAGE));
            let (trust, proxy) = route(&args, &url);
            let mut c = client::Client::connect(&url, trust, proxy, key).unwrap_or_else(|e| fail(&e));
            let reply = c.call(method, path, rest.get(2).map(String::as_bytes).unwrap_or(b"")).unwrap_or_else(|e| fail(&e.to_string()));
            println!("{}\n{}", reply.status, reply.text());
            if reply.status >= 300 { exit(1); }
        }
        Some("conformance") => {
            let key = |name: &str| client::read_key(&PathBuf::from(flag(name).unwrap_or_else(|| fail(USAGE)))).unwrap_or_else(|e| fail(&e));
            let url = flag("--hub").unwrap_or_else(|| fail(USAGE));
            let (trust, proxy) = route(&args, &url);
            let target = conformance::Target {
                url, trust, proxy,
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

fn serve(data: PathBuf, listen: Option<String>, cert: Option<String>, key: Option<String>, updates: Option<Arc<mindhub::updates::Updates>>) {
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
    if let Some(u) = &updates { eprintln!("MINDHUB: THE RELEASE TREE {}, CHANNELS {} OFFERED UNDER {}/", u.root.display(), u.public.join(" "), mindhub::updates::PREFIX); }
    let shared = Arc::new(Mutex::new(shared));
    let options = server::Options { tls, updates, ..server::Options::new() };
    if let Err(e) = server::serve(listener, shared, options) { fail(&e.to_string()); }
}
