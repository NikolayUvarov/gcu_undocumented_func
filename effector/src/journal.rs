//! The session journal's file form (750-EFF-0006): one line per saved result, appended and flushed before the result
//! is acknowledged, and a `done` line once it needs no more ACK, so a restart does not acknowledge it again. A last
//! line cut by a crash is dropped (its result was never acknowledged, or its ACK is sent once more); anything else
//! malformed refuses the journal, so a result is never acknowledged twice differently or forgotten silently. A full
//! journal is written again without its oldest done records (`session::Store::rewrite`).
use crate::session::{Artifact, Error, Failure, Final, Id, Record, RECORDS_MAX};

pub const HEADER: &str = "effector-journal 1\n";
/// `<id> <identity> <result> <expires>\n`: id ≤ 64, identity 64 hex, result `denied`, `expired`, `completed`,
/// `captured-<artifact>` or `failed-<name>` (`session::Failure`, ≤ 18), expires ≤ 20 or `-`. `<id> <identity> done -`
/// follows a record whose answer needs no more ACK.
pub const LINE_MAX: usize = 64 + 1 + 64 + 1 + 18 + 1 + 20 + 1;

fn hex(bytes: &[u8; 32], out: &mut [u8]) {
    for (i, b) in bytes.iter().enumerate() {
        out[2 * i] = b"0123456789abcdef"[(b >> 4) as usize];
        out[2 * i + 1] = b"0123456789abcdef"[(b & 15) as usize];
    }
}

/// One record's line.
pub fn line(r: &Record, out: &mut [u8; LINE_MAX]) -> usize {
    let mut n = 0;
    let mut put = |b: &[u8], n: &mut usize| { out[*n..*n + b.len()].copy_from_slice(b); *n += b.len(); };
    put(r.id.as_str().as_bytes(), &mut n);
    put(b" ", &mut n);
    let mut h = [0u8; 64];
    hex(&r.identity, &mut h);
    put(&h, &mut n);
    put(b" ", &mut n);
    match r.result {
        Final::Failed(f) => { put(b"failed-", &mut n); put(f.name().as_bytes(), &mut n); }
        Final::Captured(a) => { put(b"captured-", &mut n); put(a.name().as_bytes(), &mut n); }
        other => put(other.status().as_bytes(), &mut n),
    }
    put(b" ", &mut n);
    match r.expires_unix {
        None => put(b"-", &mut n),
        Some(mut v) => {
            let mut d = [0u8; 20];
            let mut i = d.len();
            loop { i -= 1; d[i] = b'0' + (v % 10) as u8; v /= 10; if v == 0 { break; } }
            put(&d[i..], &mut n);
        }
    }
    put(b"\n", &mut n);
    n
}

/// The line that says record `r`'s answer needs no more ACK.
pub fn done_line(r: &Record, out: &mut [u8; LINE_MAX]) -> usize {
    let id = r.id.as_str().as_bytes();
    out[..id.len()].copy_from_slice(id);
    out[id.len()] = b' ';
    hex(&r.identity, &mut out[id.len() + 1..id.len() + 65]);
    out[id.len() + 65..id.len() + 73].copy_from_slice(b" done -\n");
    id.len() + 73
}

/// The whole journal: the header, then each record oldest first, with its `done` line if it needs no more ACK.
pub fn write_all(entries: impl Iterator<Item = (Record, bool)>, sink: &mut dyn FnMut(&[u8]) -> bool) -> bool {
    if !sink(HEADER.as_bytes()) { return false; }
    let mut line = [0u8; LINE_MAX];
    for (record, done) in entries {
        let n = self::line(&record, &mut line);
        if !sink(&line[..n]) { return false; }
        if done { let n = done_line(&record, &mut line); if !sink(&line[..n]) { return false; } }
    }
    true
}

fn unhex(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 { return None; }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let d = |c: u8| match c { b'0'..=b'9' => Some(c - b'0'), b'a'..=b'f' => Some(c - b'a' + 10), _ => None };
        out[i] = d(b[2 * i])? << 4 | d(b[2 * i + 1])?;
    }
    Some(out)
}

/// Reads a journal written for `agent`; an empty file is an empty journal. Returns the records, how many, and which
/// need no more ACK.
pub fn read(text: &[u8], agent: &str) -> Result<([Option<Record>; RECORDS_MAX], usize, [bool; RECORDS_MAX]), Error> {
    let agent = Id::new(agent)?;
    let mut out = [None; RECORDS_MAX];
    let mut done = [false; RECORDS_MAX];
    if text.is_empty() { return Ok((out, 0, done)); }
    let text = core::str::from_utf8(text).map_err(|_| Error::Journal)?;
    let body = text.strip_prefix(HEADER).ok_or(Error::Journal)?;
    let mut n = 0;
    let mut rest = body;
    while !rest.is_empty() {
        let Some(end) = rest.find('\n') else { break }; // the last line, cut by a crash: never acknowledged
        let l = &rest[..end];
        rest = &rest[end + 1..];
        let mut w = l.split(' ');
        let (id, identity, result, expires) = (w.next(), w.next(), w.next(), w.next());
        if w.next().is_some() { return Err(Error::Journal); }
        let id = Id::command(id.ok_or(Error::Journal)?).map_err(|_| Error::Journal)?;
        let identity = unhex(identity.ok_or(Error::Journal)?).ok_or(Error::Journal)?;
        if result == Some("done") {
            // Only after its own record, with the same identity; again after a redelivered answer's second ACK.
            let at = out[..n].iter().position(|r| r.is_some_and(|r| r.id == id)).ok_or(Error::Journal)?;
            if out[at].unwrap().identity != identity || expires != Some("-") { return Err(Error::Journal); }
            done[at] = true;
            continue;
        }
        let result = match result {
            Some("denied") => Final::Denied, Some("expired") => Final::Expired, Some("completed") => Final::Completed,
            Some(w) if w.starts_with("captured-") => Final::Captured(Artifact::from_name(&w[9..]).ok_or(Error::Journal)?),
            Some(w) => Final::Failed(w.strip_prefix("failed-").and_then(Failure::from_name).ok_or(Error::Journal)?),
            None => return Err(Error::Journal),
        };
        let expires_unix = match expires.ok_or(Error::Journal)? {
            "-" => None,
            e if !e.is_empty() && e.len() <= 20 && e.bytes().all(|c| c.is_ascii_digit()) && (e == "0" || !e.starts_with('0')) => Some(e.parse().map_err(|_| Error::Journal)?),
            _ => return Err(Error::Journal),
        };
        if out[..n].iter().flatten().any(|r: &Record| r.id == id) { return Err(Error::Journal); }
        if n == RECORDS_MAX { return Err(Error::Full); }
        out[n] = Some(Record { agent, id, identity, result, expires_unix });
        n += 1;
    }
    Ok((out, n, done))
}
