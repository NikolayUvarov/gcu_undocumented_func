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

/// What the heartbeat says (contract 1.0's minimal heartbeat; 1.2: the command held and the load).
#[derive(Clone, Copy, Debug)]
pub struct Heartbeat<'a> {
    pub agent_id: &'a str,
    pub hostname: &'a str,
    pub arch: &'a str,
    pub agent_version: &'a str,
    pub stopping: bool,
    pub agent_time: Option<&'a str>,
    /// The command the agent holds until the server takes its answer: its one slot is occupied meanwhile, so the server
    /// does not take idle reports for an unconfirmed execution (Effector ahelp, `agent_execution_unconfirmed`).
    pub busy: Option<Busy<'a>>,
    /// The one-minute load, for the server's load gate.
    pub load: Option<Load>,
}

/// The command held: since when (RFC 3339), its ID and action.
#[derive(Clone, Copy, Debug)]
pub struct Busy<'a> { pub since: &'a str, pub command: &'a str, pub action: &'a str }

/// The load as the server reads it on Linux (Effector ahelp, "Load gate"): 100 x the one-minute load average / the
/// cores, and the average itself, both in thousandths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Load { pub percent: u64, pub average: u64, pub cores: u16 }

impl Load {
    /// From the load average x 100 and the online cores; none without cores.
    pub fn new(average_x100: u32, cores: u16) -> Option<Self> {
        (cores > 0).then(|| Self { percent: average_x100 as u64 * 1000 / cores as u64, average: average_x100 as u64 * 10, cores })
    }
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
    w.field_bool("busy", h.busy.is_some())?;
    if let Some(b) = h.busy {
        w.field_str("busy_since", b.since)?;
        w.key("executing_commands")?; w.array()?;
        w.object()?; w.field_str("command_id", b.command)?; w.field_str("action", b.action)?; w.field_str("started_at", b.since)?; w.end()?;
        w.end()?;
    }
    w.field_u64("max_concurrent_commands", 1)?;
    w.field_u64("occupied_queues", h.busy.is_some() as u64)?;
    w.field_u64("free_slots", h.busy.is_none() as u64)?;
    w.field_bool("config_auto_update_enabled", false)?;
    w.field_str("transport_mode", "https_only")?;
    w.field_str("transport", "https")?;
    // Its actions run within their fixed budget: no reservation extends one (Effector ahelp, build 232, the fixed mode).
    w.field_bool("renewable_execution_timeout", false)?;
    if let Some(t) = h.agent_time { w.field_str("agent_time", t)?; }
    if let Some(l) = h.load {
        w.key("load")?; w.object()?;
        w.field_decimal("load_percent", l.percent)?; w.field_decimal("load_average", l.average)?; w.field_u64("cpu_cores", l.cores as u64)?;
        w.end()?;
    }
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

/// What an answer carries back (contract 1.1, as the server's other agents send screenshots): its bytes go into the
/// result as base64, which the server keeps as the operation's artifact. `picture`: a PNG's width and height.
#[derive(Clone, Copy, Debug)]
pub struct Artifact<'a> { pub name: &'a str, pub content_type: &'a str, pub size: u64, pub sha256: &'a str, pub truncated: bool, pub picture: Option<(u32, u32)> }

/// What follows an artifact's base64 text in its ACK: the end of the data string, of the artifact, of the result, of
/// the ACK (the result is a string, so its quotes are escaped).
pub const ARTIFACT_SUFFIX: &[u8] = b"\\\"}}\"}";

/// The ACK of a completed result with an artifact up to its base64 text, which `ARTIFACT_SUFFIX` follows. The data is
/// the artifact's last member, so the text before it is what `ack` writes with the data empty.
pub fn artifact_ack<'b>(buf: &'b mut [u8], command_id: &str, a: &Artifact<'_>) -> Result<&'b [u8], Error> {
    let mut inner = [0u8; 1024];
    let mut w = Writer::new(&mut inner);
    w.object()?;
    w.field_str("status", "completed")?;
    w.field_u64("exit_code", 0)?;
    w.field_str("stdout", "")?;
    w.field_str("stderr", "")?;
    w.field_bool("truncated", a.truncated)?;
    w.field_str("run_as", "effector")?;
    w.key("artifact")?;
    w.object()?;
    w.field_str("name", a.name)?;
    w.field_str("content_type", a.content_type)?;
    w.field_str("encoding", "base64")?;
    w.field_u64("size", a.size)?;
    w.field_str("sha256", a.sha256)?;
    if let Some((width, height)) = a.picture {
        w.key("metadata")?; w.object()?; w.field_u64("width", width as u64)?; w.field_u64("height", height as u64)?; w.end()?;
    }
    w.field_str("data", "")?;
    w.end()?;
    w.end()?;
    let result = core::str::from_utf8(w.finish()?).map_err(|_| Error::Write)?;
    let n = ack(buf, command_id, result)?.len();
    if !buf[..n].ends_with(ARTIFACT_SUFFIX) { return Err(Error::Write); }
    Ok(&buf[..n - ARTIFACT_SUFFIX.len()])
}

