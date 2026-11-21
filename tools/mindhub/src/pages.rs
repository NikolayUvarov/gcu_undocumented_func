//! The hub's pages (docs/hub/README.md, 4.4, 6.4, 10.2): served as HTML that works without scripts, so a phone that is
//! not a MIND machine, or any computer, works through its browser. A user signs in with a pairing code from one of their
//! devices, the admin with a code made by an admin key (`mindhub admin-code`). Each form carries its session's token.

use crate::api::{self, Context, Shared};
use crate::event::{Decision, Owner};
use crate::http::{Request, Response};
use crate::hub::{self, refuse, Actor, DeviceState, Extra, Item, Refusal};
use crate::util::{self, field, hex, html, sha256};
use improve_proto::form::{Kind, Message, MessageKind, Request as Form, Verdict, Visibility};
use improve_proto::state::{State, STATES};
use std::time::{Duration, UNIX_EPOCH};

const USER_COOKIE: &str = "hub_session";
const ADMIN_COOKIE: &str = "hub_admin";

fn when(ms: u64) -> String { httpd::date::rfc3339(UNIX_EPOCH + Duration::from_millis(ms)).replace('T', " ")[..16].to_string() + " UTC" }

fn page(title: &str, body: &str) -> Response {
    Response::html(200, format!(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{t} · MIND Hub</title><style>
:root{{--bg:#f6f5f1;--fg:#1d1f22;--muted:#5d6168;--line:#d9d6cc;--card:#fffdf8;--accent:#1f5f8b;--bad:#9b2c2c}}
@media (prefers-color-scheme: dark){{:root{{--bg:#16181b;--fg:#e6e4de;--muted:#a3a7ad;--line:#33373d;--card:#1e2125;--accent:#7fb3d9;--bad:#e08a8a;color-scheme:dark}}}}
body{{background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif;margin:0;padding:1rem 16px 3rem}}
main{{max-width:52rem;margin:0 auto}} h1{{font-size:1.4rem;margin:.2rem 0 1rem}} h2{{font-size:1.1rem;margin:1.6rem 0 .5rem}}
a{{color:var(--accent)}} nav{{display:flex;flex-wrap:wrap;gap:.4rem 1rem;margin-bottom:1rem;color:var(--muted)}}
.card{{background:var(--card);border:1px solid var(--line);border-radius:6px;padding:.7rem .9rem;margin:.5rem 0}}
.meta{{color:var(--muted);font-size:.85rem}} .bad{{color:var(--bad)}} pre{{white-space:pre-wrap;word-break:break-word;margin:.3rem 0;font:inherit}}
form{{display:grid;gap:.4rem;margin:.6rem 0}} input,select,textarea,button{{font:inherit;padding:.35rem .5rem;max-width:100%;box-sizing:border-box}}
textarea{{min-height:6rem}} button{{justify-self:start;cursor:pointer}} .row{{display:flex;flex-wrap:wrap;gap:.5rem;align-items:center}}
.scroll{{overflow-x:auto}} table{{border-collapse:collapse;width:100%}} td,th{{border-bottom:1px solid var(--line);padding:.3rem .4rem;text-align:left;vertical-align:top}}
code{{font-size:.9em}}
</style></head><body><main>{body}</main></body></html>"#, t = html(title)))
}

fn error_page(r: &Refusal, back: &str) -> Response {
    let mut resp = page("Refused", &format!("<h1>Refused</h1><p class=bad>{} ({})</p><p><a href=\"{}\">Back</a></p>", html(&r.detail), html(&r.limit), html(back)));
    resp.status = r.status;
    resp
}

fn token_of(req: &Request, cookie: &str) -> Option<[u8; 32]> { req.cookie(cookie).and_then(improve_proto::parse_hex32) }

/// The token a session's forms carry, which a page elsewhere cannot know.
fn form_token(token: &[u8; 32]) -> String { let mut b = b"MIND Hub form\0".to_vec(); b.extend_from_slice(token); hex(&sha256(&b)) }

fn hidden(token: &[u8; 32]) -> String { format!("<input type=hidden name=t value=\"{}\">", form_token(token)) }

fn form_fields(req: &Request, token: &[u8; 32]) -> Result<Vec<(String, String)>, Refusal> {
    let text = std::str::from_utf8(&req.body).map_err(|_| refuse(400, "form", "the form is not UTF-8"))?;
    let f = util::fields(text).ok_or_else(|| refuse(400, "form", "the form is not one of the hub's"))?;
    if field(&f, "t") != Some(form_token(token).as_str()) { return Err(refuse(403, "form", "the form is not this session's: open the page again")); }
    Ok(f)
}

fn cookie(name: &str, token: &[u8; 32], path: &str, ms: u64) -> String {
    format!("{name}={}; Path={path}; Max-Age={}; Secure; HttpOnly; SameSite=Strict", hex(token), ms / 1000)
}
fn clear(name: &str, path: &str) -> String { format!("{name}=; Path={path}; Max-Age=0; Secure; HttpOnly; SameSite=Strict") }

fn number(text: &str) -> Option<u64> { (!text.is_empty() && text.len() <= 19 && text.bytes().all(|b| b.is_ascii_digit())).then(|| text.parse().ok()).flatten() }

pub fn handle(s: &mut Shared, req: &Request, cx: &Context) -> Response {
    let path = req.path.trim_end_matches('/');
    if path == "/hub/board" && req.method == "GET" { return board(s); }
    if path == "/hub/admin" || path.starts_with("/hub/admin/") { return admin(s, req, cx, path.strip_prefix("/hub/admin").unwrap_or("")); }
    if path == "/hub" || path.starts_with("/hub/") { return user(s, req, cx, path.strip_prefix("/hub").unwrap_or("")); }
    if path.is_empty() && req.method == "GET" { return Response::see("/hub/"); }
    api::refusal(&refuse(404, "route", "no such page"))
}

fn board(s: &Shared) -> Response {
    let mut rows = String::new();
    for e in s.hub.board() {
        rows += &format!("<tr><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                         e.id, html(&e.title), e.kind.name(), html(&e.task), e.state.name(), when(e.since_ms), html(e.release.as_deref().unwrap_or("")),
                         html(e.author.as_deref().unwrap_or("")));
    }
    if rows.is_empty() { rows = "<tr><td colspan=8>Nothing has become a task yet.</td></tr>".into(); }
    page("Board", &format!("<nav><a href=\"/hub/\">My requests</a><a href=\"/hub/v1/board\">JSON</a></nav><h1>The board</h1>\
<p class=meta>Requests that became tasks. Private ones and those about security are never here, and a user is named only when they allowed it.</p>\
<div class=scroll><table><tr><th>#</th><th>Title</th><th>Kind</th><th>Task</th><th>State</th><th>Since</th><th>Release</th><th>From</th></tr>{rows}</table></div>"))
}

// ---- the user's pages ----

fn user(s: &mut Shared, req: &Request, cx: &Context, rest: &str) -> Response {
    let now = cx.now;
    let seg: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
    if req.method == "POST" && seg.as_slice() == ["sign-in"] { return sign_in(s, req, cx, false); }
    let Some(token) = token_of(req, USER_COOKIE) else { return sign_in_page(false, None) };
    let Some(Owner::User { device, .. }) = s.hub.session(&token, now).map(|x| x.owner.clone()) else { return sign_in_page(false, None).with("Set-Cookie", clear(USER_COOKIE, "/hub/")) };
    let result = match (req.method.as_str(), seg.as_slice()) {
        ("GET", []) => Ok(home(s, &token, &device, now)),
        ("GET", ["requests", id]) => number(id).and_then(|id| s.hub.request(id)).filter(|r| !r.deleted && s.hub.device(&device).is_some_and(|d| d.user == r.user)).cloned()
            .map(|r| thread_page(&token, &r, false)).ok_or_else(|| refuse(404, "request", "no such request of yours")),
        ("POST", ["sign-out"]) => form_fields(req, &token).and_then(|_| s.commit(s.hub.sign_out(now, &token)))
            .map(|_| Response::see("/hub/").with("Set-Cookie", clear(USER_COOKIE, "/hub/"))),
        ("POST", ["requests"]) => form_fields(req, &token).and_then(|f| file_form(s, now, &device, &f)),
        // The user's data as JSON, a download (design 9).
        ("GET", ["export"]) => s.hub.device(&device).map(|d| d.user).ok_or_else(|| refuse(404, "user", "no such user"))
            .map(|user| Response::json(200, api::export(&s.hub, user, now)).with("Content-Disposition", "attachment; filename=\"mind-hub-export.json\"".into())),
        ("POST", ["requests", id, "delete"]) => {
            let id = number(id).unwrap_or(0);
            form_fields(req, &token).and_then(|_| { let events = s.hub.delete_request(now, &device, id)?; s.commit(events) }).map(|_| Response::see("/hub/"))
        }
        ("POST", ["erase"]) => form_fields(req, &token).and_then(|f| {
            if field(&f, "sure") != Some("yes") { return Err(refuse(400, "sure", "tick the box that says you are sure")); }
            let events = s.hub.erase(now, &device)?;
            s.commit(events)
        }).map(|_| page("Deleted", "<h1>Your account is deleted</h1><p>Your requests that no track took are gone, your devices are revoked, and your name and contact are erased.</p>")
            .with("Set-Cookie", clear(USER_COOKIE, "/hub/"))),
        ("POST", ["requests", id, "messages"]) => {
            let id = number(id).unwrap_or(0);
            form_fields(req, &token).and_then(|f| message_form(&f, false)).and_then(|m| {
                let events = s.hub.post(now, &Actor::Device(device), id, &m)?;
                s.commit(events)?;
                Ok(Response::see(&format!("/hub/requests/{id}")))
            })
        }
        _ => Err(refuse(404, "route", "no such page")),
    };
    result.unwrap_or_else(|r| error_page(&r, "/hub/"))
}

fn sign_in_page(admin: bool, error: Option<&str>) -> Response {
    let (title, action, how) = if admin {
        ("Admin sign-in", "/hub/admin/sign-in", "Make a code with your admin key: <code>mindhub admin-code --key ADMIN.key --hub URL</code>.")
    } else {
        ("Sign in", "/hub/sign-in", "Make a code on one of your approved devices: <code>improve pair</code>. It lasts 10 minutes and works once.")
    };
    let refused = error.is_some();
    let error = error.map(|e| format!("<p class=bad>{}</p>", html(e))).unwrap_or_default();
    let mut resp = page(title, &format!("<h1>{title}</h1>{error}<p>{how}</p><form method=post action=\"{action}\">\
<label>Code <input name=code autocomplete=one-time-code autocapitalize=characters maxlength=8 size=10 required></label><button>Sign in</button></form>\
<p><a href=\"/hub/board\">The board</a></p>"));
    if refused { resp.status = 403; }
    resp
}

fn sign_in(s: &mut Shared, req: &Request, cx: &Context, admin: bool) -> Response {
    let f = std::str::from_utf8(&req.body).ok().and_then(util::fields).unwrap_or_default();
    let code = field(&f, "code").unwrap_or("");
    let token = util::random::<32>();
    match s.hub.open_session(cx.now, code, admin, &token, &cx.network) {
        Ok(events) => match s.commit(events) {
            Ok(()) => {
                let (name, path, ms, to) = if admin { (ADMIN_COOKIE, "/hub/admin/", hub::ADMIN_SESSION_MS, "/hub/admin/") } else { (USER_COOKIE, "/hub/", hub::SESSION_MS, "/hub/") };
                Response::see(to).with("Set-Cookie", cookie(name, &token, path, ms))
            }
            Err(r) => error_page(&r, "/hub/"),
        },
        Err(r) => {
            if r.limit == "code" { s.hub.note_failure(&cx.network, cx.now); }
            sign_in_page(admin, Some(&r.detail))
        }
    }
}

fn home(s: &Shared, token: &[u8; 32], device: &[u8; 32], now: u64) -> Response {
    let d = s.hub.device(device).unwrap();
    let u = s.hub.user(d.user).unwrap();
    let (requests, messages) = s.hub.quotas(device, now);
    let band = u.trust.band().map(|b| format!("band {b}")).unwrap_or_else(|| "read-only".into());
    let mut list = String::new();
    for r in s.hub.requests().iter().filter(|r| r.user == u.id && !r.deleted).rev() {
        let q = if r.thread.iter().any(|p| p.message.kind == MessageKind::Question && !p.answered) { " <strong>· a question for you</strong>" }
            else if r.waits_for_admin() { " · waits for the admin" } else { "" };
        list += &format!("<div class=card><a href=\"/hub/requests/{id}\">#{id} {t}</a>{q}<div class=meta>{st} since {since}</div></div>",
                         id = r.id, t = html(&r.form.title), st = r.state.name(), since = when(r.since));
    }
    if list.is_empty() { list = "<p class=meta>No requests yet.</p>".into(); }
    let kinds: String = Kind::ALL.iter().map(|k| format!("<option>{}</option>", k.name())).collect();
    let client_id = hex(&util::random::<12>());
    let new = if requests > 0 { format!("<h2>A new request</h2><form method=post action=\"/hub/requests\">{h}<input type=hidden name=client_id value=\"{client_id}\">\
<label>Kind <select name=kind>{kinds}</select></label><label>Title <input name=title maxlength=120 required></label>\
<label>What happened, or what you want <textarea name=text maxlength=8192 required></textarea></label>\
<div class=row><label><input type=radio name=visibility value=public checked> public: may be on the board, without your name</label>\
<label><input type=radio name=visibility value=private> private</label></div>\
<label><input type=checkbox name=security value=yes> about a vulnerability (private always, never on the board)</label>\
<label><input type=checkbox name=named value=yes> show my name with it on the board, once it is a task</label>\
<button>Send</button></form><p class=meta>Attachments are sent from a device's <code>improve</code>.</p>", h = hidden(token))
    } else { "<p class=meta>No new request can be sent now: see your limits above.</p>".into() };
    page("My requests", &format!("<nav><a href=\"/hub/board\">The board</a><form method=post action=\"/hub/sign-out\" style=\"display:inline;margin:0\">{h}<button>Sign out</button></form></nav>\
<h1>{name}'s requests</h1><p class=meta>Signed in through {dev} · trust {trust} ({band}) · {requests} new requests and {messages} messages left now</p>{list}{new}\
<h2>Your data</h2><p><a href=\"/hub/export\">Export it</a>: all the hub keeps of you, as JSON.</p>\
<form method=post action=\"/hub/erase\">{h}<label><input type=checkbox name=sure value=yes required> I am sure</label>\
<button>Delete my account</button><p class=meta>Your requests that no track took are deleted, every device of yours is revoked, and your name and contact are erased. Requests that became tasks stay, with no name.</p></form>",
        h = hidden(token), name = html(&u.display_name), dev = html(&d.name), trust = u.trust.value()))
}

fn file_form(s: &mut Shared, now: u64, device: &[u8; 32], f: &[(String, String)]) -> Result<Response, Refusal> {
    let get = |k: &str| field(f, k).unwrap_or("");
    let form = Form {
        client_id: get("client_id").into(),
        kind: Kind::from_name(get("kind")).ok_or_else(|| refuse(400, "kind", "kind is one of fault, improvement, capability"))?,
        title: get("title").trim().into(),
        text: util::lines(get("text")),
        visibility: if get("visibility") == "private" { Visibility::Private } else { Visibility::Public },
        security: get("security") == "yes",
        named: get("named") == "yes",
        attachments: Vec::new(),
    };
    let form = Form { visibility: if form.security { Visibility::Private } else { form.visibility }, ..form };
    match s.hub.file(now, device, &form) {
        Ok((events, id, _)) => { s.commit(events)?; Ok(Response::see(&format!("/hub/requests/{id}"))) }
        Err(r) if r.status == 409 => Err(Refusal { detail: format!("{} — see request {}", r.detail, r.earlier.unwrap_or(0)), ..r }),
        Err(r) => Err(r),
    }
}

/// A message from a form: a comment, an answer to a question, a verdict; or, for the admin, a comment or a question.
fn message_form(f: &[(String, String)], admin: bool) -> Result<Message, Refusal> {
    let get = |k: &str| field(f, k).unwrap_or("");
    let kind = MessageKind::from_name(get("kind")).ok_or_else(|| refuse(400, "kind", "no such kind of message"))?;
    let allowed = if admin { matches!(kind, MessageKind::Comment | MessageKind::Question) } else { matches!(kind, MessageKind::Comment | MessageKind::Answer | MessageKind::Verdict) };
    if !allowed { return Err(refuse(403, "kind", format!("no {} from this page", kind.name()))); }
    let answers = (kind == MessageKind::Answer).then(|| number(get("answers")).and_then(|n| u32::try_from(n).ok())).flatten();
    let verdict = (kind == MessageKind::Verdict).then(|| Verdict::from_name(get("verdict"))).flatten();
    let release = (kind == MessageKind::Verdict).then(|| get("release").trim().to_string()).filter(|r| !r.is_empty());
    let m = Message { kind, text: util::lines(get("text")), answers, verdict, release, attachments: Vec::new() };
    m.check().map_err(|e| refuse(400, "message", format!("the message is not complete: {e:?}")))?;
    Ok(m)
}

fn thread_html(r: &Item, admin: bool) -> String {
    let mut out = String::new();
    for p in &r.thread {
        let mut files = String::new();
        for (k, (a, got)) in p.message.attachments.iter().zip(&p.received).enumerate() {
            let link = match (*got, admin) {
                (true, true) => format!(" · <a href=\"/hub/admin/requests/{}/messages/{}/attachments/{k}\">download</a>", r.id, p.n),
                (false, _) => " · not received".into(),
                _ => String::new(),
            };
            files += &format!("<div class=meta>{} · {} · {} bytes{link}</div>", a.kind.name(), html(&a.name), a.size);
        }
        let extra = match p.message.kind {
            MessageKind::Answer => format!(" to #{}", p.message.answers.unwrap_or(0)),
            MessageKind::Verdict => format!(": {} on {}", p.message.verdict.map(Verdict::name).unwrap_or(""), html(p.message.release.as_deref().unwrap_or(""))),
            MessageKind::Question if !p.answered => " · not answered yet".into(),
            _ => String::new(),
        };
        out += &format!("<div class=card id=m{n}><div class=meta>#{n} · {k}{extra} · {a} · {t}</div><pre>{x}</pre>{files}</div>",
                        n = p.n, k = p.message.kind.name(), a = html(&p.author.text()), t = when(p.at), x = html(&p.message.text));
    }
    out
}

fn thread_page(token: &[u8; 32], r: &Item, admin: bool) -> Response {
    let base = if admin { "/hub/admin" } else { "/hub" };
    let h = hidden(token);
    let task = match (&r.track, &r.task) { (Some(t), Some(k)) => format!(" · {} <code>{}</code>", html(t), html(k)), _ => String::new() };
    let release = r.release.as_deref().map(|v| format!(" · release {}", html(v))).unwrap_or_default();
    let mut forms = String::new();
    if admin {
        let states: String = STATES.iter().filter(|st| improve_proto::state::allowed(r.state, **st, improve_proto::state::Role::Admin))
            .map(|st| format!("<option>{}</option>", st.name())).collect();
        if !states.is_empty() {
            forms += &format!("<h2>Move it</h2><form method=post action=\"{base}/requests/{id}/state\">{h}<label>To <select name=to>{states}</select></label>\
<label>Reason <input name=reason maxlength=200 required></label><div class=row><label>Track <input name=track maxlength=3 size=4></label>\
<label>Task <input name=task maxlength=24 size=18></label><label>Duplicate of <input name=duplicate_of size=6></label></div>\
<label><input type=checkbox name=not_a_request value=yes> not a request (trust -16)</label><label><input type=checkbox name=spam value=yes> spam (trust -256)</label>\
<button>Move</button></form>", id = r.id);
        }
        if r.waits_for_admin() {
            forms += &format!("<h2>Flagged: {why}</h2><p class=meta>It seems to ask for a credential, a key or authority, so no agent reads it until you clear it.</p>\
<form method=post action=\"{base}/requests/{id}/clear\">{h}<label>Reason <input name=reason maxlength=200 required></label><button>Clear the flag</button></form>",
                id = r.id, why = r.flags.join(", "));
        }
        forms += &format!("<h2>Write</h2><form method=post action=\"{base}/requests/{id}/messages\">{h}<label>Kind <select name=kind><option>comment</option><option>question</option></select></label>\
<label>Text <textarea name=text maxlength=8192 required></textarea></label><button>Post</button></form>", id = r.id);
        if !r.form.attachments.is_empty() {
            forms += "<h2>Attachments</h2>";
            for (n, (a, got)) in r.form.attachments.iter().zip(&r.received).enumerate() {
                let link = if *got { format!("<a href=\"/hub/admin/requests/{}/attachments/{n}\">download</a>", r.id) } else { "not received".into() };
                forms += &format!("<div class=card>{} · {} · {} bytes · {link}</div>", a.kind.name(), html(&a.name), a.size);
            }
        }
    } else if !r.state.closed() || r.state == State::ClosedUnchecked {
        let open: Vec<u32> = r.thread.iter().filter(|p| p.message.kind == MessageKind::Question && !p.answered).map(|p| p.n).collect();
        for q in &open {
            forms += &format!("<h2>Answer question #{q}</h2><form method=post action=\"{base}/requests/{id}/messages\">{h}<input type=hidden name=kind value=answer>\
<input type=hidden name=answers value={q}><textarea name=text maxlength=8192 required></textarea><button>Answer</button></form>", id = r.id);
        }
        if matches!(r.state, State::AwaitingCheck | State::ClosedUnchecked) {
            let works = if r.state == State::AwaitingCheck { "<option value=works>works</option>" } else { "" };
            forms += &format!("<h2>Your check</h2><p class=meta>Follow the result's steps on your machine, then say what you saw.</p>\
<form method=post action=\"{base}/requests/{id}/messages\">{h}<input type=hidden name=kind value=verdict><label>Verdict <select name=verdict>{works}<option value=partly>partly</option>\
<option value=does-not-work>does not work</option></select></label><label>Checked on release <input name=release maxlength=32 value=\"{rel}\" required></label>\
<label>What you saw <textarea name=text maxlength=8192 required></textarea></label><button>Send the verdict</button></form>", id = r.id, rel = html(r.release.as_deref().unwrap_or("")));
        }
        if r.deletable() {
            forms += &format!("<h2>Delete it</h2><form method=post action=\"{base}/requests/{id}/delete\">{h}<p class=meta>No track took it yet: it and its thread leave the hub.</p>\
<button>Delete this request</button></form>", id = r.id);
        }
        if !r.state.closed() {
            forms += &format!("<h2>Comment</h2><form method=post action=\"{base}/requests/{id}/messages\">{h}<input type=hidden name=kind value=comment>\
<textarea name=text maxlength=8192 required></textarea><button>Post</button></form>", id = r.id);
        }
    }
    let mut who = if admin { format!(" · user {} · device <code>{}</code>", r.user, &hex(&r.device)[..16]) } else { String::new() };
    if r.deleted { who += " · deleted by its user"; }
    if r.form.named { who += " · named on the board"; }
    let nav = if admin { admin_nav(token) } else { "<nav><a href=\"/hub/\">Back</a><a href=\"/hub/board\">The board</a></nav>".to_string() };
    page(&format!("Request {}", r.id), &format!("{nav}<h1>#{id} {t}</h1>\
<p class=meta>{k} · {v}{sec} · {st} since {since}{task}{release}{who}</p><div class=card><pre>{text}</pre></div><h2>The thread</h2>{thread}{forms}",
        id = r.id, t = html(&r.form.title), k = r.form.kind.name(), v = r.form.visibility.name(), sec = if r.form.security { " · security" } else { "" },
        st = r.state.name(), since = when(r.since), text = html(&r.form.text), thread = thread_html(r, admin)))
}

// ---- the admin pages ----

fn admin(s: &mut Shared, req: &Request, cx: &Context, rest: &str) -> Response {
    let now = cx.now;
    let seg: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
    if req.method == "POST" && seg.as_slice() == ["sign-in"] { return sign_in(s, req, cx, true); }
    let Some(token) = token_of(req, ADMIN_COOKIE) else { return sign_in_page(true, None) };
    let name = match s.hub.session(&token, now).map(|x| x.owner.clone()) {
        Some(Owner::Admin(name)) if s.hub.keys.admins.values().any(|n| *n == name) => name,
        _ => return sign_in_page(true, None).with("Set-Cookie", clear(ADMIN_COOKIE, "/hub/admin/")),
    };
    let back = "/hub/admin/";
    let result = (|| -> Result<Response, Refusal> {
        match (req.method.as_str(), seg.as_slice()) {
            ("GET", []) => Ok(admin_home(s, &token, &name)),
            ("GET", ["users"]) => Ok(admin_users(s, &token)),
            ("GET", ["requests"]) => Ok(admin_requests(s, &token)),
            ("GET", ["requests", id]) => {
                let r = number(id).and_then(|id| s.hub.request(id)).cloned().ok_or_else(|| refuse(404, "request", "no such request"))?;
                Ok(thread_page(&token, &r, true))
            }
            ("GET", ["requests", _, "attachments", _]) | ("GET", ["requests", _, "messages", _, "attachments", _]) => api::admin_call(s, req, now, &name, rest.trim_start_matches('/')),
            ("GET", ["settings"]) => Ok(admin_settings(s, &token)),
            ("GET", ["journal"]) => admin_journal(s, req, &token),
            ("POST", rest_seg) => {
                let f = form_fields(req, &token)?;
                let get = |k: &str| field(&f, k).unwrap_or("").to_string();
                let device = |id: &str| improve_proto::parse_hex32(id).ok_or_else(|| refuse(404, "device", "no such device"));
                let (events, to) = match rest_seg {
                    ["sign-out"] => (s.hub.sign_out(now, &token), "/hub/admin/".to_string()),
                    ["devices", id, "decision"] => {
                        let d = Decision::from_name(&get("decision")).ok_or_else(|| refuse(400, "decision", "approve, ask or reject"))?;
                        (s.hub.decide(now, &name, &device(id)?, d, &util::lines(&get("text")))?, back.into())
                    }
                    ["devices", id, "revoke"] => (s.hub.revoke(now, &Actor::Admin(name.clone()), &device(id)?)?, back.into()),
                    ["users", id, "trust"] => {
                        let to = get("trust").trim().parse::<i16>().map_err(|_| refuse(400, "trust", "a trust is from -1023 to 1023"))?;
                        (s.hub.set_trust(now, &name, number(id).unwrap_or(0), to, get("reason").trim())?, "/hub/admin/users".into())
                    }
                    ["requests", id, "state"] => {
                        let id = number(id).unwrap_or(0);
                        let to = State::from_name(&get("to")).ok_or_else(|| refuse(400, "to", "to names no state"))?;
                        let opt = |k: &str| Some(get(k).trim().to_string()).filter(|v| !v.is_empty());
                        let extra = Extra { track: opt("track"), task: opt("task"), duplicate_of: opt("duplicate_of").and_then(|v| number(&v)),
                                            not_a_request: get("not_a_request") == "yes", spam: get("spam") == "yes" };
                        (s.hub.move_request(now, &Actor::Admin(name.clone()), id, to, get("reason").trim(), &extra)?, format!("/hub/admin/requests/{id}"))
                    }
                    ["requests", id, "clear"] => {
                        let id = number(id).unwrap_or(0);
                        (s.hub.clear_flag(now, &name, id, get("reason").trim())?, format!("/hub/admin/requests/{id}"))
                    }
                    ["requests", id, "messages"] => {
                        let id = number(id).unwrap_or(0);
                        (s.hub.post(now, &Actor::Admin(name.clone()), id, &message_form(&f, true)?)?, format!("/hub/admin/requests/{id}"))
                    }
                    ["delivered"] => {
                        let ids = get("requests").split(|c: char| c == ',' || c.is_whitespace()).filter(|x| !x.is_empty())
                            .map(|x| number(x).ok_or_else(|| refuse(400, "requests", "request numbers"))).collect::<Result<Vec<_>, _>>()?;
                        (s.hub.delivered(now, &name, get("release").trim(), &ids)?, "/hub/admin/requests".into())
                    }
                    ["settings"] => (s.hub.set_setting(now, &name, &get("name"), get("value").trim())?, "/hub/admin/settings".into()),
                    _ => return Err(refuse(404, "route", "no such page")),
                };
                s.commit(events)?;
                let resp = Response::see(&to);
                Ok(if rest_seg == ["sign-out"] { resp.with("Set-Cookie", clear(ADMIN_COOKIE, "/hub/admin/")) } else { resp })
            }
            _ => Err(refuse(404, "route", "no such page")),
        }
    })();
    result.unwrap_or_else(|r| error_page(&r, back))
}

fn admin_nav(token: &[u8; 32]) -> String {
    format!("<nav><a href=\"/hub/admin/\">Devices</a><a href=\"/hub/admin/users\">Users</a><a href=\"/hub/admin/requests\">Requests</a>\
<a href=\"/hub/admin/settings\">Settings</a><a href=\"/hub/admin/journal\">Journal</a><a href=\"/hub/board\">Board</a>\
<form method=post action=\"/hub/admin/sign-out\" style=\"display:inline;margin:0\">{}<button>Sign out</button></form></nav>", hidden(token))
}

fn notes_html(d: &hub::Device) -> String {
    d.notes.iter().map(|n| format!("<div class=card><div class=meta>{} · {} · {}</div><pre>{}</pre></div>", n.kind, html(&n.from.text()), when(n.at), html(&n.text))).collect()
}

fn admin_home(s: &Shared, token: &[u8; 32], name: &str) -> Response {
    let h = hidden(token);
    let mut pending = String::new();
    let mut list: Vec<&hub::Device> = s.hub.devices().collect();
    list.sort_by_key(|d| d.at);
    for d in list.iter().filter(|d| d.state == DeviceState::Pending) {
        let u = s.hub.user(d.user).unwrap();
        let held: String = s.hub.requests().iter().filter(|r| r.device == d.id && r.state == State::Held)
            .map(|r| format!("<div class=card>held: <a href=\"/hub/admin/requests/{id}\">#{id} {t}</a></div>", id = r.id, t = html(&r.form.title))).collect();
        pending += &format!("<div class=card><strong>{dn}</strong> · {k} · {p} {v} · user {un} (#{uid}) · network <code>{net}</code> · {at}{contact}\
<div class=meta>device <code>{id}</code></div>{notes}{held}<form method=post action=\"/hub/admin/devices/{id}/decision\">{h}\
<div class=row><label><input type=radio name=decision value=approve checked> approve</label><label><input type=radio name=decision value=ask> ask</label>\
<label><input type=radio name=decision value=reject> reject</label></div><textarea name=text maxlength=8192 required placeholder=\"The welcome, the question or the reason, which the user sees\"></textarea>\
<button>Send</button></form></div>",
            dn = html(&d.name), k = d.kind.name(), p = html(&d.platform), v = html(&d.version), un = html(&u.display_name), uid = u.id, net = html(&d.network),
            at = when(d.at), contact = d.contact.as_deref().map(|c| format!(" · {}", html(c))).unwrap_or_default(), id = hex(&d.id), notes = notes_html(d));
    }
    if pending.is_empty() { pending = "<p class=meta>No device waits.</p>".into(); }
    let mut others = String::new();
    for d in list.iter().filter(|d| d.state != DeviceState::Pending).rev() {
        let revoke = if d.state == DeviceState::Approved {
            format!("<form method=post action=\"/hub/admin/devices/{}/revoke\" style=\"margin:0\">{h}<button>Revoke</button></form>", hex(&d.id))
        } else { String::new() };
        others += &format!("<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{revoke}</td></tr>",
                           html(&d.name), d.state.name(), d.user, html(&d.version), when(d.at));
    }
    page("Admin", &format!("{nav}<h1>Devices</h1><p class=meta>Signed in as {n}</p><h2>Waiting for a decision</h2>{pending}<h2>Decided</h2>\
<div class=scroll><table><tr><th>Name</th><th>State</th><th>User</th><th>Version</th><th>Registered</th><th></th></tr>{others}</table></div>", nav = admin_nav(token), n = html(name)))
}

fn admin_users(s: &Shared, token: &[u8; 32]) -> Response {
    let h = hidden(token);
    let mut out = String::new();
    for u in s.hub.users().iter().rev() {
        let history: String = u.history.iter().rev().map(|c| format!("<div class=meta>{} · {} → {} · {} · {}</div>", when(c.at), c.from, c.to, html(&c.reason), html(&c.author.text()))).collect();
        out += &format!("<div class=card><strong>#{id} {n}</strong> · trust {t} · {d} devices{history}<form method=post action=\"/hub/admin/users/{id}/trust\">{h}\
<div class=row><label>Trust <input name=trust value={t} size=6></label><label>Reason <input name=reason maxlength=200 required></label><button>Set</button></div></form></div>",
            id = u.id, n = html(&u.display_name), t = u.trust.value(), d = u.devices.len());
    }
    if out.is_empty() { out = "<p class=meta>No users yet.</p>".into(); }
    page("Users", &format!("{}<h1>Users and their trust</h1><p class=meta>-1023 bans; a negative trust is read-only; the hub moves it by itself only within -256 … 512.</p>{out}", admin_nav(token)))
}

fn admin_requests(s: &Shared, token: &[u8; 32]) -> Response {
    let mut rows = String::new();
    for r in s.hub.requests().iter().rev() {
        let st = if r.deleted { "deleted".to_string() } else if r.waits_for_admin() { format!("{} · flagged", r.state.name()) } else { r.state.name().to_string() };
        rows += &format!("<tr><td><a href=\"/hub/admin/requests/{id}\">{id}</a></td><td>{t}</td><td>{st}</td><td>{k}</td><td>{v}{sec}</td><td>{u}</td><td>{task}</td><td>{since}</td></tr>",
            id = r.id, t = html(&r.form.title), k = r.form.kind.name(), v = r.form.visibility.name(), sec = if r.form.security { ", security" } else { "" },
            u = r.user, task = html(r.task.as_deref().unwrap_or("")), since = when(r.since));
    }
    page("Requests", &format!("{}<h1>Requests</h1><p class=meta>Open a request to moderate it.</p>\
<div class=scroll><table><tr><th>#</th><th>Title</th><th>State</th><th>Kind</th><th>Visibility</th><th>User</th><th>Task</th><th>Since</th></tr>{rows}</table></div>", admin_nav(token)))
}

fn admin_settings(s: &Shared, token: &[u8; 32]) -> Response {
    let h = hidden(token);
    let mut out = String::new();
    for name in hub::Settings::NAMES {
        out += &format!("<form method=post action=\"/hub/admin/settings\">{h}<input type=hidden name=name value=\"{name}\"><div class=row><label><code>{name}</code> \
<input name=value value=\"{v}\" size=14></label><button>Set</button></div></form>", v = html(&s.hub.settings.get(name).unwrap()));
    }
    out += &format!("<h2>A release carries landed requests</h2><form method=post action=\"/hub/admin/delivered\">{h}<label>Release <input name=release maxlength=32 required></label>\
<label>Requests <input name=requests placeholder=\"12 15 16\" required></label><button>Mark delivered</button></form>");
    page("Settings", &format!("{}<h1>Settings</h1><p class=meta><code>intake</code>: all or approved-only · <code>global-cap</code>: new requests an hour below band 2 · \
<code>work-bits</code>: the registration's proof of work, 0 is off · <code>poll-s</code>: how often devices poll.</p>{out}", admin_nav(token)))
}

fn admin_journal(s: &Shared, req: &Request, token: &[u8; 32]) -> Result<Response, Refusal> {
    let from = util::fields(&req.query).and_then(|f| field(&f, "from").and_then(number)).unwrap_or_else(|| s.hub.events.saturating_sub(199).max(1));
    let lines = s.journal.lines(from, 200).map_err(|e| refuse(500, "journal", format!("the journal cannot be read: {e}")))?;
    let rows: String = lines.iter().map(|(n, l)| format!("<tr><td>{n}</td><td><code>{}</code></td></tr>", html(l))).collect();
    let earlier = from.saturating_sub(200).max(1);
    Ok(page("Journal", &format!("{}<h1>The journal</h1>\
<p class=meta>Every change, in order, from line {from}. <a href=\"/hub/admin/journal?from={earlier}\">Earlier</a></p><div class=scroll><table>{rows}</table></div>", admin_nav(token))))
}
