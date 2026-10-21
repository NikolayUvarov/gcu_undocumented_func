//! The parser service's client side (109-NET-0008, idl/parse.wit): typed HTTP heads to and from `mind::http::Head`, and
//! a `mind::http::Parser` that asks the service, so a program that holds the network and its files parses no response
//! head itself (MC-11.11). `mind::http::get` checks whatever comes back against what it asked for (MC-11.5). Release
//! channels and boot manifests to and from `mind::release` (351-NET-0011): the updater encodes what comes back again
//! and compares it with the signed bytes. A `Session` is a parser process of one session's own (109-NET-0010).
use crate::http::{self, Head, Range};
use crate::idl::codec::{List, Text};
use crate::idl::parse::{self, HttpHead, HttpResponse};
use crate::ipc::{Endpoint, Message};
use crate::release;
use core::sync::atomic::{AtomicUsize, Ordering};

pub fn to_record(head: &Head) -> HttpHead {
    let (has_range, range_unsatisfied, range_start, range_end, range_total) = match head.range {
        None => (false, false, 0, 0, 0),
        Some(Range::Bytes { start, end, total }) => (true, false, start, end, total),
        Some(Range::Unsatisfied { total }) => (true, true, 0, 0, total),
    };
    HttpHead { status: head.status, has_length: head.length.is_some(), length: head.length.unwrap_or(0), has_range, range_unsatisfied, range_start, range_end, range_total, chunked: head.chunked }
}

pub fn from_record(record: &HttpHead) -> Head {
    let range = match (record.has_range, record.range_unsatisfied) {
        (false, _) => None,
        (true, true) => Some(Range::Unsatisfied { total: record.range_total }),
        (true, false) => Some(Range::Bytes { start: record.range_start, end: record.range_end, total: record.range_total }),
    };
    Head { status: record.status, length: record.has_length.then_some(record.length), range, chunked: record.chunked, close: false }
}

/// A head with whether its connection closes after it (parse.wit 1.3, 750-NET-0019).
pub fn to_response(head: &Head) -> HttpResponse {
    let r = to_record(head);
    HttpResponse { status: r.status, has_length: r.has_length, length: r.length, has_range: r.has_range, range_unsatisfied: r.range_unsatisfied,
                   range_start: r.range_start, range_end: r.range_end, range_total: r.range_total, chunked: r.chunked, close: head.close }
}

pub fn from_response(r: &HttpResponse) -> Head {
    let head = HttpHead { status: r.status, has_length: r.has_length, length: r.length, has_range: r.has_range, range_unsatisfied: r.range_unsatisfied,
                          range_start: r.range_start, range_end: r.range_end, range_total: r.range_total, chunked: r.chunked };
    Head { close: r.close, ..from_record(&head) }
}

/// A `mind::http::Parser` that asks the parser service at the endpoint (`SLOT_PARSE` in a program that asked for it).
pub struct Service(pub Endpoint);

impl http::Parser for Service {
    fn head(&mut self, head: &[u8]) -> Result<Head, http::Error> {
        match parse::http_response(self.0, head) {
            Ok(Ok(record)) => Ok(from_response(&record)),
            Ok(Err(parse::Error::Malformed)) => Err(http::Error::Head),
            Err(_) => Err(http::Error::Parser),
        }
    }
}

/// A parser process of one session's own (109-NET-0010, Appendix B.6): the loader starts `parse` as a helper that holds
/// nothing but this session's endpoint, and it ends when the session is dropped (or `end` is called), or after 30 s
/// without a request. The session keeps only a copy that calls, so a call to a helper that has ended fails at once,
/// and the session starts another and asks again. Where the loader starts no helper (an older loader, or no endpoint
/// left in this program's quota), the session uses the shared service at `shared`.
pub struct Session { endpoint: Endpoint, pid: Option<u64>, loader: Option<Endpoint>, restarts: u32 }

/// Why a session got no parser of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotOwn { Endpoint(crate::sys::Error), Loader(crate::idl::loader::Error), Call(crate::sys::Error) }

impl Session {
    /// A parser of the session's own, which the loader starts as a helper.
    pub fn own(loader: Endpoint) -> Result<Self, NotOwn> {
        let created = loop {
            match Endpoint::create() {
                // A helper this program just ended may hold its last endpoint until the kernel reaps it (109-KRN-0133).
                Err(crate::sys::Error::Other(crate::abi::ERR_LIMIT)) if recently_ended() => { crate::time::sleep(2); }
                other => break other.map_err(NotOwn::Endpoint)?,
            }
        };
        // The helper's copy alone can receive; this program keeps one that calls and lends its head's pages, so that with
        // the helper gone the kernel finds no receiver and fails the call (ERR_PEER) instead of letting it wait for ever.
        let calls = crate::ipc::mint(created.0, crate::abi::CAP_WRITE | crate::abi::CAP_GRANT, 0, 0).map_err(NotOwn::Endpoint);
        let started = calls.and_then(|calls| crate::ipc::mint(created.0, crate::abi::CAP_READ | crate::abi::CAP_WRITE, 0, 0).map_err(NotOwn::Endpoint).and_then(|lent| {
            let pid = crate::idl::loader::helper(loader, crate::idl::loader::HelperKind::Parse, lent);
            let _ = crate::ipc::drop_cap(lent);
            match pid { Ok(Ok(pid)) => Ok((calls, pid)), Ok(Err(error)) => Err(NotOwn::Loader(error)), Err(error) => Err(NotOwn::Call(error)) }
        }).inspect_err(|_| { let _ = crate::ipc::drop_cap(calls); }));
        let _ = crate::ipc::drop_cap(created.0);
        started.map(|(calls, pid)| Self { endpoint: Endpoint(calls), pid: Some(pid), loader: Some(loader), restarts: 0 })
    }

