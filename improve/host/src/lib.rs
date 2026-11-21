//! `improve` for any computer (100003-APP-0150; docs/hub/README.md, sections 2 and 10): a user's requests to MIND Hub,
//! their conversation, the user's check of the result and their devices, over `hub-api 2`. It keeps its own Ed25519 key
//! and the hub's address and pins in a directory of its own, and signs each call over the hub's last challenge with
//! `tools/mindhub`'s client. MIND Core's `improve` (stage 4) gives the same commands.

use ed25519_dalek::SigningKey;
use improve_proto::form::{Attachment, AttachmentKind, DeviceKind, Joining, Kind, Message, MessageKind, Registration, Request, Verdict, Visibility};
use improve_proto::json::Value;
use improve_proto::{wire, work};
use mindhub::client::{self, Client, Reply};
use sha2::Digest;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

pub const USAGE: &str = "usage: improve [--dir DIR] COMMAND
  setup --hub URL [--pin SPKI-SHA256]...      the hub (https needs its pin) and this device's key
  register [--name NAME] (--display-name NAME | --pair CODE) [--contact E-MAIL]
  status                                      the registration, the admin's words, the trust and what is left today
  answer TEXT                                 an answer to the admin's question during the registration
  new --title T (--text T | --text-file F) [--kind fault|improvement|capability] [--private] [--security] [--named]
      [--attach log|hardware|screenshot|text:PATH]... [--yes]
  list | show N
  reply N TEXT [--answer QUESTION] [--attach KIND:PATH]... [--yes]
  check N works|partly|does-not-work --release RELEASE TEXT [--attach KIND:PATH]... [--yes]
  watch [--once]                              waits for news, at the hub's interval
  devices | pair | revoke DEVICE
  export [FILE] | delete N | delete-account --yes";

/// Where the commands read and write: the configuration's directory, the person's answers, the output.
pub struct Env<'a> { pub dir: PathBuf, pub input: &'a mut dyn BufRead, pub out: &'a mut dyn Write }

/// The command's exit status: 0 when done, 1 when refused or failed, with the reason written.
pub fn run(args: &[String], env: &mut Env) -> i32 {
    match command(args, env) {
        Ok(()) => 0,
        Err(why) => { let _ = writeln!(env.out, "improve: {why}"); 1 }
    }
}

/// The flags that take a value; every other flag stands alone.
const VALUED: [&str; 11] = ["--hub", "--pin", "--name", "--display-name", "--pair", "--contact", "--title", "--text", "--text-file", "--kind", "--attach"];
const ALONE: [&str; 5] = ["--private", "--security", "--named", "--yes", "--once"];
const MORE_VALUED: [&str; 2] = ["--answer", "--release"];

struct Args { words: Vec<String>, flags: Vec<(String, Option<String>)> }

impl Args {
    fn parse(args: &[String]) -> Result<Args, String> {
        let (mut words, mut flags) = (Vec::new(), Vec::new());
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            if a.starts_with("--") {
                if VALUED.contains(&a.as_str()) || MORE_VALUED.contains(&a.as_str()) {
                    let v = args.get(i + 1).ok_or_else(|| format!("{a} takes a value"))?;
                    flags.push((a.clone(), Some(v.clone())));
                    i += 2;
                    continue;
                }
                if !ALONE.contains(&a.as_str()) { return Err(format!("no flag {a}\n{USAGE}")); }
                flags.push((a.clone(), None));
            } else {
                words.push(a.clone());
            }
            i += 1;
        }
        Ok(Args { words, flags })
    }
    fn value(&self, name: &str) -> Option<&str> { self.flags.iter().rev().find(|(k, _)| k == name).and_then(|(_, v)| v.as_deref()) }
    fn values(&self, name: &str) -> Vec<&str> { self.flags.iter().filter(|(k, _)| k == name).filter_map(|(_, v)| v.as_deref()).collect() }
    fn has(&self, name: &str) -> bool { self.flags.iter().any(|(k, _)| k == name) }
    fn word(&self, i: usize, what: &str) -> Result<&str, String> { self.words.get(i).map(String::as_str).ok_or_else(|| format!("{what} is missing\n{USAGE}")) }
}

