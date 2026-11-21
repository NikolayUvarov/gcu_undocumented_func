//! Host tests of MIND Hub's protocol crate (100003-APP-0145). The vectors in `vectors/` are what a hub in any language
//! must give, in plain words and hex: each test that makes one compares it with its file; `IMPROVE_PROTO_WRITE_VECTORS=1`
//! writes it anew.

use ed25519_dalek::SigningKey;
use improve_proto::bucket::Bucket;
use improve_proto::form::*;
use improve_proto::json::{self, Value};
use improve_proto::state::{self, Role, State, STATES};
use improve_proto::trust::{self, Access, Event, Trust, BANDS, PENDING};
use improve_proto::{repeat, sign, wire, work, DAY_MS};
use std::fmt::Write;

fn vector(name: &str, made: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vectors").join(name);
    if std::env::var_os("IMPROVE_PROTO_WRITE_VECTORS").is_some() {
        std::fs::write(&path, made).unwrap();
        return;
    }
    let kept = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {} (IMPROVE_PROTO_WRITE_VECTORS=1 writes it)", path.display(), e));
    assert_eq!(kept, made, "{} differs from what the crate makes", name);
}

const ROLES: [Role; 6] = [Role::Hub, Role::User, Role::Intake, Role::Track, Role::Integrator, Role::Admin];

#[test]
fn trust_bands_and_access() {
    assert_eq!(Trust::new(1024), None);
    assert_eq!(Trust::new(-1024), None);
    let t = |v| Trust::new(v).unwrap();
    assert_eq!((t(0).band(), t(127).band(), t(128).band(), t(1023).band(), t(-1).band()), (Some(0), Some(0), Some(1), Some(7), None));
    // Each band doubles the requests a day.
    for (i, band) in BANDS.iter().enumerate() { assert_eq!(band.requests_per_day, 1 << i, "band {}", i); }
    assert_eq!(trust::access(t(-1023), true), Access::Banned);
    assert_eq!(trust::access(t(-1), true), Access::ReadOnly);
    assert_eq!(trust::access(t(-1), false), Access::ReadOnly, "a negative user's new device files nothing either");
    assert_eq!(trust::access(t(0), false), Access::Pending(PENDING));
    assert_eq!(trust::access(t(300), true), Access::Open(BANDS[2]));
    // The channels: stable for all, test from band 2, dev from band 4 (the maintainer's choice of 2026-11-20).
    let reach = |v: i16| match trust::access(t(v), true) { Access::Open(l) => (l.channels.allows("stable"), l.channels.allows("test"), l.channels.allows("dev")), _ => (false, false, false) };
    assert_eq!((reach(255), reach(256), reach(511), reach(512)), ((true, false, false), (true, true, false), (true, true, false), (true, true, true)));
    assert!(!BANDS[7].channels.allows("nightly"), "an unknown channel is not served");
    assert_eq!(PENDING.attachments, 0);
}

#[test]
fn the_hubs_own_trust_changes_stay_within_their_bounds() {
    let t = |v| Trust::new(v).unwrap();
    assert_eq!(t(0).after(Event::Accepted).value(), 32);
    assert_eq!(t(500).after(Event::Accepted).value(), 512, "a raise stops at +512");
    assert_eq!(t(512).after(Event::Verified).value(), 512);
    assert_eq!(t(800).after(Event::Accepted).value(), 800, "nor undoes what the admin set above it");
    assert_eq!(t(800).after(Event::NotARequest).value(), 784, "a cut from above +512 is a cut");
    assert_eq!(t(-200).after(Event::Spam).value(), -256, "a cut stops at -256");
    assert_eq!(t(-600).after(Event::Spam).value(), -600, "nor deepens what the admin set below it");
    assert_eq!(t(-600).after(Event::Accepted).value(), -568);
    assert_eq!(t(0).after(Event::Spam).value(), -256, "spam alone never bans");
    let mut made = String::new();
    for v in [-1023, -600, -256, -100, -1, 0, 100, 480, 512, 800, 1023] {
        for e in [Event::Accepted, Event::Verified, Event::NotARequest, Event::Spam] {
            writeln!(made, "{} {} {}", v, e.name(), t(v).after(e).value()).unwrap();
        }
    }
    vector("trust.txt", &made);
}

