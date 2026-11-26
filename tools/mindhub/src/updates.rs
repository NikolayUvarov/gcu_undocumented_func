//! The release tree under `/updates/` (docs/hub/README.md, 8; 100003-APP-0159): what `scripts/release.py publish`
//! writes, read-only, its names and ranges by `httpd`'s own code; the public channels alone, at a rate per source
//! network. The body is sent from the file as it is read, outside the hub's lock.

use crate::http::{reason, Request};
use httpd::layout::{self, Kind};
use httpd::range::{self, Asked};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// Where the tree is served: no route of the API's or the pages' starts so.
pub const PREFIX: &str = "/updates";

/// The channel offered when none is named.
pub const PUBLIC: &str = "stable";

/// A restricted channel's files are not offered until the updater signs its fetch (8.3).
pub const RESTRICTED: &str = "a channel not offered here: a restricted channel is served on the device's signature, which the updater does not give yet";

/// The tree, the channels anyone may fetch, and the requests a minute from one source network.
pub struct Updates {
    pub root: PathBuf,
    pub public: Vec<String>,
    pub per_minute: u32,
    counts: Mutex<HashMap<String, (Instant, u32)>>,
}

impl Updates {
    /// The tree at `root` (made canonical, so that `layout::open` walks from it), `stable` alone when `public` is empty.
    pub fn new(root: PathBuf, public: Vec<String>, per_minute: u32) -> Result<Updates, String> {
        let root = std::fs::canonicalize(&root).map_err(|e| format!("--releases {}: {e}", root.display()))?;
        if !root.is_dir() { return Err(format!("--releases {}: not a directory", root.display())); }
        let public = if public.is_empty() { vec![PUBLIC.to_string()] } else { public };
        Ok(Updates { root, public, per_minute: per_minute.max(1), counts: Mutex::new(HashMap::new()) })
    }

    // Seconds until this network's minute ends, when this request is past the rate.
    fn over_rate(&self, network: &str) -> Option<u64> {
        let mut counts = self.counts.lock().unwrap();
        let now = Instant::now();
        let c = counts.entry(network.to_string()).or_insert((now, 0));
        if now.duration_since(c.0) >= Duration::from_secs(60) { *c = (now, 0); }
        c.1 += 1;
        (c.1 > self.per_minute).then(|| 60u64.saturating_sub(now.duration_since(c.0).as_secs()).max(1))
    }
}

/// Whether a path is the tree's.
pub fn ours(path: &str) -> bool { path == PREFIX || path.starts_with("/updates/") }

/// The answer to a request under `PREFIX`: its status, and whether the connection may serve another.
pub fn answer<W: Write>(updates: Option<&Updates>, out: &mut W, req: &Request, network: &str) -> io::Result<(u16, bool)> {
    let Some(u) = updates else { return refuse(out, 404, "this hub serves no releases (mindhub serve --releases DIR)", &[]) };
    if req.method != "GET" && req.method != "HEAD" { return refuse(out, 405, "the release tree is read with GET or HEAD", &[("Allow", "GET, HEAD".into())]); }
    if !req.body.is_empty() { return refuse(out, 400, "a request with a body", &[]); }
    if let Some(wait) = u.over_rate(network) {
        return refuse(out, 429, &format!("more than {} requests a minute for the release tree from this network", u.per_minute), &[("Retry-After", wait.to_string())]);
    }
    let target = if req.query.is_empty() { req.path[PREFIX.len()..].to_string() } else { format!("{}?{}", &req.path[PREFIX.len()..], req.query) };
    let (relative, kind) = match layout::resolve(&target) { Ok(found) => found, Err(why) => return refuse(out, why.status(), why.text(), &[]) };
    let names: Vec<&str> = relative.split('/').collect();
    if let ["channels" | "timestamps" | "recovery", name] = names.as_slice() {
        if !u.public.iter().any(|p| p == name) { return refuse(out, 404, RESTRICTED, &[]); }
    }
    let (file, meta) = match layout::open(&u.root, &relative) { Ok(opened) => opened, Err(why) => return refuse(out, why.status(), why.text(), &[]) };
    let len = meta.len();
    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    // As httpd tags them: a blob by its digest, anything else by its length and change time.
    let tag = match kind {
        Kind::Blob => format!("\"{}\"", names.last().unwrap_or(&"")),
        _ => format!("\"{:x}-{:x}{:08x}\"", len, meta.mtime(), meta.mtime_nsec()),
    };
    let cache = if kind == Kind::Channel { "no-cache" } else { "public, max-age=31536000, immutable" };
    let keep = if req.close { "close" } else { "keep-alive" };
    let mut head = format!("Date: {}\r\nServer: mindhub\r\nAccept-Ranges: bytes\r\nETag: {tag}\r\nLast-Modified: {}\r\nCache-Control: {cache}\r\nConnection: {keep}\r\n",
                           httpd::date::http(SystemTime::now()), httpd::date::http(modified));
    if matches_tag(req.header("if-none-match"), &tag) {
        out.write_all(format!("HTTP/1.1 304 Not Modified\r\n{head}\r\n").as_bytes())?;
        return Ok((304, true));
    }
    let (status, start, count) = match range::asked(req.header("range"), len) {
        Asked::Whole => (200, 0, len),
        Asked::Part(first, last) => { head.push_str(&format!("Content-Range: bytes {first}-{last}/{len}\r\n")); (206, first, last - first + 1) }
        Asked::Unsatisfiable => return refuse(out, 416, "the range is outside the file", &[("Content-Range", format!("bytes */{len}"))]),
    };
    out.write_all(format!("HTTP/1.1 {status} {}\r\n{head}Content-Type: application/octet-stream\r\nContent-Length: {count}\r\n\r\n", reason(status)).as_bytes())?;
    if req.method == "HEAD" { return Ok((status, true)); }
    // A file cut shorter while it was read cannot give the length promised: the connection ends there.
    Ok((status, copy(file, start, count, out)? == count))
}

fn matches_tag(header: Option<&str>, tag: &str) -> bool {
    header.is_some_and(|h| h.split(',').map(|t| t.trim().trim_start_matches("W/")).any(|t| t == "*" || t == tag))
}

// A refusal: its status, the reason as text, and the connection closed after it.
fn refuse<W: Write>(out: &mut W, status: u16, why: &str, extra: &[(&str, String)]) -> io::Result<(u16, bool)> {
    let mut head = format!("HTTP/1.1 {status} {}\r\nServer: mindhub\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n", reason(status), why.len() + 1);
    for (name, value) in extra { head.push_str(&format!("{name}: {value}\r\n")); }
    out.write_all(format!("{head}\r\n{why}\n").as_bytes())?;
    Ok((status, false))
}

// The bytes `start..start+count` of the file, as they were when it was opened.
fn copy<W: Write>(mut file: File, start: u64, count: u64, out: &mut W) -> io::Result<u64> {
    file.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut sent = 0;
    while sent < count {
        let want = (count - sent).min(buf.len() as u64) as usize;
        let n = file.read(&mut buf[..want])?;
        if n == 0 { break; }
        out.write_all(&buf[..n])?;
        sent += n as u64;
    }
    out.flush()?;
    Ok(sent)
}
