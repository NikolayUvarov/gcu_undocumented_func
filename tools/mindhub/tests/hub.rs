//! The hub's state and commands on a clock of the test's own, with the journal on disk: registration and the admin's
//! answer, held requests, rejected keys, the bands, a negative trust, floods, repeats, threads, the agents' moves, the
//! board, the user's check, and a restart rebuilding the same state (100003-APP-0146).

use ed25519_dalek::SigningKey;
use improve_proto::form::{self, Attachment, AttachmentKind, DeviceKind, Joining, Kind, Message, MessageKind, Registration, Request, Verdict, Visibility};
use improve_proto::state::{Role, State};
use improve_proto::{sign, work};
use mindhub::event::{Decision, Event, Owner};
use mindhub::hub::{Actor, Agent, DeviceState, Extra, Hub, Keys, Refusal, Settings};
use mindhub::journal::{self, Journal};
use mindhub::util::{sha256, DAY_MS, HOUR_MS};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const T0: u64 = 1_795_000_000_000;

fn key(n: u8) -> SigningKey { SigningKey::from_bytes(&[n; 32]) }
fn pk(n: u8) -> [u8; 32] { key(n).verifying_key().to_bytes() }
fn id(n: u8) -> [u8; 32] { sign::id(&pk(n)) }

const ADMIN: u8 = 200;
const INTAKE: u8 = 201;
const APP: u8 = 202;
const NET: u8 = 203;
const LANDER: u8 = 204;

struct World { hub: Hub, journal: Journal, dir: PathBuf, now: u64 }

impl Drop for World { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); } }

fn keys() -> Keys {
    let mut k = Keys::default();
    k.admins.insert(pk(ADMIN), "maintainer".into());
    k.agents.insert(pk(INTAKE), Agent { track: "APP".into(), role: Role::Intake });
    k.agents.insert(pk(APP), Agent { track: "APP".into(), role: Role::Track });
    k.agents.insert(pk(NET), Agent { track: "NET".into(), role: Role::Track });
    k.agents.insert(pk(LANDER), Agent { track: "KRN".into(), role: Role::Integrator });
    k
}