#[test]
fn a_bucket_over_a_simulated_day_for_each_band() {
    for (band, limits) in BANDS.iter().enumerate() {
        let mut b = Bucket::new(limits.burst, limits.requests_per_day, 0);
        let mut taken = 0;
        let mut now = 0;
        // A request a minute for two days: the burst at once, then the day's rate.
        while now < 2 * DAY_MS {
            if b.take(now).is_ok() { taken += 1; }
            now += 60_000;
        }
        let expected = limits.burst + 2 * limits.requests_per_day;
        assert!(taken + 1 >= expected && taken <= expected, "band {}: {} taken, {} expected", band, taken, expected);
    }
    let mut b = Bucket::new(1, 1, 0);
    assert_eq!(b.take(0), Ok(()));
    assert_eq!(b.take(0), Err(Some(DAY_MS)), "the wait names when the next token is there");
    assert_eq!(b.take(DAY_MS / 2), Err(Some(DAY_MS / 2)));
    assert_eq!(b.take(DAY_MS), Ok(()));
    // A clock that went back adds nothing, and the bucket counts again from where it was.
    let mut b = Bucket::new(2, 2, 10 * DAY_MS);
    assert_eq!((b.take(10 * DAY_MS), b.take(10 * DAY_MS)), (Ok(()), Ok(())));
    assert!(b.take(DAY_MS).is_err());
    assert_eq!(b.tokens(10 * DAY_MS + DAY_MS / 2), 1);
    assert_eq!(Bucket::new(0, 0, 0).take(5), Err(None), "a rate of 0 never gives");
    // A band raised keeps what was spent.
    let mut b = Bucket::new(1, 1, 0);
    b.take(0).unwrap();
    let mut b = b.resized(4, 4, 0);
    assert!(b.take(0).is_err());
    assert_eq!(b.take(DAY_MS / 4), Ok(()));
    let mut made = String::new();
    let mut b = Bucket::new(3, 4, 0);
    for step in 0..24u64 {
        let at = step * DAY_MS / 12;
        let taken = match b.take(at) { Ok(()) => "taken".to_string(), Err(Some(ms)) => format!("wait {}", ms), Err(None) => "never".to_string() };
        writeln!(made, "{} {} {}", at, taken, b.tokens(at)).unwrap();
    }
    vector("bucket.txt", &made);
}

#[test]
fn every_move_by_every_role() {
    let mut made = String::new();
    for from in STATES {
        for to in STATES {
            for role in ROLES {
                if state::allowed(from, to, role) { writeln!(made, "{} {} {}", role.name(), from.name(), to.name()).unwrap(); }
            }
        }
    }
    vector("states.txt", &made);
    use State::*;
    // The table's main paths, and what it refuses.
    assert_eq!((state::first(false), state::first(true)), (Held, New));
    for (from, to, role) in [(Held, New, Role::Hub), (New, Accepted, Role::Intake), (Accepted, InProgress, Role::Track), (InProgress, Landed, Role::Integrator),
                             (Landed, Delivered, Role::Hub), (Delivered, AwaitingCheck, Role::Hub), (AwaitingCheck, Verified, Role::User),
                             (AwaitingCheck, Reopened, Role::User), (Reopened, InProgress, Role::Track), (New, NeedsInfo, Role::Intake), (NeedsInfo, New, Role::User),
                             (AwaitingCheck, ClosedUnchecked, Role::Hub), (ClosedUnchecked, Reopened, Role::User), (Rejected, New, Role::Admin), (InProgress, Rejected, Role::Admin)] {
        assert!(state::allowed(from, to, role), "{:?} {:?} -> {:?}", role, from, to);
    }
    for (from, to, role) in [(New, Accepted, Role::User), (AwaitingCheck, Verified, Role::Track), (Verified, Reopened, Role::User), (Landed, InProgress, Role::Track),
                             (InProgress, Landed, Role::Track), (Held, New, Role::Intake), (Verified, Rejected, Role::Admin), (New, Verified, Role::Admin),
                             (Delivered, Verified, Role::User), (Rejected, New, Role::Intake)] {
        assert!(!state::allowed(from, to, role), "{:?} {:?} -> {:?}", role, from, to);
    }
    for s in STATES { assert_eq!(State::from_name(s.name()), Some(s)); }
    assert!(Accepted.on_board() && !New.on_board() && !Rejected.on_board() && !Held.on_board() && !NeedsInfo.on_board());
}

