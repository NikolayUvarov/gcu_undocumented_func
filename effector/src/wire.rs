//! The agent's side of contract 1.0 (docs/effector/protocol.md): request bodies written by `json`, and the server's
//! answers read from the values the parser service gives (750-EFF-0006). A missing or mistyped field, a duplicate key
//! or too many values refuses the answer; unknown fields are skipped.
use crate::json::Writer;
use crate::session::Command;

/// The largest answer the agent reads, in values: contract 1.0's bound (docs/effector/protocol.md), which `parse` keeps too.
pub const VALUES_MAX: usize = 1024;
pub const COMMANDS_MAX: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Object, Array, String, Number, True, False, Null }

/// One value as `parse.json` answers it: kind, the index of its object or array, its key, its text.
#[derive(Clone, Copy, Debug)]
pub struct Value<'a> { pub kind: Kind, pub parent: Option<u16>, pub key: &'a str, pub text: &'a str }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Shape, Duplicate, Missing, Type, Bounds, Write }

impl From<crate::json::Error> for Error { fn from(_: crate::json::Error) -> Self { Error::Write } }

/// What the heartbeat says (contract 1.0's minimal heartbeat).
#[derive(Clone, Copy, Debug)]
pub struct Heartbeat<'a> {
    pub agent_id: &'a str,
    pub hostname: &'a str,
    pub arch: &'a str,
    pub agent_version: &'a str,
    pub stopping: bool,
    pub agent_time: Option<&'a str>,
}

pub fn arch() -> &'static str { if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" } }

pub fn heartbeat<'b>(buf: &'b mut [u8], h: &Heartbeat<'_>) -> Result<&'b [u8], Error> {
    let mut w = Writer::new(buf);
    w.object()?;
    w.field_str("agent_id", h.agent_id)?;
    w.field_str("hostname", h.hostname)?;
    w.field_str("os_version", "mindcore")?;
    w.field_str("arch", h.arch)?;
    w.field_str("run_mode", "service")?;
    w.field_str("username", "effector")?;
    w.field_str("agent_version", h.agent_version)?;
    w.key("programs")?; w.array()?; w.end()?;
    w.field_str("system_state", if h.stopping { "shutting_down" } else { "running" })?;
    w.field_bool("busy", false)?;
    w.field_u64("max_concurrent_commands", 1)?;
    w.field_u64("occupied_queues", 0)?;
    w.field_u64("free_slots", 1)?;
    w.field_bool("config_auto_update_enabled", false)?;
    w.field_str("transport_mode", "https_only")?;
    w.field_str("transport", "https")?;
    if let Some(t) = h.agent_time { w.field_str("agent_time", t)?; }
    w.end()?;
    Ok(w.finish()?)
}

/// `{"command_id": ID, "result": RESULT}`: the result is the command's JSON result as a string (contract 1.0).
pub fn ack<'b>(buf: &'b mut [u8], command_id: &str, result_json: &str) -> Result<&'b [u8], Error> {
    let mut w = Writer::new(buf);
    w.object()?;
    w.field_str("command_id", command_id)?;
    w.field_str("result", result_json)?;
    w.end()?;
    Ok(w.finish()?)
}

/// The members of object `at`: refused if a key comes twice; `wanted` keys are returned by position.
fn members<const N: usize>(values: &[Value<'_>], at: usize, wanted: [&str; N]) -> Result<[Option<usize>; N], Error> {
    if values.get(at).map(|v| v.kind) != Some(Kind::Object) { return Err(Error::Shape); }
    let mut found = [None; N];
    for (i, v) in values.iter().enumerate().skip(at + 1) {
        if v.parent != Some(at as u16) { continue; }
        if values[at + 1..i].iter().any(|o| o.parent == Some(at as u16) && o.key == v.key) { return Err(Error::Duplicate); }
        if let Some(slot) = wanted.iter().position(|w| *w == v.key) { found[slot] = Some(i); }
    }
    Ok(found)
}

fn root(values: &[Value<'_>]) -> Result<(), Error> {
    if values.len() > VALUES_MAX { return Err(Error::Bounds); }
    match values.first() { Some(v) if v.kind == Kind::Object && v.parent.is_none() => Ok(()), _ => Err(Error::Shape) }
}

fn string<'a>(values: &[Value<'a>], at: Option<usize>) -> Result<&'a str, Error> {
    let v = values[at.ok_or(Error::Missing)?];
    if v.kind == Kind::String { Ok(v.text) } else { Err(Error::Type) }
}

fn optional_string<'a>(values: &[Value<'a>], at: Option<usize>) -> Result<&'a str, Error> {
    match at { None => Ok(""), Some(i) if values[i].kind == Kind::Null => Ok(""), some => string(values, some) }
}

/// `{"status": "ok", ...}`: what heartbeat, ACK and the deployment result answer on success.
pub fn ok_answer(values: &[Value<'_>]) -> Result<(), Error> {
    root(values)?;
    let [status] = members(values, 0, ["status"])?;
    if string(values, status)? == "ok" { Ok(()) } else { Err(Error::Shape) }
}

/// One command as delivered, with what the gateway's audit will need.
#[derive(Clone, Copy, Debug)]
pub struct Delivered<'a> { pub command: Command<'a>, pub initiator: &'a str, pub reason: &'a str, pub operation: &'a str }

/// A poll's answer: its commands in order, and whether the server waits for the owner to activate this agent.
#[derive(Clone, Copy, Debug)]
pub struct Commands<'a> { pub list: [Option<Delivered<'a>>; COMMANDS_MAX], pub count: usize, pub activation_required: bool }