/// The configuration's directory: `$XDG_CONFIG_HOME/mind-improve`, or `~/.config/mind-improve`.
pub fn default_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))).unwrap_or_else(|| PathBuf::from("."));
    base.join("mind-improve")
}

struct Config { hub: String, pins: Vec<[u8; 32]> }

fn load(dir: &Path) -> Result<Config, String> {
    let path = dir.join("hub.conf");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e} (improve setup --hub URL first)", path.display()))?;
    let (mut hub, mut pins) = (None, Vec::new());
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        match line.split_once(' ') {
            Some(("hub", url)) => hub = Some(url.trim().to_string()),
            Some(("pin", pin)) => pins.push(improve_proto::parse_hex32(pin.trim()).ok_or_else(|| format!("{}, line {}: a pin is 64 hex", path.display(), n + 1))?),
            _ => return Err(format!("{}, line {}: not `hub URL` or `pin HEX`", path.display(), n + 1)),
        }
    }
    Ok(Config { hub: hub.ok_or_else(|| format!("{}: no hub", path.display()))?, pins })
}

fn key(dir: &Path) -> Result<SigningKey, String> { client::read_key(&dir.join("device.key")) }

fn connect(dir: &Path) -> Result<Client, String> {
    let config = load(dir)?;
    Client::new(&config.hub, &config.pins, key(dir)?)
}

fn io(e: std::io::Error) -> String { format!("the hub cannot be reached: {e}") }

/// A refusal in words: the hub's detail, the limit it met, and when it ends.
fn refusal(r: &Reply) -> String {
    let v = r.json();
    let Some(limit) = v.get("error").and_then(Value::as_str) else { return format!("the hub answered {} {}", r.status, r.text().trim()) };
    let mut out = format!("refused ({} {limit}): {}", r.status, v.get("detail").and_then(Value::as_str).unwrap_or(""));
    if let Some(s) = v.get("retry_after_s").and_then(Value::as_int) { out += &format!("; again in {}", duration(s as u64)); }
    if let Some(n) = v.get("earlier").and_then(Value::as_int) { out += &format!("; see request {n}"); }
    out
}

fn duration(s: u64) -> String {
    match s {
        0..=59 => format!("{s} s"),
        60..=3599 => format!("{} min", s.div_ceil(60)),
        _ => format!("{} h {} min", s / 3600, (s % 3600) / 60),
    }
}

fn expect(r: Reply, ok: &[u16]) -> Result<Value, String> { if ok.contains(&r.status) { Ok(r.json()) } else { Err(refusal(&r)) } }

fn when(ms: i64) -> String {
    let t = UNIX_EPOCH + Duration::from_millis(ms.max(0) as u64);
    httpd::date::rfc3339(t).replace('T', " ")[..16].to_string() + " UTC"
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn n(v: &Value, k: &str) -> i64 { v.get(k).and_then(Value::as_int).unwrap_or(0) }
fn list<'a>(v: &'a Value, k: &str) -> &'a [Value] { v.get(k).and_then(Value::as_array).unwrap_or(&[]) }

fn hostname() -> String {
    let name = std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok()).unwrap_or_else(|| "host".into());
    name.chars().filter(|c| !c.is_control()).take(64).collect()
}

/// The attachments `--attach KIND:PATH` names, read, with their entries.
fn attachments(args: &Args) -> Result<Vec<(Attachment, Vec<u8>, String)>, String> {
    let mut out = Vec::new();
    for spec in args.values("--attach") {
        let (kind, path) = spec.split_once(':').ok_or("--attach takes KIND:PATH")?;
        let kind = AttachmentKind::from_name(kind).ok_or("an attachment's kind is log, hardware, screenshot or text")?;
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let name: String = Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("attachment").chars().take(64).collect();
        let name = name.trim_start_matches('.').to_string();
        out.push((Attachment { kind, name, size: bytes.len() as u64, sha256: sha2::Sha256::digest(&bytes).into() }, bytes, path.to_string()));
    }
    Ok(out)
}