impl World {
    fn new() -> World {
        let dir = std::env::temp_dir().join(format!("mindhub-test-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(&dir).unwrap();
        let mut hub = Hub::new(keys(), Settings::default());
        let journal = Journal::open(&dir.join("journal.jsonl"), &mut hub).unwrap();
        World { hub, journal, dir, now: T0 }
    }

    fn commit(&mut self, events: Result<Vec<Event>, Refusal>) -> Result<(), Refusal> {
        let events = events?;
        let erasing = events.iter().any(|e| matches!(e, Event::Deleted { .. } | Event::Erased { .. }));
        journal::commit(&mut self.hub, &mut self.journal, events).unwrap();
        if erasing { journal::erase(&self.hub, &mut self.journal, &self.dir.join("attachments"), self.now).unwrap(); }
        Ok(())
    }

    fn journal_text(&self) -> String { std::fs::read_to_string(self.dir.join("journal.jsonl")).unwrap() }

    /// A second hub from this one's journal.
    fn replayed(&self) -> Hub {
        let mut again = Hub::new(keys(), Settings::default());
        Journal::open(&self.dir.join("journal.jsonl"), &mut again).unwrap();
        again
    }

    fn later(&mut self, ms: u64) { self.now += ms; }

    fn register(&mut self, n: u8, network: &str) -> Result<(), Refusal> {
        let reg = registration(n, Joining::NewUser { display_name: format!("user {n}") });
        let r = self.hub.register(self.now, &reg, &pk(n), &[0; 32], network).map(|(e, _)| e);
        self.commit(r)
    }

    fn decide(&mut self, n: u8, d: Decision, text: &str) -> Result<(), Refusal> {
        let r = self.hub.decide(self.now, "maintainer", &id(n), d, text);
        self.commit(r)
    }

    /// A user of their own, approved.
    fn approved(&mut self, n: u8, network: &str) {
        self.register(n, network).unwrap();
        self.decide(n, Decision::Approve, "welcome").unwrap();
    }

    fn file(&mut self, n: u8, title: &str) -> Result<u64, Refusal> {
        let form = request(title, Visibility::Public);
        self.file_form(n, &form)
    }

    fn file_form(&mut self, n: u8, form: &Request) -> Result<u64, Refusal> {
        let (events, id, _) = self.hub.file(self.now, &id(n), form)?;
        self.commit(Ok(events))?;
        Ok(id)
    }

    fn post(&mut self, by: Actor, req: u64, m: Message) -> Result<(), Refusal> {
        let r = self.hub.post(self.now, &by, req, &m);
        self.commit(r)
    }

    fn mv(&mut self, by: Actor, req: u64, to: State, extra: Extra) -> Result<(), Refusal> {
        let r = self.hub.move_request(self.now, &by, req, to, "the reason", &extra);
        self.commit(r)
    }

    fn trust(&mut self, n: u8, to: i16) {
        let user = self.hub.device(&id(n)).unwrap().user;
        let r = self.hub.set_trust(self.now, "maintainer", user, to, "set by the test");
        self.commit(r).unwrap();
    }

    fn state(&self, req: u64) -> State { self.hub.request(req).unwrap().state }
    fn user_trust(&self, n: u8) -> i16 { self.hub.user(self.hub.device(&id(n)).unwrap().user).unwrap().trust.value() }

    /// The state as the tests see it, to compare two hubs.
    fn digest(&self) -> String {
        let mut out = String::new();
        let mut devices: Vec<_> = self.hub.devices().collect();
        devices.sort_by_key(|d| d.id);
        for d in devices { out += &format!("{:?} {} {:?} {} {:?} {} {:?} {}\n", d.id, d.user, d.state, d.version, d.notes, d.name, d.contact, d.network); }
        for u in self.hub.users() { out += &format!("{} {} {} {} {:?} {:?}\n", u.id, u.display_name, u.erased, u.trust.value(), u.history, u.devices); }
        for r in self.hub.requests() {
            out += &format!("{} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {} {:?} {} {:?}\n", r.id, r.state, r.form, r.thread, r.changes, r.track, r.task, r.release, r.deleted, r.flags, r.cleared, r.received);
        }
        out += &format!("{:?} {:?}\n", self.hub.settings, self.hub.board());
        for n in [1u8, 2, 3] { out += &format!("{:?}\n", self.hub.quotas(&id(n), self.now)); }
        out
    }
}

fn registration(n: u8, joining: Joining) -> Registration {
    Registration { public_key: pk(n), name: format!("device {n}"), kind: DeviceKind::Mind, platform: "x86_64".into(), version: "202611200000".into(),
                   joining, contact: None, work: None }
}

fn request(title: &str, visibility: Visibility) -> Request {
    Request { client_id: format!("client-{}", hex8(title)), kind: Kind::Fault, title: title.into(), text: format!("What happens: {title}."), visibility, security: false, named: false, attachments: Vec::new() }
}

fn hex8(text: &str) -> String { improve_proto::hex(&sha256(text.as_bytes())[..8]) }

fn message(kind: MessageKind, text: &str) -> Message { Message { kind, text: text.into(), answers: None, verdict: None, release: None, attachments: Vec::new() } }

fn limit(r: Result<impl std::fmt::Debug, Refusal>) -> (u16, String) {
    let e = r.expect_err("a refusal");
    (e.status, e.limit)
}

#[test]
fn a_device_registered_asked_answered_approved_and_its_held_request_released() {
    let mut w = World::new();
    w.register(1, "10.0.0.1").unwrap();
    assert_eq!(w.hub.device(&id(1)).unwrap().state, DeviceState::Pending);
    // A pending device files one held request, which no agent and no board sees.
    let held = w.file(1, "The screen stays black").unwrap();
    assert_eq!(w.state(held), State::Held);
    assert_eq!(limit(w.file(1, "Another one")), (403, "held".into()));
    assert!(w.hub.queue(&Agent { track: "APP".into(), role: Role::Intake }).is_empty());
    // The admin asks; the device answers; several rounds are allowed.
    w.decide(1, Decision::Ask, "Which machine is it?").unwrap();
    w.commit(w.hub.answer_registration(w.now, &id(1), "A ThinkPad X230")).unwrap();
    w.decide(1, Decision::Ask, "Since when?").unwrap();
    w.commit(w.hub.answer_registration(w.now, &id(1), "Since yesterday's update")).unwrap();
    let notes: Vec<&str> = w.hub.device(&id(1)).unwrap().notes.iter().map(|n| n.kind).collect();
    assert_eq!(notes, ["question", "answer", "question", "answer"]);
    // Approval releases the held request to intake, as if filed then.
    w.decide(1, Decision::Approve, "Welcome; the rules are on the board").unwrap();
    assert_eq!(w.hub.device(&id(1)).unwrap().state, DeviceState::Approved);
    assert_eq!(w.state(held), State::New);
    assert_eq!(w.hub.queue(&Agent { track: "APP".into(), role: Role::Intake }).len(), 1);
    // A decided registration takes no more answers, and is not decided twice.
    assert_eq!(limit(w.hub.answer_registration(w.now, &id(1), "more")), (409, "state".into()));
    assert_eq!(limit(w.decide(1, Decision::Reject, "no")), (409, "state".into()));
}

#[test]
fn a_pending_devices_messages_are_five_a_day() {
    let mut w = World::new();
    w.register(1, "10.0.0.1").unwrap();
    for _ in 0..5 { w.commit(w.hub.answer_registration(w.now, &id(1), "hello")).unwrap(); }
    let e = w.hub.answer_registration(w.now, &id(1), "hello").unwrap_err();
    assert_eq!((e.status, e.limit.as_str()), (429, "messages"));
    assert!(e.retry_after_ms.unwrap() > 0);
    w.later(DAY_MS / 5 + 1);
    w.commit(w.hub.answer_registration(w.now, &id(1), "hello")).unwrap();
}

#[test]
fn a_rejected_key_is_refused_and_its_held_requests_dropped() {
    let mut w = World::new();
    w.register(1, "10.0.0.1").unwrap();
    let held = w.file(1, "Please do my homework").unwrap();
    w.decide(1, Decision::Reject, "Not about MIND Core").unwrap();
    assert_eq!(w.state(held), State::Rejected);
    assert_eq!(limit(w.file(1, "Again")), (403, "rejected".into()));
    assert_eq!(limit(w.register(1, "10.0.0.1")), (403, "rejected".into()));
    assert_eq!(limit(w.hub.access(&id(1))), (403, "rejected".into()));
    // The admin's reason stays for the device to read.
    assert_eq!(w.hub.device(&id(1)).unwrap().notes.last().unwrap().text, "Not about MIND Core");
}

#[test]
fn each_bands_limits_hold() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    // Band 0: one a day, a burst of one, links at most 3, one attachment of 256 KiB.
    let first = w.file(1, "One").unwrap();
    let e = w.file(1, "Two").unwrap_err();
    assert_eq!((e.status, e.limit.as_str()), (429, "requests"));
    let wait = e.retry_after_ms.unwrap();
    assert!(wait > 0 && wait <= DAY_MS, "{wait}");
    w.later(wait);
    w.file(1, "Two").unwrap();
    w.later(DAY_MS);
    let mut links = request("Links", Visibility::Public);
    links.text = "see http://a http://b https://c http://d".into();
    assert_eq!(limit(w.file_form(1, &links)), (413, "links".into()));
    let mut two = request("Attachments", Visibility::Public);
    let a = |name: &str, size| Attachment { kind: AttachmentKind::Log, name: name.into(), size, sha256: [7; 32] };
    two.attachments = vec![a("a.log", 10), a("b.log", 10)];
    assert_eq!(limit(w.file_form(1, &two)), (413, "attachments".into()));
    two.attachments = vec![a("a.log", 256 * 1024 + 1)];
    assert_eq!(limit(w.file_form(1, &two)), (413, "attachments.size".into()));
    // Band 2: the links bound is gone, four attachments of 1 MiB, a burst of 3, the test channel.
    w.trust(1, 256);
    w.later(DAY_MS);
    two.attachments = vec![a("a.log", 1 << 20), a("b.log", 1), a("c.log", 1), a("d.log", 1)];
    w.file_form(1, &links).unwrap();
    assert!(w.hub.limits(&id(1)).unwrap().channels.allows("test"));
    assert!(!w.hub.limits(&id(1)).unwrap().channels.allows("dev"));
    w.later(DAY_MS);
    w.file_form(1, &two).unwrap();
    // Band 7: 128 a day, a burst of 16.
    w.trust(1, 1023);
    assert_eq!(w.hub.limits(&id(1)).unwrap().burst, 16);
    let _ = first;
}

#[test]
fn a_negative_trust_is_read_only_and_minus_1023_a_ban() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    let r = w.file(1, "Before").unwrap();
    w.trust(1, -1);
    w.later(DAY_MS);
    assert_eq!(limit(w.file(1, "After")), (403, "trust".into()));
    assert_eq!(limit(w.post(Actor::Device(id(1)), r, message(MessageKind::Comment, "hi"))), (403, "trust".into()));
    // Read-only: the user's own requests are still theirs to read.
    assert!(w.hub.access(&id(1)).is_ok());
    w.trust(1, -1023);
    assert_eq!(limit(w.hub.access(&id(1))), (403, "ban".into()));
    // A banned user's code pairs no new device: none can be made, and none made before is taken.
    assert_eq!(limit(w.hub.make_code(w.now, Owner::User { user: 1, device: id(1) }, "23456789")), (403, "ban".into()));
}