pub fn commands<'a>(values: &[Value<'a>]) -> Result<Commands<'a>, Error> {
    root(values)?;
    let [list, activation] = members(values, 0, ["commands", "activation_required"])?;
    let activation_required = match activation.map(|i| values[i].kind) {
        None | Some(Kind::False) => false, Some(Kind::True) => true, Some(_) => return Err(Error::Type),
    };
    let mut out = Commands { list: [None; COMMANDS_MAX], count: 0, activation_required };
    let list = list.ok_or(Error::Missing)?;
    match values[list].kind {
        Kind::Null => return Ok(out),
        Kind::Array => {}
        _ => return Err(Error::Type),
    }
    for (i, v) in values.iter().enumerate().skip(list + 1) {
        if v.parent != Some(list as u16) { continue; }
        if out.count == COMMANDS_MAX { return Err(Error::Bounds); }
        let [id, agent, action, program, payload, expires, initiator, reason, operation] =
            members(values, i, ["id", "agent_id", "action", "program_name", "new_hostport", "expires_at", "initiator", "reason", "operation"])?;
        let expires_unix = match optional_string(values, expires)? { "" => None, t => rfc3339(t).ok_or(Error::Type)? };
        out.list[out.count] = Some(Delivered {
            command: Command {
                id: string(values, id)?, agent_id: string(values, agent)?, action: string(values, action)?,
                program: string(values, program)?, payload: string(values, payload)?, expires_unix,
            },
            initiator: optional_string(values, initiator)?, reason: optional_string(values, reason)?,
            operation: optional_string(values, operation)?,
        });
        out.count += 1;
    }
    Ok(out)
}

/// RFC 3339 time to Unix seconds: `None` inside for Go's zero time (no expiry); refused if malformed.
/// A `collect_file` command's payload (`new_hostport`, Effector's `FileTransferOperation`): the path, and the most the
/// server takes if it says; other fields are skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Collect<'a> { pub path: &'a str, pub max_bytes: Option<u64> }

pub fn collect<'a>(values: &[Value<'a>]) -> Result<Collect<'a>, Error> {
    root(values)?;
    let [path, max] = members(values, 0, ["path", "max_bytes"])?;
    let path = string(values, path)?;
    if path.is_empty() || path.len() > 256 { return Err(Error::Bounds); }
    let max_bytes = match max {
        None => None,
        Some(i) if values[i].kind == Kind::Number => Some(values[i].text.parse::<u64>().map_err(|_| Error::Type)?),
        Some(_) => return Err(Error::Type),
    };
    Ok(Collect { path, max_bytes })
}

pub fn rfc3339(t: &str) -> Option<Option<u64>> {
    let b = t.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !(b[10] == b'T' || b[10] == b't') || b[13] != b':' || b[16] != b':' { return None; }
    let num = |r: core::ops::Range<usize>| -> Option<u64> {
        if !b[r.clone()].iter().all(u8::is_ascii_digit) { return None; }
        core::str::from_utf8(&b[r]).ok()?.parse().ok()
    };
    let (year, month, day, hour, minute, second) = (num(0..4)?, num(5..7)?, num(8..10)?, num(11..13)?, num(14..16)?, num(17..19)?);
    let mut i = 19;
    if b.get(i) == Some(&b'.') { i += 1; let start = i; while b.get(i).map_or(false, u8::is_ascii_digit) { i += 1; } if i == start { return None; } }
    let offset: i64 = match b.get(i) {
        Some(b'Z') | Some(b'z') if i + 1 == b.len() => 0,
        Some(&s @ (b'+' | b'-')) if i + 6 == b.len() && b[i + 3] == b':' => {
            let (h, m) = (num(i + 1..i + 3)? as i64, num(i + 4..i + 6)? as i64);
            if h > 23 || m > 59 { return None; }
            let o = h * 3600 + m * 60; if s == b'+' { o } else { -o }
        }
        _ => return None,
    };
    if !(1..=12).contains(&month) || day == 0 || day > 31 || hour > 23 || minute > 59 || second > 60 { return None; }
    if year == 1 && month == 1 && day == 1 && hour == 0 && minute == 0 && second == 0 && offset == 0 { return Some(None); }
    if year < 1970 { return None; }
    // Days from the civil date (Howard Hinnant's algorithm).
    let (y, m) = if month <= 2 { (year as i64 - 1, month as i64 + 9) } else { (year as i64, month as i64 - 3) };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let secs = days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64 - offset;
    if secs < 0 { None } else { Some(Some(secs as u64)) }
}

/// Unix seconds to `YYYY-MM-DDTHH:MM:SSZ`, for `agent_time`.
pub fn rfc3339_utc(unix: u64, out: &mut [u8; 20]) -> &str {
    let days = (unix / 86400) as i64;
    let rem = unix % 86400;
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    let fields = [(year as u64, 4), (month as u64, 2), (day as u64, 2), (rem / 3600, 2), (rem / 60 % 60, 2), (rem % 60, 2)];
    let seps = [b'-', b'-', b'T', b':', b':', b'Z'];
    let mut i = 0;
    for ((v, width), sep) in fields.iter().zip(seps) {
        let mut v = *v;
        for k in (0..*width).rev() { out[i + k] = b'0' + (v % 10) as u8; v /= 10; }
        i += width;
        out[i] = sep;
        i += 1;
    }
    core::str::from_utf8(&out[..]).unwrap()
}