fn limits() -> trust::Limits { BANDS[0] }

fn request() -> Request {
    Request { client_id: "c-0001-abcd".into(), kind: Kind::Improvement, title: "Тише вентилятор в простое".into(), text: "На Ryzen вентилятор шумит.\nВо всех режимах.".into(),
              visibility: Visibility::Public, security: false, named: false, attachments: vec![] }
}

#[test]
fn a_requests_bounds_at_their_limit_and_one_past() {
    assert_eq!(request().check(&limits()), Ok(()));
    let with = |f: &dyn Fn(&mut Request)| { let mut r = request(); f(&mut r); r.check(&limits()) };
    assert_eq!(with(&|r| r.title = "я".repeat(TITLE_MAX)), Ok(()), "120 characters of two bytes each");
    assert_eq!(with(&|r| r.title = "я".repeat(TITLE_MAX + 1)), Err(Error::TooLong("title", TITLE_MAX)));
    assert_eq!(with(&|r| r.title = "  ".into()), Err(Error::Empty("title")));
    assert_eq!(with(&|r| r.title = "a\tb".into()), Err(Error::Control("title")));
    assert_eq!(with(&|r| r.text = "x".repeat(TEXT_MAX)), Ok(()));
    assert_eq!(with(&|r| r.text = "x".repeat(TEXT_MAX + 1)), Err(Error::TooLong("text", TEXT_MAX)));
    assert_eq!(with(&|r| r.text = "a\u{1b}[31mb".into()), Err(Error::Control("text")), "no escape sequence into a console");
    assert_eq!(with(&|r| r.text = "see https://a.example http://b.example HTTPS://c.example".into()), Ok(()), "3 links in band 0");
    assert_eq!(with(&|r| r.text = "https://a https://b https://c https://d".into()), Err(Error::Links(3)));
    let mut r = request();
    r.text = "https://a https://b https://c https://d".into();
    assert_eq!(r.check(&BANDS[2]), Ok(()), "band 2 counts no links");
    assert_eq!(with(&|r| r.security = true), Err(Error::Value("visibility")), "a security report is private");
    assert_eq!(with(&|r| { r.security = true; r.visibility = Visibility::Private }), Ok(()));
    assert_eq!(with(&|r| r.client_id = "short".into()), Err(Error::Value("client_id")));
    assert_eq!(with(&|r| r.client_id = "a".repeat(CLIENT_ID_MAX + 1)), Err(Error::Value("client_id")));
    let attachment = |name: &str, size| Attachment { kind: AttachmentKind::Log, name: name.into(), size, sha256: [7; 32] };
    assert_eq!(with(&|r| r.attachments = vec![attachment("boot.txt", 256 * 1024)]), Ok(()));
    assert_eq!(with(&|r| r.attachments = vec![attachment("boot.txt", 256 * 1024 + 1)]), Err(Error::Size("attachments.size", 256 * 1024)));
    assert_eq!(with(&|r| r.attachments = vec![attachment("a", 1), attachment("b", 1)]), Err(Error::Count("attachments", 1)));
    assert_eq!(with(&|r| r.attachments = vec![attachment("../x", 1)]), Err(Error::Value("attachments.name")));
    assert_eq!(with(&|r| r.attachments = vec![attachment(".hidden", 1)]), Err(Error::Value("attachments.name")));
    assert_eq!(with(&|r| r.attachments = vec![attachment("x", 0)]), Err(Error::Empty("attachments.size")));
    let mut two = request();
    two.attachments = vec![attachment("a", 1), attachment("a", 2)];
    assert_eq!(two.check(&BANDS[1]), Err(Error::Value("attachments.name")), "two of one name");
    assert_eq!(request().check(&PENDING), Ok(()));
    let mut held = request();
    held.attachments = vec![attachment("a", 1)];
    assert_eq!(held.check(&PENDING), Err(Error::Count("attachments", 0)), "a pending device attaches nothing");
}