/// What is about to leave the machine, shown, and the person's yes (design 9); `--yes` gives it for a script.
fn confirm(env: &mut Env, args: &Args, what: &str, files: &[(Attachment, Vec<u8>, String)]) -> Result<bool, String> {
    let out = &mut env.out;
    let _ = writeln!(out, "{what}");
    for (a, _, path) in files { let _ = writeln!(out, "  attachment: {} {} ({} bytes, sha256 {}) from {path}", a.kind.name(), a.name, a.size, improve_proto::hex32(&a.sha256)); }
    if args.has("--yes") { return Ok(true); }
    let _ = write!(out, "send it? [y/N] ");
    let _ = out.flush();
    let mut line = String::new();
    env.input.read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(matches!(line.trim(), "y" | "Y" | "yes"))
}

fn put_files(c: &mut Client, base: &str, files: &[(Attachment, Vec<u8>, String)]) -> Result<(), String> {
    for (i, (a, bytes, _)) in files.iter().enumerate() {
        expect(c.call("PUT", &format!("{base}/attachments/{i}"), bytes).map_err(io)?, &[200]).map_err(|e| format!("{}: {e}", a.name))?;
    }
    Ok(())
}

fn number(text: &str) -> Result<u64, String> { text.parse().map_err(|_| format!("{text:?} is not a request's number")) }