#[test]
fn a_flood_from_one_network_is_held_back() {
    let mut w = World::new();
    for n in 1..=5 { w.register(n, "198.51.100.7").unwrap(); }
    let e = w.register(6, "198.51.100.7").unwrap_err();
    assert_eq!((e.status, e.limit.as_str()), (429, "registrations"));
    assert_eq!(e.retry_after_ms, Some(DAY_MS));
    // Another network is not held back by this one.
    w.register(6, "203.0.113.9").unwrap();
    // Three held requests a day from one network.
    for n in 1..=3 { w.file(n, &format!("Held {n}")).unwrap(); }
    assert_eq!(limit(w.file(4, "Held 4")), (429, "held".into()));
    // A day later the network registers again, but after a rejection only one key a day.
    w.later(DAY_MS);
    w.decide(1, Decision::Reject, "spam").unwrap();
    w.register(7, "198.51.100.7").unwrap();
    assert_eq!(limit(w.register(8, "198.51.100.7")), (429, "registrations".into()));
}

#[test]
fn the_global_cap_holds_back_low_bands_alone() {
    let mut w = World::new();
    w.commit(w.hub.set_setting(w.now, "maintainer", "global-cap", "2")).unwrap();
    for n in 1..=4 { w.approved(n, &format!("10.0.0.{n}")); }
    w.file(1, "A").unwrap();
    w.file(2, "B").unwrap();
    let e = w.file(3, "C").unwrap_err();
    assert_eq!((e.status, e.limit.as_str()), (429, "intake"));
    assert_eq!(e.retry_after_ms, Some(HOUR_MS));
    // A trusted user is not held back by it.
    w.trust(4, 300);
    w.file(4, "D").unwrap();
    w.later(HOUR_MS);
    w.file(3, "C").unwrap();
}

#[test]
fn a_repeat_is_refused_with_the_earlier_number_and_a_client_id_files_once() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    let form = request("Wi-Fi drops", Visibility::Public);
    let first = w.file_form(1, &form).unwrap();
    // The same client ID again: the same request, no new one and no token.
    let (events, again, before) = w.hub.file(w.now, &id(1), &form).unwrap();
    assert!(events.is_empty() && before && again == first);
    // The same text with another client ID, case and spacing aside: refused, naming the earlier one, costing no token.
    w.later(DAY_MS);
    let mut repeat = form.clone();
    repeat.client_id = "another-client".into();
    repeat.title = "  WI-FI   drops ".into();
    let e = w.hub.file(w.now, &id(1), &repeat).unwrap_err();
    assert_eq!((e.status, e.limit.as_str(), e.earlier), (409, "repeat", Some(first)));
    w.file(1, "Something else").unwrap();
    // After 30 days it is a new request.
    w.later(30 * DAY_MS);
    w.file_form(1, &repeat).unwrap();
}