fn registration() -> Registration {
    Registration { public_key: [9; 32], name: "Ryzen".into(), kind: DeviceKind::Mind, platform: "mindcore-x64".into(), version: "202611201200".into(),
                   joining: Joining::NewUser { display_name: "Николай".into() }, contact: None, work: None }
}

#[test]
fn a_registration_and_a_message_in_their_bounds() {
    assert_eq!(registration().check(), Ok(()));
    let with = |f: &dyn Fn(&mut Registration)| { let mut r = registration(); f(&mut r); r.check() };
    assert_eq!(with(&|r| r.name = "n".repeat(NAME_MAX + 1)), Err(Error::TooLong("name", NAME_MAX)));
    assert_eq!(with(&|r| r.joining = Joining::Pair { code: "ABCD2345".into() }), Ok(()));
    assert_eq!(with(&|r| r.joining = Joining::Pair { code: "ABCD234O".into() }), Err(Error::Value("pair_code")), "no O, which reads as 0");
    assert_eq!(with(&|r| r.joining = Joining::Pair { code: "abcd2345".into() }), Err(Error::Value("pair_code")));
    assert_eq!(with(&|r| r.contact = Some("owner@example.org".into())), Ok(()));
    for wrong in ["owner", "@example.org", "a@b@c", "a b@c"] { assert_eq!(with(&|r| r.contact = Some(wrong.into())), Err(Error::Value("contact")), "{}", wrong); }
    let message = |kind, answers, verdict: Option<Verdict>, release: Option<&str>| Message { kind, text: "ok".into(), answers, verdict, release: release.map(String::from), attachments: vec![] };
    assert_eq!(message(MessageKind::Comment, None, None, None).check(), Ok(()));
    assert_eq!(message(MessageKind::Answer, Some(3), None, None).check(), Ok(()));
    assert_eq!(message(MessageKind::Answer, None, None, None).check(), Err(Error::Value("kind")), "an answer names its question");
    assert_eq!(message(MessageKind::Verdict, None, Some(Verdict::Works), Some("202611201200")).check(), Ok(()));
    assert_eq!(message(MessageKind::Verdict, None, Some(Verdict::Works), None).check(), Err(Error::Empty("release")), "a verdict names the release it was checked on");
    assert_eq!(message(MessageKind::Comment, None, Some(Verdict::Works), None).check(), Err(Error::Value("kind")));
    assert_eq!(message(MessageKind::Result, None, None, Some("202611201200")).check(), Ok(()));
    assert_eq!(message(MessageKind::Question, None, None, Some("x")).check(), Err(Error::Value("release")));
    // 100003-APP-0148: attachments with an answer or a verdict alone, within the largest band's bounds.
    let log = Attachment { kind: AttachmentKind::Log, name: "boot.log".into(), size: 10, sha256: [1; 32] };
    let with = |m: Message, list: Vec<Attachment>| Message { attachments: list, ..m }.check();
    assert_eq!(with(message(MessageKind::Answer, Some(3), None, None), vec![log.clone()]), Ok(()));
    assert_eq!(with(message(MessageKind::Verdict, None, Some(Verdict::Partly), Some("202611201200")), vec![log.clone()]), Ok(()));
    assert_eq!(with(message(MessageKind::Comment, None, None, None), vec![log.clone()]), Err(Error::Value("attachments")));
    assert_eq!(with(message(MessageKind::Answer, Some(3), None, None), vec![log.clone(), log.clone()]), Err(Error::Value("attachments.name")));
    assert_eq!(with(message(MessageKind::Answer, Some(3), None, None), vec![log.clone(); 5]), Err(Error::Count("attachments", 4)));
    let huge = Attachment { size: 8 * 1024 * 1024 + 1, ..log.clone() };
    assert_eq!(with(message(MessageKind::Answer, Some(3), None, None), vec![huge]), Err(Error::Size("attachments.size", 8 * 1024 * 1024)));
    assert_eq!(attachments_within(&[log.clone()], &BANDS[0]), Ok(()));
    assert_eq!(attachments_within(&[log], &PENDING), Err(Error::Count("attachments", 0)));
    // Who may post what.
    assert!(may_post(Role::User, MessageKind::Verdict) && !may_post(Role::User, MessageKind::Question) && !may_post(Role::User, MessageKind::Status));
    assert!(may_post(Role::Track, MessageKind::Proposal) && !may_post(Role::Track, MessageKind::Verdict));
    assert!(may_post(Role::Hub, MessageKind::Status) && !may_post(Role::Admin, MessageKind::Status));
    let change = Change { from: State::New, to: State::Accepted, role: Role::Intake, at_ms: 1, reason: "100003-APP-0145".into() };
    assert_eq!(change.check(), Ok(()));
    assert_eq!(Change { role: Role::User, ..change.clone() }.check(), Err(Error::Value("to")));
    assert_eq!(Change { reason: " ".into(), ..change }.check(), Err(Error::Empty("reason")));
}

