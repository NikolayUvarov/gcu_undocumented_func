//! The hub's pages for a person in a browser (100003-APP-0155): a proposal of the agents shown with its three answers,
//! and one of them sent by its form, as the API would take it.

use ed25519_dalek::SigningKey;
use improve_proto::form::{DeviceKind, Joining, Kind, Registration, Request, Visibility};
use improve_proto::wire;
use mindhub::client::{Client, Reply};
use mindhub::server::{self, Options};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

fn key(n: u8) -> SigningKey { SigningKey::from_bytes(&[n; 32]) }
fn public(n: u8) -> String { improve_proto::hex(key(n).verifying_key().as_bytes()) }

fn ok(r: Reply) -> String { assert!(r.status < 400, "{} {}", r.status, r.text()); r.text() }

fn form(fields: &[(&str, &str)]) -> Vec<u8> {
    fields.iter().map(|(k, v)| format!("{k}={}", v.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>()))
        .collect::<Vec<_>>().join("&").into_bytes()
}

#[test]
fn a_proposal_is_answered_from_its_page() {
    let dir = std::env::temp_dir().join(format!("mindhub-pages-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("hub.conf"), format!("admin maintainer {}\nagent APP intake {}\nagent APP track {}\n", public(1), public(2), public(3))).unwrap();
    let shared = server::open(&dir).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    std::thread::spawn(move || server::serve(listener, Arc::new(Mutex::new(shared)), Options::new()));
    let (mut admin, mut intake, mut track, mut user, mut browser) = (Client::new(&url, &[], key(1)).unwrap(), Client::new(&url, &[], key(2)).unwrap(),
        Client::new(&url, &[], key(3)).unwrap(), Client::new(&url, &[], key(7)).unwrap(), Client::new(&url, &[], key(9)).unwrap());
    // A user, approved, with a request the track proposes on.
    let reg = Registration { public_key: user.public_key(), name: "phone".into(), kind: DeviceKind::Host, platform: "test".into(), version: "1".into(),
                             joining: Joining::NewUser { display_name: "Dana".into() }, contact: None, work: None };
    let v = ok(user.call("POST", "/hub/v1/devices", wire::encode_registration(&reg).as_bytes()).unwrap());
    let device = v.split("\"device\":\"").nth(1).unwrap().split('"').next().unwrap().to_string();
    ok(admin.call("POST", &format!("/hub/admin/v1/devices/{device}/decision"), br#"{"api":"hub-api 3","decision":"approve","text":"Welcome"}"#).unwrap());
    let request = Request { client_id: "00112233aabbccdd".into(), kind: Kind::Improvement, title: "Two cameras".into(), text: "Switch between them".into(), visibility: Visibility::Public,
                            security: false, named: false, attachments: vec![] };
    ok(user.call("POST", "/hub/v1/requests", wire::encode_request(&request).as_bytes()).unwrap());
    ok(intake.call("POST", "/hub/agent/v1/requests/1/state", br#"{"api":"hub-api 3","to":"accepted","reason":"cameras","track":"APP","task":"158-APP-9999"}"#).unwrap());
    let v = ok(track.call("POST", "/hub/agent/v1/requests/1/messages", br#"{"api":"hub-api 3","kind":"proposal","text":"A key n to the next camera"}"#).unwrap());
    let p = v.split("\"n\":").nth(1).unwrap().split(|c: char| !c.is_ascii_digit()).next().unwrap().to_string();
    // The browser, signed in by a pairing code, sees the proposal waiting and its three answers.
    let code = ok(user.call("POST", "/hub/v1/devices/pair", b"").unwrap()).split("\"code\":\"").nth(1).unwrap().split('"').next().unwrap().to_string();
    let r = browser.plain("POST", "/hub/sign-in", &[("Content-Type", "application/x-www-form-urlencoded".into())], &form(&[("code", &code)])).unwrap();
    let session = r.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("set-cookie")).map(|(_, v)| v.split(';').next().unwrap().to_string()).unwrap();
    let home = ok(browser.plain("GET", "/hub/", &[("Cookie", session.clone())], b"").unwrap());
    assert!(home.contains("a proposal for you"), "{home}");
    let page = ok(browser.plain("GET", "/hub/requests/1", &[("Cookie", session.clone())], b"").unwrap());
    assert!(page.contains(&format!("<h2>Proposal #{p}</h2>")) && page.contains("name=consent value=accept") && page.contains("name=consent value=decline"), "{page}");
    let t = page.split("name=t value=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
    // A correction with no words is refused; an acceptance with none is taken, and the track reads it.
    let r = browser.plain("POST", "/hub/requests/1/messages", &[("Cookie", session.clone())], &form(&[("t", &t), ("kind", "answer"), ("answers", &p), ("text", ""), ("consent", "correct")])).unwrap();
    assert_eq!(r.status, 400, "{}", r.text());
    let r = browser.plain("POST", "/hub/requests/1/messages", &[("Cookie", session.clone())], &form(&[("t", &t), ("kind", "answer"), ("answers", &p), ("text", ""), ("consent", "accept")])).unwrap();
    assert!(r.status < 400, "{} {}", r.status, r.text());
    let thread = ok(track.call("GET", "/hub/agent/v1/requests/1", b"").unwrap());
    assert!(thread.contains(&format!(r#""answers":{p},"consent":"accept""#)), "{thread}");
    let page = ok(browser.plain("GET", "/hub/requests/1", &[("Cookie", session)], b"").unwrap());
    assert!(!page.contains(&format!("<h2>Proposal #{p}</h2>")) && page.contains(&format!("to #{p}: accepted")) && page.contains("accepted by its user"), "{page}");
    let _ = std::fs::remove_dir_all(&dir);
}
