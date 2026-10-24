//! The page's live feed over a WebSocket (`GET /api/v1/stream`): the cable's state, the operations and the output of
//! the one the page follows, and the lease that holds the machine. It must never slow the cable's work: a feed never waits for the state's lock (a busy lock
//! skips a tick), looks at it only when the change counter moved, holds it only to copy a few fields, and writes to the
//! browser after letting it go. A browser that stops reading is dropped, and few feeds run at once.
use crate::json::{self, obj, s, Value};
use crate::ops::{State, FINAL};
use crate::ws::{self, Message};
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::{Duration, Instant};

/// The feeds served at once; another page is refused until one closes.
const FEEDS_MAX: usize = 8;
static FEEDS: AtomicUsize = AtomicUsize::new(0);
/// How often a feed looks for changes; how long a browser may keep a write waiting before it is dropped.
const TICK: Duration = Duration::from_millis(250);
const WRITE_WAIT: Duration = Duration::from_secs(2);
/// The cable's state is sent this often even when nothing changed, so the page sees the feed alive.
const BEAT: Duration = Duration::from_secs(2);
/// The traffic's totals, read with no lock, this often: the page reckons the speed from them.
const TRAFFIC_EVERY: Duration = Duration::from_secs(1);
/// The newest operations listed, and the output sent in one message.
const LISTED: usize = 200;
const CHUNK: usize = 48 * 1024;
/// How long a page has to send the key when the daemon has one.
const KEY_WAIT: Duration = Duration::from_secs(10);

/// One of the FEEDS_MAX places, given back when the feed ends.
pub struct Place(());

impl Place {
    pub fn take() -> Option<Place> {
        if FEEDS.fetch_add(1, Ordering::AcqRel) < FEEDS_MAX { return Some(Place(())); }
        FEEDS.fetch_sub(1, Ordering::AcqRel);
        None
    }
}

impl Drop for Place {
    fn drop(&mut self) { FEEDS.fetch_sub(1, Ordering::AcqRel); }
}

/// What a feed copies under the lock: small values, and at most CHUNK bytes of output.
struct Copy { link: Value, ops: Option<Vec<Value>>, output: Option<(String, u64, u64, Vec<u8>)> }

fn copy(st: &State, follow: Option<&(String, u64)>, list: bool) -> Copy {
    let (l, now) = (&st.link, crate::ops::now());
    let link = obj(vec![
        ("type", s("link")), ("port", s(&l.port)), ("online", Value::Bool(l.online)), ("name", s(&l.name)),
        ("last_seen", Value::Num(l.last_seen as f64)), ("error", s(&l.error)),
        ("queued", Value::Num(st.queued() as f64)), ("running", Value::Num(st.running() as f64)),
        ("reservation", st.lease_at(now).map_or(Value::Null, |lease| lease.json(now))),
    ]);
    let ops = list.then(|| {
        let mut queued = 0;
        let positions: Vec<Option<usize>> = st.ops.iter().map(|o| (o.status == "queued").then(|| { queued += 1; queued })).collect();
        let from = st.ops.len().saturating_sub(LISTED);
        st.ops[from..].iter().zip(&positions[from..]).map(|(o, p)| {
            let time = |t: Option<u64>| t.map_or(Value::Null, |t| Value::Num(t as f64));
            obj(vec![
                ("id", s(&o.id)), ("kind", s(&o.kind)), ("status", s(&o.status)), ("reason", s(&o.reason)), ("params", o.params.clone()),
                ("position", p.map_or(Value::Null, |p| Value::Num(p as f64))), ("created_at", time(Some(o.created))),
                ("started_at", time(o.started)), ("finished_at", time(o.finished)), ("result", o.result.clone()),
                ("output_bytes", Value::Num(o.output_bytes as f64)), ("cancelling", Value::Bool(o.cancel.is_some())),
                ("waiting", st.waiting(o, now).unwrap_or(Value::Null)),
            ])
        }).collect()
    });
    let output = follow.and_then(|(id, from)| {
        let o = st.ops.iter().find(|o| &o.id == id)?;
        let first = o.output_bytes - o.output.len() as u64;
        let from = (*from).clamp(first, o.output_bytes);
        let at = (from - first) as usize;
        let bytes = o.output[at..(at + CHUNK).min(o.output.len())].to_vec();
        Some((o.status.clone(), from, o.output_bytes, bytes))
    });
    Copy { link, ops, output }
}