#[test]
fn a_repeat_is_the_same_text_however_spaced_or_cased() {
    assert_eq!(repeat::normalise("  Тише   Вентилятор\n\tв простое "), "тише вентилятор в простое");
    assert_eq!(repeat::digest("Fan  NOISE", "idle\n\nloud"), repeat::digest("fan noise", "IDLE loud"));
    assert_ne!(repeat::digest("fan noise", "idle loud"), repeat::digest("fan noise", "idle louder"));
    assert_ne!(repeat::digest("a", "b c"), repeat::digest("a b", "c"), "the title and the text stay apart");
    vector("repeat.txt", &format!("{}\n{}\n", improve_proto::hex32(&repeat::digest("Fan  NOISE", "idle\n\nloud")), improve_proto::hex32(&repeat::digest("Тише", "вентилятор"))));
}

#[test]
fn signatures_over_the_hubs_challenge() {
    let key = SigningKey::from_bytes(&[42; 32]);
    let challenge = [5u8; 32];
    let body = br#"{"api":"hub-api 1"}"#;
    let header = sign::header(&key, "POST", "/hub/v1/requests", body, &challenge).unwrap();
    let signed = sign::parse_header(&header).unwrap();
    assert_eq!(signed.public_key, *key.verifying_key().as_bytes());
    assert_eq!(sign::verify(&signed, "POST", "/hub/v1/requests", body), Ok(()));
    assert_eq!(sign::verify(&signed, "POST", "/hub/v1/requests", b"{}"), Err(sign::Error::Signature), "a changed body");
    assert_eq!(sign::verify(&signed, "PUT", "/hub/v1/requests", body), Err(sign::Error::Signature), "another method");
    assert_eq!(sign::verify(&signed, "POST", "/hub/v1/devices", body), Err(sign::Error::Signature), "another path");
    let mut other = signed;
    other.challenge = [6; 32];
    assert_eq!(sign::verify(&other, "POST", "/hub/v1/requests", body), Err(sign::Error::Signature), "another challenge: a replay");
    assert_eq!(sign::signed_bytes("PATCH", "/x", b"", &challenge), Err(sign::Error::Request));
    assert_eq!(sign::signed_bytes("GET", "x", b"", &challenge), Err(sign::Error::Request));
    assert_eq!(sign::signed_bytes("GET", "/a b", b"", &challenge), Err(sign::Error::Request));
    for wrong in ["", "rsa 00 00 00", &header.to_uppercase(), &format!("{} extra", header), &header.replacen(' ', "  ", 1)] {
        assert_eq!(sign::parse_header(wrong), Err(sign::Error::Header), "{:?}", wrong);
    }
    // A small-order public key never verifies.
    let mut weak = signed;
    weak.public_key = [0; 32];
    weak.public_key[0] = 1;
    assert!(sign::verify(&weak, "POST", "/hub/v1/requests", body).is_err());
    let id = sign::id(key.verifying_key().as_bytes());
    vector("signature.txt", &format!("key-seed {}\nmethod POST\npath /hub/v1/requests\nbody {}\nchallenge {}\nsigned {}\nheader {}\nid {}\n",
        improve_proto::hex32(&[42; 32]), std::str::from_utf8(body).unwrap(), improve_proto::hex32(&challenge),
        improve_proto::hex(&sign::signed_bytes("POST", "/hub/v1/requests", body, &challenge).unwrap()), header, improve_proto::hex32(&id)));
}