#[test]
fn a_thread_with_a_question_and_its_answer_and_an_agents_states() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    let r = w.file(1, "The camera does not switch").unwrap();
    let intake = Actor::Agent(pk(INTAKE));
    // Intake asks: the request waits on the user, and the user's answer takes it back.
    w.post(intake.clone(), r, message(MessageKind::Question, "Which cameras are plugged in?")).unwrap();
    assert_eq!(w.state(r), State::NeedsInfo);
    let q = w.hub.request(r).unwrap().thread.iter().find(|p| p.message.kind == MessageKind::Question).unwrap().n;
    let answer = Message { answers: Some(q), ..message(MessageKind::Answer, "Two USB ones") };
    // An answer is taken even when the day's messages are spent.
    for _ in 0..20 { w.post(Actor::Device(id(1)), r, message(MessageKind::Comment, "more")).unwrap_or_else(|_| ()); }
    assert_eq!(limit(w.post(Actor::Device(id(1)), r, message(MessageKind::Comment, "more"))), (429, "messages".into()));
    w.post(Actor::Device(id(1)), r, answer.clone()).unwrap();
    assert_eq!(w.state(r), State::New);
    // Once only.
    assert_eq!(limit(w.post(Actor::Device(id(1)), r, answer)), (400, "answers".into()));
    // A user posts no question; an agent no verdict.
    assert_eq!(limit(w.post(Actor::Device(id(1)), r, message(MessageKind::Question, "?"))), (403, "kind".into()));
    // Intake accepts it into APP's plan: the user's trust is raised once.
    assert_eq!(limit(w.mv(intake.clone(), r, State::Accepted, Extra::default())), (400, "track".into()));
    w.mv(intake.clone(), r, State::Accepted, Extra { track: Some("APP".into()), task: Some("158-APP-0150".into()), ..Extra::default() }).unwrap();
    assert_eq!(w.user_trust(1), 32);
    // Another track's agent may not move it; the owning track's may, and asks again.
    assert_eq!(limit(w.mv(Actor::Agent(pk(NET)), r, State::InProgress, Extra::default())), (403, "track".into()));
    w.mv(Actor::Agent(pk(APP)), r, State::InProgress, Extra::default()).unwrap();
    w.post(Actor::Agent(pk(APP)), r, message(MessageKind::Question, "Does n switch it?")).unwrap();
    assert_eq!(w.state(r), State::NeedsInfo);
    let q = w.hub.request(r).unwrap().thread.iter().rev().find(|p| p.message.kind == MessageKind::Question).unwrap().n;
    w.post(Actor::Device(id(1)), r, Message { answers: Some(q), ..message(MessageKind::Answer, "No") }).unwrap();
    assert_eq!(w.state(r), State::InProgress);
    w.post(Actor::Agent(pk(APP)), r, Message { release: Some("202611210000".into()), ..message(MessageKind::Result, "Fixed: press n, then check the title") }).unwrap();
    // The integrator lands it; the admin's word that a release carries it; the device runs it; the user checks.
    assert_eq!(limit(w.mv(Actor::Agent(pk(APP)), r, State::Landed, Extra::default())), (409, "state".into()));
    w.mv(Actor::Agent(pk(LANDER)), r, State::Landed, Extra::default()).unwrap();
    w.commit(w.hub.delivered(w.now, "maintainer", "202611210000", &[r])).unwrap();
    assert_eq!(w.state(r), State::Delivered);
    w.commit(w.hub.report_version(w.now, &id(1), "202611210000")).unwrap();
    assert_eq!(w.state(r), State::AwaitingCheck);
    let works = Message { verdict: Some(Verdict::Works), release: Some("202611210000".into()), ..message(MessageKind::Verdict, "It switches") };
    w.later(DAY_MS);
    w.post(Actor::Device(id(1)), r, works).unwrap();
    assert_eq!(w.state(r), State::Verified);
    assert_eq!(w.user_trust(1), 48);
    let roles: Vec<Role> = w.hub.request(r).unwrap().changes.iter().map(|c| c.role).collect();
    assert_eq!(roles, [Role::Intake, Role::User, Role::Intake, Role::Track, Role::Track, Role::User, Role::Integrator, Role::Hub, Role::Hub, Role::User]);
}

#[test]
fn a_verdict_that_it_does_not_work_reopens_and_silence_closes_it_unchecked() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    let r = w.file(1, "Sound crackles").unwrap();
    w.mv(Actor::Admin("maintainer".into()), r, State::Accepted, Extra { track: Some("APP".into()), task: Some("551-APP-0160".into()), ..Extra::default() }).unwrap();
    w.mv(Actor::Agent(pk(APP)), r, State::InProgress, Extra::default()).unwrap();
    w.mv(Actor::Agent(pk(LANDER)), r, State::Landed, Extra::default()).unwrap();
    w.commit(w.hub.report_version(w.now, &id(1), "202611220000")).unwrap();
    // The device runs a later release already: delivered and awaiting the check at once.
    w.commit(w.hub.delivered(w.now, "maintainer", "202611210000", &[r])).unwrap();
    assert_eq!(w.state(r), State::AwaitingCheck);
    let no = Message { verdict: Some(Verdict::DoesNotWork), release: Some("202611220000".into()), ..message(MessageKind::Verdict, "Still crackles") };
    w.post(Actor::Device(id(1)), r, no).unwrap();
    assert_eq!(w.state(r), State::Reopened);
    w.mv(Actor::Agent(pk(APP)), r, State::InProgress, Extra::default()).unwrap();
    w.mv(Actor::Agent(pk(LANDER)), r, State::Landed, Extra::default()).unwrap();
    w.commit(w.hub.delivered(w.now, "maintainer", "202611230000", &[r])).unwrap();
    assert_eq!(w.state(r), State::Delivered);
    w.commit(w.hub.report_version(w.now, &id(1), "202611230000")).unwrap();
    assert!(w.hub.sweep(w.now + 29 * DAY_MS).is_empty());
    w.later(30 * DAY_MS);
    w.commit(Ok(w.hub.sweep(w.now))).unwrap();
    assert_eq!(w.state(r), State::ClosedUnchecked);
    // Closed unchecked, never verified; a works verdict is not taken now, a reopening one is.
    let works = Message { verdict: Some(Verdict::Works), release: Some("202611230000".into()), ..message(MessageKind::Verdict, "ok") };
    assert_eq!(limit(w.post(Actor::Device(id(1)), r, works)), (409, "state".into()));
    let partly = Message { verdict: Some(Verdict::Partly), release: Some("202611230000".into()), ..message(MessageKind::Verdict, "half") };
    w.post(Actor::Device(id(1)), r, partly).unwrap();
    assert_eq!(w.state(r), State::Reopened);
}

#[test]
fn agents_read_what_is_theirs_and_the_board_shows_only_what_it_should() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    w.approved(2, "10.0.0.2");
    w.trust(1, 400);
    // A raised band keeps the credit it had; a day fills its burst of four.
    w.later(DAY_MS);
    let public = w.file(1, "Public fault").unwrap();
    let private = w.file_form(1, &request("Private fault", Visibility::Private)).unwrap();
    let mut sec = request("A hole", Visibility::Private);
    sec.security = true;
    let security = w.file_form(1, &sec).unwrap();
    let new = w.file(1, "Still new").unwrap();
    let held = { w.register(3, "10.0.0.3").unwrap(); w.file(3, "Held one").unwrap() };
    let admin = Actor::Admin("maintainer".into());
    for (r, t) in [(public, "APP"), (private, "APP"), (security, "APP")] {
        w.mv(admin.clone(), r, State::Accepted, Extra { track: Some(t.into()), task: Some(format!("000-APP-{r:04}")), ..Extra::default() }).unwrap();
    }
    let rejected = w.file(2, "Spam").unwrap();
    w.mv(admin.clone(), rejected, State::Rejected, Extra { spam: true, ..Extra::default() }).unwrap();
    assert_eq!(w.user_trust(2), -256);
    let board: Vec<u64> = w.hub.board().iter().map(|e| e.id).collect();
    assert_eq!(board, [public]);
    assert!(w.hub.board().iter().all(|e| e.author.is_none()));
    // NET's agent reads the public ones alone (a rejected one too); APP's reads its own; none reads a held one.
    let net = Agent { track: "NET".into(), role: Role::Track };
    let app = Agent { track: "APP".into(), role: Role::Track };
    let readable = |a: &Agent| -> Vec<u64> { w.hub.requests().iter().filter(|r| w.hub.agent_reads(a, r)).map(|r| r.id).collect() };
    assert_eq!(readable(&net), [public, new, rejected]);
    assert_eq!(readable(&app), [public, private, security, new, rejected]);
    assert!(!readable(&Agent { track: "APP".into(), role: Role::Intake }).contains(&held));
    assert_eq!(limit(w.post(Actor::Agent(pk(NET)), private, message(MessageKind::Comment, "hi"))), (404, "request".into()));
    // Only the admin marks spam.
    w.later(DAY_MS);
    let other = w.file(1, "Another").unwrap();
    assert_eq!(limit(w.mv(Actor::Agent(pk(INTAKE)), other, State::Rejected, Extra { spam: true, ..Extra::default() })), (403, "spam".into()));
    w.mv(Actor::Agent(pk(INTAKE)), other, State::Rejected, Extra { not_a_request: true, ..Extra::default() }).unwrap();
    assert_eq!(w.user_trust(1), 400 + 3 * 32 - 16);
}