fn command(args: &[String], env: &mut Env) -> Result<(), String> {
    let mut args = args.to_vec();
    if args.first().map(String::as_str) == Some("--dir") {
        if args.len() < 2 { return Err("--dir takes a directory".into()); }
        env.dir = PathBuf::from(args[1].clone());
        args.drain(..2);
    }
    let a = Args::parse(&args)?;
    let dir = env.dir.clone();
    match a.words.first().map(String::as_str) {
        Some("setup") => setup(&a, env),
        Some("register") => register(&a, env),
        Some("status") => status(env),
        Some("answer") => {
            let mut c = connect(&dir)?;
            let body = mindhub_body(&[("text", a.word(1, "the answer")?)]);
            expect(c.call("POST", "/hub/v1/devices/me/messages", &body).map_err(io)?, &[201])?;
            let _ = writeln!(env.out, "sent to the admin");
            Ok(())
        }
        Some("new") => new(&a, env),
        Some("list") => {
            let mut c = connect(&dir)?;
            let v = expect(c.call("GET", "/hub/v1/requests", b"").map_err(io)?, &[200])?;
            let all = list(&v, "requests");
            if all.is_empty() { let _ = writeln!(env.out, "no requests"); }
            for r in all {
                let question = if r.get("open_question").and_then(Value::as_bool) == Some(true) { "  [a question for you]" } else { "" };
                let waits = if !list(r, "flagged").is_empty() && r.get("cleared").and_then(Value::as_bool) != Some(true) { "  [waits for the admin]" } else { "" };
                let _ = writeln!(env.out, "{:>5}  {:<16} {}{question}{waits}", n(r, "id"), s(r, "state"), s(r, "title"));
            }
            Ok(())
        }
        Some("show") => show(number(a.word(1, "the request's number")?)?, env),
        Some("reply") => reply(&a, env),
        Some("check") => check(&a, env),
        Some("watch") => watch(&a, env),
        Some("devices") => {
            let mut c = connect(&dir)?;
            let v = expect(c.call("GET", "/hub/v1/devices", b"").map_err(io)?, &[200])?;
            for d in list(&v, "devices") {
                let this = if d.get("this").and_then(Value::as_bool) == Some(true) { "  (this one)" } else { "" };
                let _ = writeln!(env.out, "{}  {:<9} {} {} {} {}, since {}{this}", &s(d, "device")[..16], s(d, "state"), s(d, "kind"), s(d, "name"), s(d, "platform"),
                                 s(d, "version"), when(n(d, "registered_ms")));
            }
            Ok(())
        }
        Some("pair") => {
            let mut c = connect(&dir)?;
            let v = expect(c.call("POST", "/hub/v1/devices/pair", b"").map_err(io)?, &[201])?;
            let _ = writeln!(env.out, "pairing code {} until {}: another device registers with --pair, or a browser signs in at /hub/", s(&v, "code"), when(n(&v, "until_ms")));
            Ok(())
        }
        Some("revoke") => {
            let mut c = connect(&dir)?;
            let prefix = a.word(1, "the device")?.to_ascii_lowercase();
            let v = expect(c.call("GET", "/hub/v1/devices", b"").map_err(io)?, &[200])?;
            let found: Vec<&str> = list(&v, "devices").iter().map(|d| s(d, "device")).filter(|id| id.starts_with(&prefix)).collect();
            let [id] = found.as_slice() else { return Err(format!("{} devices start with {prefix:?}: name one (improve devices)", found.len())) };
            expect(c.call("DELETE", &format!("/hub/v1/devices/{id}"), b"").map_err(io)?, &[200])?;
            let _ = writeln!(env.out, "revoked {}", &id[..16]);
            Ok(())
        }
        Some("export") => {
            let mut c = connect(&dir)?;
            let r = c.call("GET", "/hub/v1/me/export", b"").map_err(io)?;
            if r.status != 200 { return Err(refusal(&r)); }
            match a.words.get(1) {
                Some(path) => {
                    std::fs::write(path, &r.body).map_err(|e| format!("{path}: {e}"))?;
                    let _ = writeln!(env.out, "exported to {path} ({} bytes)", r.body.len());
                }
                None => { let _ = env.out.write_all(&r.body); let _ = writeln!(env.out); }
            }
            Ok(())
        }
        Some("delete") => {
            let mut c = connect(&dir)?;
            let id = number(a.word(1, "the request's number")?)?;
            expect(c.call("DELETE", &format!("/hub/v1/requests/{id}"), b"").map_err(io)?, &[200])?;
            let _ = writeln!(env.out, "request {id} deleted, with its thread");
            Ok(())
        }
        Some("delete-account") => {
            if !a.has("--yes") { return Err("deleting the account deletes every request no track took and revokes every device: give --yes".into()); }
            let mut c = connect(&dir)?;
            expect(c.call("DELETE", "/hub/v1/me", b"").map_err(io)?, &[200])?;
            let _ = writeln!(env.out, "the account is deleted: its requests no track took, its devices and its name");
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

/// A body of the API's version with string fields.
fn mindhub_body(fields: &[(&str, &str)]) -> Vec<u8> {
    let mut w = improve_proto::json::Writer::new();
    w.begin_object().field_str("api", improve_proto::API);
    for (k, v) in fields { w.field_str(k, v); }
    w.end_object();
    w.finish().into_bytes()
}

fn setup(a: &Args, env: &mut Env) -> Result<(), String> {
    let hub = a.value("--hub").ok_or_else(|| format!("setup needs --hub URL\n{USAGE}"))?.trim_end_matches('/').to_string();
    let pins = a.values("--pin");
    for p in &pins { improve_proto::parse_hex32(p).ok_or("a pin is the hub key's SPKI SHA-256, 64 hex")?; }
    if hub.starts_with("https://") && pins.is_empty() { return Err("a hub over TLS is known by its key: --pin, as the hub printed it at its start".into()); }
    if !hub.starts_with("https://") && !hub.starts_with("http://127.") && !hub.starts_with("http://localhost") && !hub.starts_with("http://[::1]") {
        return Err("a hub off this machine speaks https alone".into());
    }
    std::fs::create_dir_all(&env.dir).map_err(|e| format!("{}: {e}", env.dir.display()))?;
    let mut text = format!("# MIND Hub for improve\nhub {hub}\n");
    for p in pins { text += &format!("pin {p}\n"); }
    std::fs::write(env.dir.join("hub.conf"), text).map_err(|e| e.to_string())?;
    let key_path = env.dir.join("device.key");
    let key = if key_path.exists() { client::read_key(&key_path)? } else { client::write_key(&key_path)? };
    let _ = writeln!(env.out, "hub {hub}\nthis device's key {} in {}", improve_proto::hex32(key.verifying_key().as_bytes()), key_path.display());
    Ok(())
}

fn register(a: &Args, env: &mut Env) -> Result<(), String> {
    let mut c = connect(&env.dir)?;
    let joining = match (a.value("--display-name"), a.value("--pair")) {
        (Some(name), None) => Joining::NewUser { display_name: name.into() },
        (None, Some(code)) => Joining::Pair { code: code.trim().to_ascii_uppercase() },
        _ => return Err("register needs --display-name NAME for a new user, or --pair CODE from a device of yours".into()),
    };
    let mut reg = Registration {
        public_key: c.public_key(), name: a.value("--name").map(String::from).unwrap_or_else(hostname), kind: DeviceKind::Host,
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH), version: env!("CARGO_PKG_VERSION").into(),
        joining, contact: a.value("--contact").map(String::from), work: None,
    };
    reg.check().map_err(|e| format!("the registration is out of its bounds: {e:?}"))?;
    for attempt in 0..2 {
        // The proof of work the hub asks now, over the challenge this call is signed with (5.4).
        let v = expect(c.plain("GET", "/hub/v1/challenge", &[], b"").map_err(io)?, &[200])?;
        let bits = n(&v, "work_bits") as u8;
        reg.work = match (bits, c.challenge()) { (0, _) => None, (b, Some(ch)) => work::solve(&ch, b), _ => None };
        let r = c.call("POST", "/hub/v1/devices", wire::encode_registration(&reg).as_bytes()).map_err(io)?;
        if r.status == 401 && r.error().as_deref() == Some("work") && attempt == 0 { continue; }
        let v = expect(r, &[200, 201])?;
        let state = s(&v, "state");
        let _ = writeln!(env.out, "registered {}: {state}{}", &s(&v, "device")[..16],
                         if state == "pending" { " (the admin decides; improve status shows their words)" } else { "" });
        return Ok(());
    }
    unreachable!()
}

fn status(env: &mut Env) -> Result<(), String> {
    let mut c = connect(&env.dir)?;
    let v = expect(c.call("GET", "/hub/v1/devices/me", b"").map_err(io)?, &[200])?;
    let u = v.get("user").unwrap_or(&Value::Null);
    let band = u.get("band").and_then(Value::as_int).map(|b| format!("band {b}")).unwrap_or_else(|| "read-only".into());
    let q = v.get("quotas").unwrap_or(&Value::Null);
    let _ = writeln!(env.out, "device {} ({}): {}\nuser {} #{}, trust {} ({band}), access {}\nleft now: {} new requests, {} messages",
                     s(&v, "name"), &s(&v, "device")[..16], s(&v, "state"), s(u, "display_name"), n(u, "id"), n(u, "trust"), s(&v, "access"), n(q, "requests"), n(q, "messages"));
    for note in list(&v, "notes") {
        let _ = writeln!(env.out, "  {} {}, {}: {}", s(note, "from"), when(n(note, "at_ms")), s(note, "kind"), s(note, "text"));
    }
    for t in list(&v, "trust_history") {
        let _ = writeln!(env.out, "  trust {} -> {}, {}: {} ({})", n(t, "from"), n(t, "to"), when(n(t, "at_ms")), s(t, "reason"), s(t, "author"));
    }
    Ok(())
}

fn new(a: &Args, env: &mut Env) -> Result<(), String> {
    let text = match (a.value("--text"), a.value("--text-file")) {
        (Some(t), None) => t.to_string(),
        (None, Some(path)) => std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
        _ => return Err("new needs --text TEXT or --text-file FILE".into()),
    };
    let files = attachments(a)?;
    let security = a.has("--security");
    let form = Request {
        client_id: improve_proto::hex(&mindhub::util::random::<12>()),
        kind: Kind::from_name(a.value("--kind").unwrap_or("fault")).ok_or("a kind is fault, improvement or capability")?,
        title: a.value("--title").ok_or("new needs --title")?.to_string(),
        text: text.replace("\r\n", "\n"),
        visibility: if a.has("--private") || security { Visibility::Private } else { Visibility::Public },
        security, named: a.has("--named"), attachments: files.iter().map(|(e, _, _)| e.clone()).collect(),
    };
    let what = format!("a {} request, {}{}{}: {}\n{}", form.kind.name(), form.visibility.name(), if security { ", about security" } else { "" },
                       if form.named { ", your name on the board" } else { "" }, form.title, form.text.trim_end());
    if !confirm(env, a, &what, &files)? { let _ = writeln!(env.out, "not sent"); return Ok(()); }
    let mut c = connect(&env.dir)?;
    let v = expect(c.call("POST", "/hub/v1/requests", wire::encode_request(&form).as_bytes()).map_err(io)?, &[200, 201])?;
    let id = n(&v, "id");
    put_files(&mut c, &format!("/hub/v1/requests/{id}"), &files)?;
    let _ = writeln!(env.out, "request {id}: {}", s(&v, "state"));
    Ok(())
}

fn show(id: u64, env: &mut Env) -> Result<(), String> {
    let mut c = connect(&env.dir)?;
    let v = expect(c.call("GET", &format!("/hub/v1/requests/{id}"), b"").map_err(io)?, &[200])?;
    let r = v.get("request").unwrap_or(&Value::Null);
    let task = match (r.get("track").and_then(Value::as_str), r.get("task").and_then(Value::as_str)) { (Some(t), Some(k)) => format!(", {t} {k}"), _ => String::new() };
    let release = r.get("release").and_then(Value::as_str).map(|x| format!(", release {x}")).unwrap_or_default();
    let _ = writeln!(env.out, "#{} {}\n{} {}, {} since {}{task}{release}\n\n{}\n", n(r, "id"), s(r, "title"), s(r, "kind"), s(r, "visibility"), s(r, "state"),
                     when(n(r, "since_ms")), s(r, "text").trim_end());
    if !list(r, "flagged").is_empty() && r.get("cleared").and_then(Value::as_bool) != Some(true) {
        let _ = writeln!(env.out, "it waits for the admin: it seems to ask for a credential or authority\n");
    }
    for f in list(r, "attachments") {
        let _ = writeln!(env.out, "attachment: {} {} ({} bytes){}", s(f, "kind"), s(f, "name"), n(f, "size"), if f.get("received").and_then(Value::as_bool) == Some(true) { "" } else { ", not received" });
    }
    for p in list(r, "thread") {
        let extra = match s(p, "kind") {
            "answer" => format!(" to #{}", n(p, "answers")),
            "verdict" => format!(": {} on {}", s(p, "verdict"), s(p, "release")),
            "question" if p.get("answered").and_then(Value::as_bool) == Some(false) => " (waits for your answer: improve reply N TEXT --answer #)".into(),
            _ => String::new(),
        };
        let _ = writeln!(env.out, "#{} {}{extra}, {}, {}\n  {}", n(p, "n"), s(p, "kind"), s(p, "author"), when(n(p, "at_ms")), s(p, "text").trim_end().replace('\n', "\n  "));
        for f in list(p, "attachments") { let _ = writeln!(env.out, "  attachment: {} {} ({} bytes)", s(f, "kind"), s(f, "name"), n(f, "size")); }
    }
    Ok(())
}

/// A message, its attachments shown first and their bytes sent after it.
fn send(a: &Args, env: &mut Env, id: u64, message: Message, what: &str) -> Result<(), String> {
    let files = attachments(a)?;
    let message = Message { attachments: files.iter().map(|(e, _, _)| e.clone()).collect(), ..message };
    message.check().map_err(|e| format!("the message is out of its bounds: {e:?}"))?;
    if !files.is_empty() && !confirm(env, a, what, &files)? { let _ = writeln!(env.out, "not sent"); return Ok(()); }
    let mut c = connect(&env.dir)?;
    let v = expect(c.call("POST", &format!("/hub/v1/requests/{id}/messages"), wire::encode_message(&message).as_bytes()).map_err(io)?, &[201])?;
    let m = n(&v, "n");
    put_files(&mut c, &format!("/hub/v1/requests/{id}/messages/{m}"), &files)?;
    let _ = writeln!(env.out, "message #{m} on request {id}; the request is {}", s(&v, "state"));
    Ok(())
}

fn reply(a: &Args, env: &mut Env) -> Result<(), String> {
    let id = number(a.word(1, "the request's number")?)?;
    let text = a.word(2, "the text")?.to_string();
    let message = match a.value("--answer") {
        Some(q) => Message { kind: MessageKind::Answer, text, answers: Some(q.trim_start_matches('#').parse().map_err(|_| "--answer takes the question's number")?), verdict: None, release: None, attachments: Vec::new() },
        None => Message { kind: MessageKind::Comment, text, answers: None, verdict: None, release: None, attachments: Vec::new() },
    };
    let what = format!("a reply on request {id}");
    send(a, env, id, message, &what)
}

fn check(a: &Args, env: &mut Env) -> Result<(), String> {
    let id = number(a.word(1, "the request's number")?)?;
    let verdict = Verdict::from_name(a.word(2, "the verdict")?).ok_or("a verdict is works, partly or does-not-work")?;
    let release = a.value("--release").ok_or("check needs --release, the release you checked on")?.to_string();
    let message = Message { kind: MessageKind::Verdict, text: a.word(3, "what you saw")?.to_string(), answers: None, verdict: Some(verdict), release: Some(release), attachments: Vec::new() };
    let what = format!("your verdict on request {id}: {}", verdict.name());
    send(a, env, id, message, &what)
}

/// News since the last look: requests that changed, and a question that waits. The last look is kept in `seen`.
fn watch(a: &Args, env: &mut Env) -> Result<(), String> {
    let seen_path = env.dir.join("seen");
    let mut since: i64 = std::fs::read_to_string(&seen_path).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
    let mut wait = 0u64;
    loop {
        let result = (|| -> Result<u64, String> {
            let mut c = connect(&env.dir)?;
            let me = expect(c.call("GET", "/hub/v1/devices/me", b"").map_err(io)?, &[200])?;
            let v = expect(c.call("GET", &format!("/hub/v1/requests?since={}", since + 1), b"").map_err(io)?, &[200])?;
            let changed = list(&v, "requests");
            for r in changed {
                let question = if r.get("open_question").and_then(Value::as_bool) == Some(true) { ", a question waits for you" } else { "" };
                let _ = writeln!(env.out, "request {}: {} since {}{question} (improve show {})", n(r, "id"), s(r, "state"), when(n(r, "since_ms")), n(r, "id"));
                since = since.max(n(r, "since_ms"));
            }
            if changed.is_empty() && a.has("--once") { let _ = writeln!(env.out, "no news"); }
            let _ = std::fs::write(&seen_path, since.to_string());
            Ok(n(&me, "poll_s").max(10) as u64)
        })();
        let poll = match result {
            Ok(p) => { wait = 0; p }
            Err(why) if a.has("--once") => return Err(why),
            Err(why) => {
                // The hub away: polled again later and later, up to 15 minutes.
                wait = (wait * 2).clamp(30, 900);
                let _ = writeln!(env.out, "improve: {why}; again in {}", duration(wait));
                wait
            }
        };
        if a.has("--once") { return Ok(()); }
        std::thread::sleep(Duration::from_secs(poll));
    }
}