    /// The shared service at `endpoint`, for a session that got no parser of its own.
    pub fn shared(endpoint: Endpoint) -> Self { Self { endpoint, pid: None, loader: None, restarts: 0 } }

    /// How many times its own parser had ended (30 s without a request) and another was started.
    pub fn restarts(&self) -> u32 { self.restarts }

    /// A parser of the session's own, else the shared service at `shared`.
    pub fn start(loader: Endpoint, shared: Endpoint) -> Self { Self::own(loader).unwrap_or(Self::shared(shared)) }

    /// Where the session's requests go: its own parser's endpoint, or the shared service's.
    pub fn endpoint(&self) -> Endpoint { self.endpoint }

    /// The PID of the session's own parser; None when it uses the shared service.
    pub fn pid(&self) -> Option<u64> { self.pid }
}

/// Ends a session's own parser: a message that is not a call, then this program's capability goes.
pub fn end_session(endpoint: Endpoint) {
    let _ = endpoint.send_timeout(&Message::new(0, 0), 100);
    let _ = crate::ipc::drop_cap(endpoint.0);
}

/// When this program last ended a parser of its own (uptime in ms; 0: never).
static ENDED_AT: AtomicUsize = AtomicUsize::new(0);
/// How long an ended helper is waited for, and how long after it its endpoint may still count in this program's quota.
const ENDING_MS: usize = 1000;

fn recently_ended() -> bool {
    let at = ENDED_AT.load(Ordering::Relaxed);
    at != 0 && crate::time::uptime_ms().saturating_sub(at) < ENDING_MS
}

// Ends the session's own parser and waits, within ENDING_MS, until its task has ended, so that the next session finds
// the endpoint's quota free again once the kernel has reaped it (109-KRN-0133).
fn end_own(endpoint: Endpoint, pid: u64) {
    end_session(endpoint);
    let started = crate::time::uptime_ms();
    while crate::process::alive(pid) && crate::time::uptime_ms().saturating_sub(started) < ENDING_MS { crate::time::sleep(2); }
    ENDED_AT.store(crate::time::uptime_ms().max(1), Ordering::Relaxed);
}

impl Drop for Session {
    fn drop(&mut self) { if let Some(pid) = self.pid.take() { end_own(self.endpoint, pid); } }
}

impl http::Parser for Session {
    fn head(&mut self, head: &[u8]) -> Result<Head, http::Error> {
        match (Service(self.endpoint).head(head), self.loader) {
            // Its own parser could not be reached, as when it ended after 30 s idle: another one, asked once more.
            (Err(http::Error::Parser), Some(loader)) if self.pid.is_some() => {
                if let Some(pid) = self.pid.take() { end_own(self.endpoint, pid); }
                let restarts = self.restarts + 1;
                *self = Self::own(loader).map_err(|_| http::Error::Parser)?;
                self.restarts = restarts;
                Service(self.endpoint).head(head)
            }
            (result, _) => result,
        }
    }
}

/// The record of a channel `mind::release` read.
pub fn channel_record(channel: &release::Channel, signed: &release::Signed) -> parse::Channel {
    let mut manifests = List::default();
    for (arch, digest) in &channel.manifests[..channel.count] {
        manifests.push(parse::ManifestRef { arch: Text::new(arch.as_str()).unwrap_or_default(), digest: List::from_slice(digest).unwrap_or_default() });
    }
    parse::Channel {
        name: Text::new(channel.name.as_str()).unwrap_or_default(), version: channel.version, minimum: channel.minimum,
        expires: Text::new(core::str::from_utf8(&channel.expires).unwrap_or("")).unwrap_or_default(), manifests,
        signed: signed.body as u32, signature: List::from_slice(&signed.signature).unwrap_or_default(),
    }
}

/// A channel from the service's record, if every field has its form. Its encoding is still to be compared with the
/// signed bytes before anything in it is trusted.
pub fn channel_from(record: &parse::Channel) -> Option<(release::Channel, release::Signed)> {
    let mut manifests = [(release::Text::EMPTY, [0u8; 32]); release::ARCHES];
    for (slot, m) in manifests.iter_mut().zip(record.manifests.as_slice()) {
        *slot = (release::Text::new(m.arch.as_str().as_bytes())?, m.digest.as_slice().try_into().ok()?);
    }
    let channel = release::Channel {
        name: release::Text::new(record.name.as_str().as_bytes())?, version: record.version, minimum: record.minimum,
        expires: record.expires.as_str().as_bytes().try_into().ok()?, manifests, count: record.manifests.as_slice().len(),
    };
    Some((channel, release::Signed { body: record.signed as usize, signature: record.signature.as_slice().try_into().ok()? }))
}