#[test]
fn the_hub_moves_trust_by_itself_within_its_bounds() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    w.trust(1, 500);
    let admin = Actor::Admin("maintainer".into());
    for n in 0..2 {
        w.later(DAY_MS);
        let r = w.file(1, &format!("Accepted {n}")).unwrap();
        w.mv(admin.clone(), r, State::Accepted, Extra { track: Some("APP".into()), task: Some(format!("000-APP-{n:04}")), ..Extra::default() }).unwrap();
    }
    assert_eq!(w.user_trust(1), 512);
    // Beyond what the admin set the hub takes nothing back, and below -256 it cuts nothing more.
    w.trust(1, 900);
    w.later(DAY_MS);
    let r = w.file(1, "Accepted 3").unwrap();
    w.mv(admin.clone(), r, State::Accepted, Extra { track: Some("APP".into()), task: Some("000-APP-0003".into()), ..Extra::default() }).unwrap();
    assert_eq!(w.user_trust(1), 900);
    w.trust(1, 0);
    w.later(DAY_MS);
    let r = w.file(1, "Spam 1").unwrap();
    w.mv(admin.clone(), r, State::Rejected, Extra { spam: true, ..Extra::default() }).unwrap();
    assert_eq!(w.user_trust(1), -256);
    // Read-only now; the admin's spam mark on a later request cuts no more.
    let user = w.hub.device(&id(1)).unwrap().user;
    assert!(w.hub.user(user).unwrap().history.iter().any(|c| c.reason.contains("spam")));
}

#[test]
fn pairing_joins_a_device_and_a_browser_and_revoking_ends_both() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    let pending_code = w.hub.make_code(w.now, Owner::User { user: 1, device: id(1) }, "23456789");
    w.commit(pending_code).unwrap();
    let reg = registration(2, Joining::Pair { code: "23456789".into() });
    let (events, _) = w.hub.register(w.now, &reg, &pk(2), &[0; 32], "192.0.2.5").unwrap();
    w.commit(Ok(events)).unwrap();
    let d = w.hub.device(&id(2)).unwrap();
    assert_eq!((d.state, d.user, d.paired), (DeviceState::Approved, 1, true));
    // A code works once.
    let reg3 = registration(3, Joining::Pair { code: "23456789".into() });
    assert_eq!(limit(w.hub.register(w.now, &reg3, &pk(3), &[0; 32], "192.0.2.5")), (403, "pair_code".into()));
    // A browser's session by a code made on device 2; it ends when device 2 is revoked from device 1.
    w.commit(w.hub.make_code(w.now, Owner::User { user: 1, device: id(2) }, "ABCDEFGH")).unwrap();
    let token = [9u8; 32];
    assert_eq!(limit(w.hub.open_session(w.now, "ABCDEFGH", true, &token, "192.0.2.5")), (403, "code".into()));
    w.commit(w.hub.open_session(w.now, "abcdefgh", false, &token, "192.0.2.5")).unwrap();
    assert!(w.hub.session(&token, w.now).is_some());
    w.commit(w.hub.revoke(w.now, &Actor::Device(id(1)), &id(2))).unwrap();
    assert!(w.hub.session(&token, w.now).is_none());
    assert_eq!(limit(w.hub.access(&id(2))), (403, "revoked".into()));
    // A code lasts 10 minutes; three at once at most.
    for c in ["22222222", "33333333", "44444444"] { w.commit(w.hub.make_code(w.now, Owner::User { user: 1, device: id(1) }, c)).unwrap(); }
    assert_eq!(limit(w.hub.make_code(w.now, Owner::User { user: 1, device: id(1) }, "55555555")), (429, "codes".into()));
    w.later(10 * 60_000);
    let reg4 = registration(4, Joining::Pair { code: "22222222".into() });
    assert_eq!(limit(w.hub.register(w.now, &reg4, &pk(4), &[0; 32], "192.0.2.5")), (403, "pair_code".into()));
    // Wrong codes from one network wait after ten an hour.
    for _ in 0..10 { assert!(w.hub.open_session(w.now, "ZZZZZZZZ", false, &token, "192.0.2.9").is_err()); w.hub.note_failure("192.0.2.9", w.now); }
    assert_eq!(limit(w.hub.open_session(w.now, "ZZZZZZZZ", false, &token, "192.0.2.9")), (429, "sign-in".into()));
}