#[test]
fn the_proof_of_work() {
    let challenge = [3u8; 32];
    assert_eq!(work::zero_bits(&[0u8; 32]), 256);
    let mut d = [0xffu8; 32];
    d[0] = 0;
    d[1] = 0x10;
    assert_eq!(work::zero_bits(&d), 11);
    let n = work::solve(&challenge, 12).unwrap();
    assert!(work::check(&challenge, n, 12));
    assert!(work::check(&challenge, n, 0), "0 bits takes any counter");
    assert!((0..n).all(|c| !work::check(&challenge, c, 12)), "the first one that answers");
    assert!(!work::check(&challenge, n, work::BITS_MAX + 1), "never above the most the hub may ask");
    assert_eq!(work::solve(&challenge, work::BITS_MAX + 1), None);
    vector("work.txt", &format!("challenge {} bits 12 counter {}\n", improve_proto::hex32(&challenge), n));
}

#[test]
fn the_json_reader_takes_one_encoding() {
    assert_eq!(json::parse(r#" {"a":[1,-2,true,null,"xé😀"]} "#).unwrap(),
               Value::Object(vec![("a".into(), Value::Array(vec![Value::Int(1), Value::Int(-2), Value::Bool(true), Value::Null, Value::Str("xé😀".into())]))]));
    for (text, what) in [(r#"{"a":1,"a":2}"#, "a key twice"), ("01", "a leading zero"), ("-0", "a signed zero"), ("1.5", "a fraction"), ("1e3", "an exponent"),
                         ("9223372036854775808", "past i64"), ("\"a\u{1}\"", "a raw control character"), (r#""\x""#, "an escape JSON has not"),
                         (r#""\ud800""#, "a lone high surrogate"), (r#""\udc00""#, "a lone low surrogate"), ("[1,]", "a trailing comma"), ("{} {}", "two values"),
                         ("'a'", "single quotes"), ("[1 2]", "no comma"), ("tru", "a cut word"), ("", "nothing")] {
        assert!(json::parse(text).is_err(), "{}: {:?}", what, text);
    }
    assert_eq!(json::parse("-9223372036854775808"), Ok(Value::Int(i64::MIN)));
    let deep = "[".repeat(json::DEPTH) + &"]".repeat(json::DEPTH);
    assert!(json::parse(&deep).is_ok());
    let deeper = "[".repeat(json::DEPTH + 1) + &"]".repeat(json::DEPTH + 1);
    assert_eq!(json::parse(&deeper), Err(json::Error::TooDeep));
    assert_eq!(json::parse(&" ".repeat(json::BODY_MAX + 1)), Err(json::Error::TooLong));
}

#[test]
fn the_forms_round_trip_and_refuse_unknown_keys() {
    let mut r = request();
    r.attachments = vec![Attachment { kind: AttachmentKind::Screenshot, name: "screen-001.bmp".into(), size: 1234, sha256: [0xab; 32] }];
    r.text = "quote \" backslash \\ line\nend".into();
    let text = wire::encode_request(&r);
    assert_eq!(wire::read(&text, wire::decode_request), Ok(r));
    let reg = registration();
    assert_eq!(wire::read(&wire::encode_registration(&reg), wire::decode_registration), Ok(reg.clone()));
    let paired = Registration { joining: Joining::Pair { code: "ABCD2345".into() }, contact: Some("a@b".into()), work: Some(77), ..reg };
    assert_eq!(wire::read(&wire::encode_registration(&paired), wire::decode_registration), Ok(paired));
    let m = Message { kind: MessageKind::Verdict, text: "работает".into(), answers: None, verdict: Some(Verdict::DoesNotWork), release: Some("202611201200".into()),
                      attachments: vec![Attachment { kind: AttachmentKind::Log, name: "after.log".into(), size: 2048, sha256: [0x5c; 32] }] };
    assert_eq!(wire::read(&wire::encode_message(&m), wire::decode_message), Ok(m.clone()));
    let plain = Message { attachments: vec![], ..m.clone() };
    assert!(!wire::encode_message(&plain).contains("attachments"), "no attachments, no key");
    let empty = wire::encode_message(&plain).replace(r#""release""#, r#""attachments":[],"release""#);
    assert_eq!(wire::read(&empty, wire::decode_message), Err(wire::Error::Value("attachments")), "an empty list is not its encoding");
    let named = Request { named: true, ..request() };
    assert_eq!(wire::read(&wire::encode_request(&named), wire::decode_request), Ok(named));
    let c = Change { from: State::Landed, to: State::Delivered, role: Role::Hub, at_ms: 123, reason: "release 202611201200 on stable".into() };
    assert_eq!(wire::read(&wire::encode_change(&c), wire::decode_change), Ok(c));
    let board = vec![BoardEntry { id: 7, title: "Тише вентилятор".into(), kind: Kind::Improvement, track: "DRV".into(), task: "213-DRV-0099".into(),
                                  state: State::InProgress, since_ms: 5, release: None, author: None }];
    assert_eq!(wire::read(&wire::encode_board(&board), wire::decode_board), Ok(board.clone()));
    assert_eq!(board[0].check(), Ok(()));
    // Exactly its keys: an unknown one, a missing one, another type, another version.
    let mut extra = wire::encode_request(&request());
    extra.insert_str(extra.len() - 1, r#","grant":"all""#);
    assert_eq!(wire::read(&extra, wire::decode_request), Err(wire::Error::Unknown("grant".into())));
    let missing = wire::encode_request(&request()).replace(r#","security":false"#, "");
    assert_eq!(wire::read(&missing, wire::decode_request), Err(wire::Error::Missing("security")));
    let typed = wire::encode_request(&request()).replace(r#""security":false"#, r#""security":"no""#);
    assert_eq!(wire::read(&typed, wire::decode_request), Err(wire::Error::Type("security")));
    let unknown = wire::encode_request(&request()).replace("hub-api 2", "hub-api 0");
    assert_eq!(wire::read(&unknown, wire::decode_request), Err(wire::Error::Api));
    // The version before is refused by its name, never read (MC-12.4).
    let old = wire::encode_request(&request()).replace("hub-api 2", "hub-api 1");
    assert_eq!(wire::read(&old, wire::decode_request), Err(wire::Error::Retired("hub-api 1")));
    let missing_named = wire::encode_request(&request()).replace(r#","named":false"#, "");
    assert_eq!(wire::read(&missing_named, wire::decode_request), Err(wire::Error::Missing("named")));
    let both = wire::encode_registration(&registration()).replace(r#""display_name""#, r#""pair_code":"ABCD2345","display_name""#);
    assert_eq!(wire::read(&both, wire::decode_registration), Err(wire::Error::Value("pair_code")));
    let negative = wire::encode_change(&Change { from: State::New, to: State::Accepted, role: Role::Intake, at_ms: 1, reason: "x".into() }).replace(r#""at_ms":1"#, r#""at_ms":-1"#);
    assert_eq!(wire::read(&negative, wire::decode_change), Err(wire::Error::Value("at_ms")));
    vector("request.json", &(wire::encode_request(&request()) + "\n"));
    vector("message.json", &(wire::encode_message(&m) + "\n"));
}