/// The record of a manifest `mind::release` read: its header lines, its number of files, and up to `PAGE` from `start`.
pub fn manifest_record(manifest: &release::Manifest, start: u32) -> parse::Manifest {
    let mut headers = List::default();
    for line in (0..manifest.headers()).filter_map(|i| manifest.header(i)) {
        headers.push(Text::new(core::str::from_utf8(line).unwrap_or("")).unwrap_or_default());
    }
    let mut page = List::default();
    for file in (start as usize..manifest.files()).take(PAGE).filter_map(|i| manifest.file(i)) {
        page.push(parse::File { path: Text::new(file.path.as_str()).unwrap_or_default(), size: file.size, digest: List::from_slice(&file.digest).unwrap_or_default(), flags: file.flags, mib: file.mib });
    }
    parse::Manifest { headers, files: manifest.files() as u32, page }
}

/// A file line from the service's record, if every field has its form.
pub fn file_from(record: &parse::File) -> Option<release::File> {
    Some(release::File { path: release::Text::new(record.path.as_str().as_bytes())?, size: record.size, digest: record.digest.as_slice().try_into().ok()?, flags: record.flags, mib: record.mib })
}

/// File lines in one reply of `manifest` (idl/parse.wit).
pub const PAGE: usize = 8;

/// Why the service's reading of a release file was not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// No parser service answered.
    Service,
    /// The service found the bytes malformed.
    Malformed,
    /// The service's answer does not make exactly the bytes: it is not to be trusted.
    Lied,
}

/// A channel file read by the service at `endpoint`, taken only if its answer makes exactly `file` (`release::channel_is`).
/// The signature is still to be verified over the file's first `signed.body` bytes.
pub fn read_channel(endpoint: Endpoint, file: &[u8]) -> Result<(release::Channel, release::Signed), Refused> {
    let record = match parse::channel(endpoint, file) { Ok(Ok(record)) => record, Ok(Err(_)) => return Err(Refused::Malformed), Err(_) => return Err(Refused::Service) };
    let (channel, signed) = channel_from(&record).ok_or(Refused::Lied)?;
    if release::channel_is(file, &channel, &signed) { Ok((channel, signed)) } else { Err(Refused::Lied) }
}

/// A boot manifest read by the service at `endpoint` a page at a time, taken only if its header lines and its files,
/// encoded again, make exactly `text`; `each` sees every file in order (and may act only once this returns Ok). Gives
/// the number of header lines and of files.
pub fn read_manifest(endpoint: Endpoint, text: &[u8], mut each: impl FnMut(&release::File)) -> Result<(usize, usize), Refused> {
    let mut rebuild = release::Rebuild::new(text);
    let (mut headers, mut files, mut start) = (0, None, 0u32);
    loop {
        let page = match parse::manifest(endpoint, text, start) { Ok(Ok(page)) => page, Ok(Err(_)) => return Err(Refused::Malformed), Err(_) => return Err(Refused::Service) };
        let total = *files.get_or_insert(page.files);
        let expected = (total.saturating_sub(start) as usize).min(PAGE);
        if page.files != total || page.page.as_slice().len() != expected { return Err(Refused::Lied); }
        if start == 0 {
            headers = page.headers.as_slice().len();
            if !page.headers.as_slice().iter().all(|line| rebuild.line(line.as_str().as_bytes())) { return Err(Refused::Lied); }
        }
        for record in page.page.as_slice() {
            let file = file_from(record).ok_or(Refused::Lied)?;
            if !rebuild.file(&file) { return Err(Refused::Lied); }
            each(&file);
        }
        start += expected as u32;
        if expected < PAGE || start == total { break; }
    }
    if rebuild.done() { Ok((headers, files.unwrap_or(0) as usize)) } else { Err(Refused::Lied) }
}

/// The record of model `index` of a model manifest (`mind::models`), with up to 8 of its files from `start`.
pub fn model_record(text: &[u8], index: u32, start: u32) -> Result<parse::Model, crate::models::Malformed> {
    let mut files = [crate::models::File::EMPTY; 8];
    let (count, model, n) = crate::models::read(text, index as usize, start as usize, &mut files)?;
    let mut page = List::default();
    for f in &files[..n] {
        page.push(parse::ModelFile { path: Text::new(f.path.as_str()).unwrap_or_default(), size: f.size, sha256: List::from_slice(&f.sha256).unwrap_or_default() });
    }
    Ok(parse::Model { models: count as u32, id: Text::new(model.id.as_str()).unwrap_or_default(), entry_start: model.start as u32,
                      entry_length: (model.end - model.start) as u32, files: model.files as u32, page })
}