#[test]
fn the_proof_of_work_when_the_admin_turns_it_on() {
    let mut w = World::new();
    w.commit(w.hub.set_setting(w.now, "maintainer", "work-bits", "8")).unwrap();
    let challenge = [5u8; 32];
    let reg = registration(1, Joining::NewUser { display_name: "u".into() });
    let e = w.hub.register(w.now, &reg, &pk(1), &challenge, "10.0.0.1").unwrap_err();
    assert_eq!((e.status, e.limit.as_str(), e.work_bits), (401, "work", Some(8)));
    let counter = work::solve(&challenge, 8).unwrap();
    let reg = Registration { work: Some(counter), ..reg };
    assert_eq!(limit(w.hub.register(w.now, &reg, &pk(1), &[6; 32], "10.0.0.1")), (401, "work".into()));
    let (events, _) = w.hub.register(w.now, &reg, &pk(1), &challenge, "10.0.0.1").unwrap();
    w.commit(Ok(events)).unwrap();
    assert_eq!(limit(w.commit(w.hub.set_setting(w.now, "maintainer", "work-bits", "29"))), (400, "setting".into()));
}

#[test]
fn the_admin_approved_only_setting_holds_pending_devices_requests() {
    let mut w = World::new();
    w.commit(w.hub.set_setting(w.now, "maintainer", "intake", "approved-only")).unwrap();
    w.register(1, "10.0.0.1").unwrap();
    assert_eq!(limit(w.file(1, "Early")), (403, "intake".into()));
    w.decide(1, Decision::Approve, "ok").unwrap();
    w.file(1, "Now").unwrap();
}

#[test]
fn a_restart_rebuilds_the_same_state_from_the_journal() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    w.register(2, "10.0.0.2").unwrap();
    let held = w.file(2, "Held").unwrap();
    w.decide(2, Decision::Ask, "Who are you?").unwrap();
    w.commit(w.hub.answer_registration(w.now, &id(2), "A tester")).unwrap();
    let r = w.file(1, "Rebuilt").unwrap();
    w.post(Actor::Agent(pk(INTAKE)), r, message(MessageKind::Question, "Which release?")).unwrap();
    w.later(HOUR_MS);
    w.trust(1, 200);
    w.commit(w.hub.set_setting(w.now, "maintainer", "poll-s", "120")).unwrap();
    w.commit(w.hub.make_code(w.now, Owner::User { user: 1, device: id(1) }, "KKKKKKKK")).unwrap();
    w.commit(w.hub.open_session(w.now, "KKKKKKKK", false, &[3; 32], "10.0.0.1")).unwrap();
    let before = w.digest();
    let mut again = Hub::new(keys(), Settings::default());
    Journal::open(&w.dir.join("journal.jsonl"), &mut again).unwrap();
    let rebuilt = World { hub: again, journal: Journal::open(&w.dir.join("other.jsonl"), &mut Hub::new(keys(), Settings::default())).unwrap(), dir: w.dir.join("none"), now: w.now };
    assert_eq!(rebuilt.digest(), before);
    assert_eq!(rebuilt.hub.events, w.hub.events);
    assert!(rebuilt.hub.session(&[3; 32], w.now).is_some());
    assert_eq!(rebuilt.hub.request(held).unwrap().state, State::Held);
    // A cut last line is dropped; a broken line in the middle stops the start, naming it.
    let path = w.dir.join("journal.jsonl");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{text}{{\"event\":\"trust\",\"at_")).unwrap();
    let mut cut = Hub::new(keys(), Settings::default());
    Journal::open(&path, &mut cut).unwrap();
    assert_eq!(cut.events, w.hub.events);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    let mut lines: Vec<&str> = text.lines().collect();
    lines.insert(2, "{\"event\":\"nonsense\",\"at_ms\":1}");
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    let err = Journal::open(&path, &mut Hub::new(keys(), Settings::default())).err().unwrap();
    assert!(err.contains("line 3") && err.contains("nonsense"), "{err}");
    // An event that does not fit the state stops it too: a request moved where the machine does not go.
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    lines.push(format!("{{\"event\":\"moved\",\"at_ms\":{},\"id\":{r},\"to\":\"verified\",\"role\":\"user\",\"author\":\"user\",\"reason\":\"forged\"}}", w.now));
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    let err = Journal::open(&path, &mut Hub::new(keys(), Settings::default())).err().unwrap();
    assert!(err.contains(&format!("line {}", lines.len())) && err.contains("does not move"), "{err}");
    drop(rebuilt);
}

#[test]
fn every_event_reads_back_as_it_was_written() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    let mut form = request("Round trip \"quoted\"\nand ünïcode", Visibility::Private);
    form.title = "Round trip \"quoted\" ünïcode".into();
    form.attachments = vec![Attachment { kind: AttachmentKind::Screenshot, name: "screen.png".into(), size: 100, sha256: [3; 32] }];
    w.file_form(1, &form).unwrap();
    let text = std::fs::read_to_string(w.dir.join("journal.jsonl")).unwrap();
    for line in text.lines() {
        let e = Event::decode(line).unwrap();
        assert_eq!(e.encode(), line);
    }
    assert!(form::TEXT_MAX * 4 < improve_proto::json::BODY_MAX, "a filed event's line stays within the JSON reader's bound");
}

fn accept(w: &mut World, r: u64) {
    w.mv(Actor::Admin("maintainer".into()), r, State::Accepted, Extra { track: Some("APP".into()), task: Some(format!("000-APP-{r:04}")), ..Extra::default() }).unwrap();
}

#[test]
fn a_user_named_on_the_board_by_their_leave_alone() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    w.trust(1, 300);
    w.later(DAY_MS);
    let named = w.file_form(1, &Request { named: true, ..request("Named", Visibility::Public) }).unwrap();
    let plain = w.file(1, "Plain").unwrap();
    accept(&mut w, named);
    accept(&mut w, plain);
    let board = w.hub.board();
    let author = |id| board.iter().find(|e| e.id == id).unwrap().author.clone();
    assert_eq!((author(named), author(plain)), (Some("user 1".to_string()), None));
}