/// Serves one page after the handshake, until it closes, stops reading or the daemon's key is not given.
pub fn serve(mut stream: TcpStream, state: Arc<Mutex<State>>, changes: Arc<AtomicU64>, key: Option<String>, _place: Place) {
    stream.set_read_timeout(Some(TICK)).ok();
    stream.set_write_timeout(Some(WRITE_WAIT)).ok();
    stream.set_nodelay(true).ok();
    let send = |stream: &mut TcpStream, opcode: u8, payload: &[u8]| stream.write_all(&ws::frame(opcode, payload)).is_ok();
    let say = |stream: &mut TcpStream, v: Value| send(stream, ws::TEXT, v.text().as_bytes());
    let opened = Instant::now();
    let mut keyed = key.is_none();
    if !say(&mut stream, obj(vec![("type", s("hello")), ("key_needed", Value::Bool(!keyed))])) { return; }
    let (mut inbox, mut chunk) = (Vec::new(), [0u8; 2048]);
    // The operation followed and the byte its output reached; the change counter last sent; the list last sent.
    let mut follow: Option<(String, u64)> = None;
    let (mut seen, mut listed, mut beat, mut more) = (u64::MAX, String::new(), Instant::now() - BEAT, false);
    let mut counted = Instant::now() - TRAFFIC_EVERY;
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => return,
            Ok(n) => inbox.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return,
        }
        loop {
            match ws::next(&mut inbox) {
                Ok(None) => break,
                Ok(Some(Message::Text(text))) => {
                    let Ok(m) = json::parse(&text) else { continue };
                    if !keyed {
                        if m.str("key") != key.as_deref() {
                            say(&mut stream, obj(vec![("type", s("error")), ("error", s("the key is wrong"))]));
                            send(&mut stream, ws::CLOSE, &1008u16.to_be_bytes());
                            return;
                        }
                        keyed = true;
                        seen = u64::MAX;
                    }
                    match m.get("follow") {
                        Some(Value::Str(id)) => { follow = Some((id.clone(), m.num("from").unwrap_or(0.0) as u64)); seen = u64::MAX; }
                        Some(Value::Null) => follow = None,
                        _ => {}
                    }
                }
                Ok(Some(Message::Ping(p))) => if !send(&mut stream, ws::PONG, &p) { return },
                Ok(Some(Message::Close)) => { send(&mut stream, ws::CLOSE, &1000u16.to_be_bytes()); return; }
                Ok(Some(Message::Other)) => {}
                Err(_) => { send(&mut stream, ws::CLOSE, &1002u16.to_be_bytes()); return; }
            }
        }
        if !keyed {
            if opened.elapsed() > KEY_WAIT { return; }
            continue;
        }
        if counted.elapsed() >= TRAFFIC_EVERY {
            if !say(&mut stream, crate::traffic::message()) { return; }
            counted = Instant::now();
        }
        let now = changes.load(Ordering::Acquire);
        if now == seen && !more && beat.elapsed() < BEAT { continue; }
        let c = match state.try_lock() {
            Ok(st) => copy(&st, follow.as_ref(), now != seen),
            Err(TryLockError::WouldBlock) => continue,
            Err(TryLockError::Poisoned(p)) => copy(&p.into_inner(), follow.as_ref(), now != seen),
        };
        // The lock is let go: what follows waits on the browser alone.
        (seen, beat, more) = (now, Instant::now(), false);
        if !say(&mut stream, c.link) { return; }
        if let Some(ops) = c.ops {
            let text = obj(vec![("type", s("ops")), ("ops", Value::Arr(ops))]).text();
            if text != listed {
                if !send(&mut stream, ws::TEXT, text.as_bytes()) { return; }
                listed = text;
            }
        }
        match (c.output, follow.as_mut()) {
            (Some((status, from, total, mut bytes)), Some((id, at))) => {
                // A character cut at the chunk's end waits for the next one.
                if let Err(e) = std::str::from_utf8(&bytes) { if e.error_len().is_none() && e.valid_up_to() > 0 { bytes.truncate(e.valid_up_to()); } }
                let next = from + bytes.len() as u64;
                let ended = FINAL.contains(&status.as_str()) && next == total;
                if !bytes.is_empty() || from > *at || ended {
                    let m = obj(vec![("type", s("output")), ("id", s(id)), ("status", s(&status)), ("from", Value::Num(from as f64)),
                                     ("next", Value::Num(next as f64)), ("text", s(&String::from_utf8_lossy(&bytes)))]);
                    if !say(&mut stream, m) { return; }
                }
                *at = next;
                // More is held than one message took: the next tick takes it without waiting for a change.
                more = next < total;
                if ended { follow = None; }
            }
            (None, Some((id, _))) => {
                say(&mut stream, obj(vec![("type", s("output")), ("id", s(id)), ("status", s("unknown")), ("text", s(""))]));
                follow = None;
            }
            _ => {}
        }
    }
}