/// The length of `n` bytes as base64 (with padding).
pub fn base64_len(n: u64) -> u64 { n.div_ceil(3) * 4 }

/// `input` as base64 into `out` (at least `base64_len(input.len())` bytes); only the last piece of a stream may have a
/// length that is not a multiple of 3. Returns the bytes written.
pub fn base64(input: &[u8], out: &mut [u8]) -> usize {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut n = 0;
    for group in input.chunks(3) {
        let v = (group[0] as u32) << 16 | (*group.get(1).unwrap_or(&0) as u32) << 8 | *group.get(2).unwrap_or(&0) as u32;
        out[n] = ALPHABET[(v >> 18) as usize & 63];
        out[n + 1] = ALPHABET[(v >> 12) as usize & 63];
        out[n + 2] = if group.len() > 1 { ALPHABET[(v >> 6) as usize & 63] } else { b'=' };
        out[n + 3] = if group.len() > 2 { ALPHABET[v as usize & 63] } else { b'=' };
        n += 4;
    }
    n
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

/// A `deliver_file` payload (Effector's FileTransferOperation): where, how many bytes with which SHA-256, and whether an
/// existing file may be replaced. The gateway's policy decides the path; `max_bytes` bounds the announced size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deliver<'a> { pub path: &'a str, pub size: u64, pub sha256: &'a str, pub overwrite: bool }

pub fn deliver<'a>(values: &[Value<'a>]) -> Result<Deliver<'a>, Error> {
    root(values)?;
    let [path, size, sha256, overwrite, max] = members(values, 0, ["path", "size", "sha256", "overwrite", "max_bytes"])?;
    let path = string(values, path)?;
    if path.is_empty() || path.len() > 256 { return Err(Error::Bounds); }
    let number = |at: Option<usize>| match at.map(|i| values[i]) {
        None => Ok(None),
        Some(v) if v.kind == Kind::Number => v.text.parse::<u64>().map(Some).map_err(|_| Error::Type),
        Some(_) => Err(Error::Type),
    };
    let size = number(size)?.ok_or(Error::Missing)?;
    if number(max)?.is_some_and(|m| size > m) { return Err(Error::Bounds); }
    let sha256 = string(values, sha256)?;
    if sha256.len() != 64 || !sha256.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) { return Err(Error::Type); }
    let overwrite = match overwrite.map(|i| values[i].kind) { None | Some(Kind::False) => false, Some(Kind::True) => true, Some(_) => return Err(Error::Type) };
    Ok(Deliver { path, size, sha256, overwrite })
}

/// An `exec_v1` payload's command (Effector's ExecOperation). MIND runs none: the agent maps only `reboot` to the
/// gateway's restart, under the owner's `reboot on`; every other command is denied.
pub fn exec_command<'a>(values: &[Value<'a>]) -> Result<&'a str, Error> {
    root(values)?;
    let [command] = members(values, 0, ["command"])?;
    string(values, command)
}

/// A command's own execution budget (`timeout_seconds`, 1–3600) when its payload names one; it is apart from the
/// command's term in the server's queue (Effector ahelp, "Reservation lease, queue waiting and command runtime").
pub fn timeout_seconds(values: &[Value<'_>]) -> Result<Option<u64>, Error> {
    root(values)?;
    let [t] = members(values, 0, ["timeout_seconds"])?;
    match t.map(|i| values[i]) {
        None => Ok(None),
        Some(v) if v.kind == Kind::Number => v.text.parse::<u64>().ok().filter(|s| (1..=3600).contains(s)).map(Some).ok_or(Error::Bounds),
        Some(_) => Err(Error::Type),
    }
}

/// A `performance_measure` payload's `duration_seconds` (Effector's PerformanceOperation, 1–300), if it names one.
pub fn duration_seconds(values: &[Value<'_>]) -> Result<Option<u64>, Error> {
    root(values)?;
    let [d] = members(values, 0, ["duration_seconds"])?;
    match d.map(|i| values[i]) {
        None => Ok(None),
        Some(v) if v.kind == Kind::Number => v.text.parse::<u64>().ok().filter(|s| (1..=300).contains(s)).map(Some).ok_or(Error::Bounds),
        Some(_) => Err(Error::Type),
    }
}

/// A `collect_logs` payload: JSON with `path` (Effector's CollectLogOperation), or the path itself.
pub fn log_path<'a>(values: &[Value<'a>]) -> Result<&'a str, Error> {
    root(values)?;
    let [path] = members(values, 0, ["path"])?;
    let path = string(values, path)?;
    if path.is_empty() || path.len() > 256 { return Err(Error::Bounds); }
    Ok(path)
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