#[test]
fn an_answer_and_a_verdict_carry_attachments_within_the_band() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    let r = w.file(1, "The fan").unwrap();
    w.post(Actor::Agent(pk(INTAKE)), r, message(MessageKind::Question, "Send the boot log, please")).unwrap();
    let q = w.hub.request(r).unwrap().thread.iter().find(|p| p.message.kind == MessageKind::Question).unwrap().n;
    let log = b"[ 0.0 ] fan at 4000 rpm\n".repeat(10);
    let entry = |name: &str, bytes: &[u8]| Attachment { kind: AttachmentKind::Log, name: name.into(), size: bytes.len() as u64, sha256: sha256(bytes) };
    let answer = |list| Message { answers: Some(q), attachments: list, ..message(MessageKind::Answer, "Here it is") };
    // Band 0: one attachment; a comment carries none.
    assert_eq!(limit(w.post(Actor::Device(id(1)), r, answer(vec![entry("a.log", &log), entry("b.log", &log)]))), (413, "attachments".into()));
    let comment = Message { attachments: vec![entry("a.log", &log)], ..message(MessageKind::Comment, "and this") };
    assert_eq!(limit(w.post(Actor::Device(id(1)), r, comment)), (400, "attachments".into()));
    w.post(Actor::Device(id(1)), r, answer(vec![entry("boot.log", &log)])).unwrap();
    let n = w.hub.request(r).unwrap().thread.iter().find(|p| p.message.kind == MessageKind::Answer).unwrap().n;
    let mut wrong = log.clone();
    wrong[0] ^= 1;
    assert_eq!(limit(w.hub.attach(w.now, &id(1), r, Some(n), 0, &wrong)), (400, "sha256".into()));
    assert_eq!(limit(w.hub.attach(w.now, &id(1), r, Some(q), 0, &log)), (404, "message".into()), "the agent's question is not the user's");
    w.commit(w.hub.attach(w.now, &id(1), r, Some(n), 0, &log)).unwrap();
    assert_eq!(w.hub.request(r).unwrap().thread.iter().find(|p| p.n == n).unwrap().received, [true]);
    assert_eq!(w.replayed().request(r).unwrap().thread, w.hub.request(r).unwrap().thread);
}

#[test]
fn a_deleted_request_leaves_the_journal_and_an_accepted_one_stays() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    w.trust(1, 300);
    w.later(DAY_MS);
    let secret = w.file_form(1, &Request { title: "Fan XYZZY noise".into(), text: "My flat in PLUGH street".into(), ..request("x", Visibility::Private) }).unwrap();
    w.post(Actor::Agent(pk(INTAKE)), secret, message(MessageKind::Question, "Which street was it, QUUX?")).unwrap();
    let q = w.hub.request(secret).unwrap().thread.iter().find(|p| p.message.kind == MessageKind::Question).unwrap().n;
    w.post(Actor::Device(id(1)), secret, Message { answers: Some(q), ..message(MessageKind::Answer, "GARPLY street") }).unwrap();
    let kept = w.file(1, "Accepted one").unwrap();
    accept(&mut w, kept);
    for word in ["XYZZY", "PLUGH", "QUUX", "GARPLY"] { assert!(w.journal_text().contains(word), "{word} is in the journal before"); }
    assert_eq!(limit(w.hub.delete_request(w.now, &id(1), kept)), (409, "accepted".into()));
    w.commit(w.hub.delete_request(w.now, &id(1), secret)).unwrap();
    for word in ["XYZZY", "PLUGH", "QUUX", "GARPLY"] { assert!(!w.journal_text().contains(word), "{word} left the journal"); }
    let r = w.hub.request(secret).unwrap();
    assert!(r.deleted && r.form.title == mindhub::event::DELETED);
    assert_eq!(limit(w.post(Actor::Device(id(1)), secret, message(MessageKind::Comment, "hi"))), (404, "request".into()));
    assert_eq!(limit(w.hub.delete_request(w.now, &id(1), secret)), (404, "request".into()));
    assert!(!w.hub.requests().iter().any(|r| r.deleted && w.hub.agent_reads(&Agent { track: "APP".into(), role: Role::Intake }, r)));
    // A repeat of a deleted request is a new one.
    w.later(DAY_MS);
    w.file_form(1, &Request { title: "Fan XYZZY noise".into(), text: "My flat in PLUGH street".into(), client_id: "client-again-1".into(), ..request("x", Visibility::Private) }).unwrap();
    // The replay of the journal as it is now gives the same state.
    let again = World { hub: w.replayed(), journal: Journal::open(&w.dir.join("other.jsonl"), &mut Hub::new(keys(), Settings::default())).unwrap(), dir: w.dir.join("none"), now: w.now };
    assert_eq!(again.digest(), w.digest());
}

#[test]
fn a_deleted_account_keeps_no_name_and_its_tasks_no_author() {
    let mut w = World::new();
    let reg = Registration { contact: Some("alice@example.org".into()), ..registration(1, Joining::NewUser { display_name: "Alice ZORKMID".into() }) };
    let (events, _) = w.hub.register(w.now, &reg, &pk(1), &[0; 32], "10.0.0.1").unwrap();
    w.commit(Ok(events)).unwrap();
    w.decide(1, Decision::Ask, "Who are you?").unwrap();
    w.commit(w.hub.answer_registration(w.now, &id(1), "I live at FROTZ lane")).unwrap();
    w.decide(1, Decision::Approve, "welcome").unwrap();
    w.commit(w.hub.make_code(w.now, Owner::User { user: 1, device: id(1) }, "PAQRPAQR")).unwrap();
    let (events, _) = w.hub.register(w.now, &registration(2, Joining::Pair { code: "PAQRPAQR".into() }), &pk(2), &[0; 32], "10.0.0.1").unwrap();
    w.commit(Ok(events)).unwrap();
    w.trust(1, 300);
    w.later(DAY_MS);
    let task = w.file_form(1, &Request { named: true, ..request("Kept as a task", Visibility::Public) }).unwrap();
    accept(&mut w, task);
    let gone = w.file_form(1, &Request { title: "Gone BLORB".into(), ..request("y", Visibility::Public) }).unwrap();
    assert_eq!(w.hub.board()[0].author.as_deref(), Some("Alice ZORKMID"));
    w.later(2 * DAY_MS);
    w.commit(w.hub.erase(w.now, &id(2))).unwrap();
    for word in ["ZORKMID", "alice@example.org", "FROTZ", "BLORB", "10.0.0.1"] { assert!(!w.journal_text().contains(word), "{word} left the journal"); }
    assert!(w.hub.request(gone).unwrap().deleted && !w.hub.request(task).unwrap().deleted);
    assert_eq!(w.hub.board().iter().map(|e| (e.id, e.author.clone())).collect::<Vec<_>>(), [(task, None)]);
    assert_eq!(limit(w.hub.access(&id(1))), (403, "revoked".into()));
    assert_eq!(limit(w.hub.access(&id(2))), (403, "revoked".into()));
    let u = w.hub.user(1).unwrap();
    assert!(u.erased && u.display_name == mindhub::event::ERASED_NAME);
    let again = World { hub: w.replayed(), journal: Journal::open(&w.dir.join("other.jsonl"), &mut Hub::new(keys(), Settings::default())).unwrap(), dir: w.dir.join("none"), now: w.now };
    assert_eq!(again.digest(), w.digest());
}

