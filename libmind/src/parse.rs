//! The parser service's client side (109-NET-0008, idl/parse.wit): typed HTTP heads to and from `mind::http::Head`, and
//! a `mind::http::Parser` that asks the service, so a program that holds the network and its files parses no response
//! head itself (MC-11.11). `mind::http::get` checks whatever comes back against what it asked for (MC-11.5). Release
//! channels and boot manifests to and from `mind::release` (351-NET-0011): the updater encodes what comes back again
//! and compares it with the signed bytes.
use crate::http::{self, Head, Range};
use crate::idl::codec::{List, Text};
use crate::idl::parse::{self, HttpHead};
use crate::ipc::Endpoint;
use crate::release;

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
    Head { status: record.status, length: record.has_length.then_some(record.length), range, chunked: record.chunked }
}

/// A `mind::http::Parser` that asks the parser service at the endpoint (`SLOT_PARSE` in a program that asked for it).
pub struct Service(pub Endpoint);

impl http::Parser for Service {
    fn head(&mut self, head: &[u8]) -> Result<Head, http::Error> {
        match parse::http_head(self.0, head) {
            Ok(Ok(record)) => Ok(from_record(&record)),
            Ok(Err(parse::Error::Malformed)) => Err(http::Error::Head),
            Err(_) => Err(http::Error::Parser),
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