#[test]
fn an_export_holds_what_the_hub_keeps_of_a_user() {
    let mut w = World::new();
    let reg = Registration { contact: Some("bob@example.org".into()), ..registration(1, Joining::NewUser { display_name: "Bob".into() }) };
    let (events, _) = w.hub.register(w.now, &reg, &pk(1), &[0; 32], "10.0.0.1").unwrap();
    w.commit(Ok(events)).unwrap();
    w.decide(1, Decision::Approve, "Welcome, Bob").unwrap();
    let r = w.file(1, "Exported").unwrap();
    w.approved(2, "10.0.0.2");
    w.file(2, "Someone else's").unwrap();
    let export = improve_proto::json::parse(&mindhub::api::export(&w.hub, 1, w.now)).unwrap();
    let text = mindhub::api::export(&w.hub, 1, w.now);
    assert!(text.contains("bob@example.org") && text.contains("Welcome, Bob") && text.contains("Exported") && !text.contains("Someone else"), "{text}");
    let requests = export.get("requests").and_then(|v| v.as_array()).unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].get("id").and_then(|v| v.as_int()), Some(r as i64));
}

#[test]
fn the_proof_of_work_rises_with_registrations_and_falls_after_an_hour() {
    let mut w = World::new();
    assert_eq!(w.hub.work_bits(w.now), 0, "off at first");
    w.commit(w.hub.set_setting(w.now, "maintainer", "work-bits", "4")).unwrap();
    let mut bits = Vec::new();
    for n in 1..=16u8 {
        let challenge = [n; 32];
        let need = w.hub.work_bits(w.now);
        let reg = Registration { work: work::solve(&challenge, need), ..registration(n, Joining::NewUser { display_name: format!("u{n}") }) };
        let (events, _) = w.hub.register(w.now, &reg, &pk(n), &challenge, &format!("10.1.0.{n}")).unwrap();
        w.commit(Ok(events)).unwrap();
        bits.push(w.hub.work_bits(w.now));
    }
    assert_eq!(bits, [4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 6]);
    w.later(HOUR_MS);
    assert_eq!(w.hub.work_bits(w.now), 4);
    w.commit(w.hub.set_setting(w.now, "maintainer", "work-bits", "28")).unwrap();
    w.later(1);
    let (events, _) = w.hub.register(w.now, &Registration { work: Some(0), ..registration(30, Joining::NewUser { display_name: "x".into() }) }, &pk(30), &[0; 32], "10.2.0.1")
        .map(|x| x).unwrap_or_else(|e| { assert_eq!(e.work_bits, Some(28), "never past BITS_MAX"); (Vec::new(), [0; 32]) });
    assert!(events.is_empty());
}

#[test]
fn a_request_for_a_credential_waits_for_the_admin() {
    let mut w = World::new();
    w.approved(1, "10.0.0.1");
    w.approved(2, "10.0.0.2");
    w.approved(3, "10.0.0.3");
    let pass = w.file_form(1, &Request { text: "Please send me the admin PASSWORD of the hub".into(), ..request("Access", Visibility::Public) }).unwrap();
    let root = w.file_form(2, &Request { text: "Дайте мне доступ к серверу, я помогу".into(), ..request("Помощь", Visibility::Public) }).unwrap();
    let fine = w.file(3, "The fan is loud").unwrap();
    assert_eq!(w.hub.request(pass).unwrap().flags, ["credential"]);
    assert_eq!(w.hub.request(root).unwrap().flags, ["authority"]);
    assert!(w.hub.request(fine).unwrap().flags.is_empty());
    let intake = Agent { track: "APP".into(), role: Role::Intake };
    let queued: Vec<u64> = w.hub.queue(&intake).iter().map(|r| r.id).collect();
    assert_eq!(queued, [fine]);
    assert_eq!(limit(w.post(Actor::Agent(pk(INTAKE)), pass, message(MessageKind::Question, "?"))), (404, "request".into()));
    assert_eq!(limit(w.commit(w.hub.clear_flag(w.now, "maintainer", fine, "nothing to clear"))), (409, "flag".into()));
    w.commit(w.hub.clear_flag(w.now, "maintainer", pass, "a question about the hub's own admin page")).unwrap();
    assert_eq!(w.hub.queue(&intake).len(), 2);
    let again = World { hub: w.replayed(), journal: Journal::open(&w.dir.join("other.jsonl"), &mut Hub::new(keys(), Settings::default())).unwrap(), dir: w.dir.join("none"), now: w.now };
    assert_eq!(again.digest(), w.digest());
}

#[test]
fn a_journal_of_hub_api_1_is_refused_by_its_name() {
    let w = World::new();
    let reg = Registration { ..registration(1, Joining::NewUser { display_name: "old".into() }) };
    let hub = w.hub.clone();
    let (events, _) = hub.register(w.now, &reg, &pk(1), &[0; 32], "10.0.0.1").unwrap();
    let mut lines: Vec<String> = events.iter().map(|e| e.encode()).collect();
    let filed = Event::Filed { at: w.now, id: 1, user: 1, device: id(1), form: request("Old", Visibility::Public), held: true }.encode();
    lines.push(filed.replace("hub-api 2", "hub-api 1"));
    std::fs::write(w.dir.join("old.jsonl"), lines.join("\n") + "\n").unwrap();
    let err = Journal::open(&w.dir.join("old.jsonl"), &mut Hub::new(keys(), Settings::default())).err().unwrap();
    assert!(err.contains("line 2") && err.contains("hub-api 1"), "{err}");
}

