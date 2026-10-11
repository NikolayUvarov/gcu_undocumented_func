// blocks models (251-STO-0014): speech models of a model disk (`models:`) into the block store, each model one object
// named `models/<id>`, and a model's file read back by name. The parser service reads MANIFEST.json (`parse::model`);
// every file is checked against its size and SHA-256 as it is read, and a model with a file that differs is not named.
//
// A model's object is its header, then its files' bytes one after another in the header's order:
//   MIND-MODEL 1\n  id <id>\n  entry <n>\n<the manifest's entry, n bytes>\n  file <size> <sha256> <path>\n ...  data\n
// The entry keeps the licence and the terms with the model. One object per model, so its name retains every file.
use mind::abi::SLOT_PARSE;
use mind::cid::Cid;
use mind::dag::{self, Builder, CHUNK};
use mind::fs::File;
use mind::idl::{blockstore, parse};
use mind::ipc::Endpoint;
use mind::sha256::Sha256;

use crate::{report, Remote, STORE};

const PARSE: Endpoint = Endpoint(SLOT_PARSE);
/// The most a model's files and its header may be.
const FILES: usize = 64;
const HEADER_MAX: usize = 32 * 1024;

static mut MANIFEST: [u8; 60_000] = [0; 60_000];
static mut HEADER: [u8; HEADER_MAX] = [0; HEADER_MAX];

// Bytes into a buffer, failing when it is full.
struct Out<'a> { buf: &'a mut [u8], len: usize }
impl core::fmt::Write for Out<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result { self.put(s.as_bytes()).ok_or(core::fmt::Error) }
}
impl Out<'_> {
    fn put(&mut self, bytes: &[u8]) -> Option<()> {
        let end = self.len.checked_add(bytes.len()).filter(|&e| e <= self.buf.len())?;
        self.buf[self.len..end].copy_from_slice(bytes);
        self.len = end;
        Some(())
    }
}

fn hex(digest: &[u8]) -> impl core::fmt::Display + '_ {
    struct Hex<'a>(&'a [u8]);
    impl core::fmt::Display for Hex<'_> {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { self.0.iter().try_for_each(|b| write!(f, "{:02x}", b)) }
    }
    Hex(digest)
}

pub fn command<'a>(mut words: impl Iterator<Item = &'a str>, builder: &mut Builder, buffer: &mut [u8; CHUNK], data: &mut [u8; CHUNK], remote: &mut Remote) {
    match (words.next(), words.next(), words.next(), words.next()) {
        (Some("import"), id, None, _) => import(id, builder, data, remote),
        (Some("get"), Some(id), Some(path), Some(dest)) => get(id, path, dest, buffer, data, remote),
        _ => mind::println!("Usage: blocks models import [id] | get <id> <path> <file>"),
    }
}

// The manifest of `models:`, read whole.
fn manifest() -> Option<&'static [u8]> {
    let text = unsafe { &mut *core::ptr::addr_of_mut!(MANIFEST) };
    let Ok(mut file) = File::open("models:MANIFEST.json") else { mind::println!("blocks: models: no models:MANIFEST.json"); return None };
    if file.size() > text.len() { mind::println!("blocks: models: MANIFEST.json is larger than {} bytes", text.len()); return None; }
    let n = file.size();
    if file.read(&mut text[..n]) != Ok(n) { mind::println!("blocks: models: cannot read MANIFEST.json"); return None; }
    Some(&text[..n])
}

// Model `index` of the manifest with every one of its files, through the parser service, page by page.
fn model(text: &[u8], index: u32, list: &mut [parse::ModelFile; FILES]) -> Option<(parse::Model, usize)> {
    let mut first: Option<parse::Model> = None;
    let mut n = 0;
    loop {
        let page = match parse::model(PARSE, text, index, n as u32) {
            Ok(Ok(page)) => page,
            Ok(Err(e)) => { mind::println!("blocks: models: the parser refused the manifest ({:?})", e); return None; }
            Err(e) => { mind::println!("blocks: models: no parser service ({:?})", e); return None; }
        };
        let total = page.files as usize;
        if total > FILES { mind::println!("blocks: models: {} has {} files, more than {}", page.id, total, FILES); return None; }
        // Every page tells the same model; anything else is not taken.
        if first.as_ref().is_some_and(|f| (f.models, f.id, f.entry_start, f.entry_length, f.files) != (page.models, page.id, page.entry_start, page.entry_length, page.files)) {
            mind::println!("blocks: models: the parser's pages differ");
            return None;
        }
        for f in page.page.as_slice() { if n < FILES { list[n] = *f; n += 1; } }
        let done = page.page.as_slice().len() < 8 || n >= total;
        first.get_or_insert(page);
        if done { break; }
    }
    let model = first?;
    if n != model.files as usize { mind::println!("blocks: models: the parser gave {} of {} files", n, model.files); return None; }
    Some((model, n))
}

fn import(only: Option<&str>, builder: &mut Builder, data: &mut [u8; CHUNK], remote: &mut Remote) {
    let Some(text) = manifest() else { return };
    let mut list = [parse::ModelFile::default(); FILES];
    let header = unsafe { &mut *core::ptr::addr_of_mut!(HEADER) };
    let (mut index, mut count, mut imported) = (0u32, 1u32, 0);
    while index < count {
        let Some((model, n)) = model(text, index, &mut list) else { return };
        count = model.models;
        index += 1;
        let id = model.id.as_str();
        if only.is_some_and(|o| o != id) { continue; }
        // The entry's bytes, from this program's own copy of the manifest: an object whole, by the parser's bounds.
        let (start, length) = (model.entry_start as usize, model.entry_length as usize);
        let Some(entry) = text.get(start..start + length).filter(|e| e.first() == Some(&b'{') && e.last() == Some(&b'}')) else {
            mind::println!("blocks: models: {}: the entry's bounds are not an object", id);
            return;
        };
        let mut h = Out { buf: header, len: 0 };
        let written = (|| -> Option<()> {
            use core::fmt::Write;
            write!(h, "MIND-MODEL 1\nid {}\nentry {}\n", id, entry.len()).ok()?;
            h.put(entry)?;
            h.put(b"\n")?;
            for f in &list[..n] { write!(h, "file {} {} {}\n", f.size, hex(f.sha256.as_slice()), f.path).ok()?; }
            h.put(b"data\n")
        })();
        if written.is_none() { mind::println!("blocks: models: {}: its header is larger than {} bytes", id, HEADER_MAX); return; }
        if let Err(e) = builder.write(remote, &h.buf[..h.len]) { return report("models import", e, remote); }
        let mut bytes = 0u64;
        for f in &list[..n] {
            let mut path = Out { buf: &mut [0u8; 200], len: 0 };
            if core::fmt::Write::write_fmt(&mut path, format_args!("models:{}/{}", id, f.path)).is_err() { return; }
            let path = core::str::from_utf8(&path.buf[..path.len]).unwrap_or("");
            let Ok(mut file) = File::open(path) else { mind::println!("blocks: models: cannot open {}", path); return };
            let mut hash = Sha256::new();
            let mut got = 0u64;
            loop {
                let k = match file.read(data) { Ok(k) => k, Err(e) => { mind::println!("blocks: models: reading {}: {:?}", path, e); return } };
                if k == 0 { break; }
                hash.update(&data[..k]);
                got += k as u64;
                if got > f.size { break; }
                if let Err(e) = builder.write(remote, &data[..k]) { return report("models import", e, remote); }
            }
            if got != f.size || hash.finish()[..] != *f.sha256.as_slice() {
                // Nothing is named: the blocks written so far are not retained and a collection frees them.
                mind::println!("blocks: models: {} is not as the manifest says ({} bytes read of {}): {} not imported", path, got, f.size, id);
                return;
            }
            bytes += got;
        }
        let (root, size) = match builder.finish(remote) { Ok(r) => r, Err(e) => return report("models import", e, remote) };
        let mut name = Out { buf: &mut [0u8; 64], len: 0 };
        if core::fmt::Write::write_fmt(&mut name, format_args!("models/{}", id)).is_err() { mind::println!("blocks: models: {}: the id is too long for a name", id); return; }
        let name = core::str::from_utf8(&name.buf[..name.len]).unwrap_or("");
        let expected = match blockstore::resolve(STORE, name) { Ok(Ok(head)) => head.version, _ => 0 };
        match blockstore::publish(STORE, name, expected, &root.to_bytes()) {
            Ok(Ok(version)) => mind::println!("IMPORTED {}: {} FILES, {} BYTES, OBJECT {} ({} BYTES) AS {} VERSION {}", id, n, bytes, root, size, name, version),
            Ok(Err(e)) => { mind::println!("blocks: models: publish {}: {:?}", name, e); return; }
            Err(e) => { mind::println!("blocks: models: the store does not answer: {:?}", e); return; }
        }
        imported += 1;
    }
    if imported == 0 { mind::println!("blocks: models: no model {}", only.unwrap_or("on models:")); } else { mind::println!("{} MODELS IMPORTED", imported); }
}

// A file of a model, by the model's name, to `dest`, checked against the SHA-256 its header gives.
fn get(id: &str, path: &str, dest: &str, buffer: &mut [u8; CHUNK], data: &mut [u8; CHUNK], remote: &mut Remote) {
    let mut name = Out { buf: &mut [0u8; 64], len: 0 };
    if core::fmt::Write::write_fmt(&mut name, format_args!("models/{}", id)).is_err() { return; }
    let name = core::str::from_utf8(&name.buf[..name.len]).unwrap_or("");
    let root = match blockstore::resolve(STORE, name) {
        Ok(Ok(head)) => match Cid::from_bytes(head.root.as_slice()) { Ok(root) => root, Err(_) => { mind::println!("blocks: models: {} has no root", name); return } },
        Ok(Err(e)) => { mind::println!("blocks: models: {}: {:?}", name, e); return; }
        Err(e) => { mind::println!("blocks: models: the store does not answer: {:?}", e); return; }
    };
    // The header, read until its `data` line; the object is this program's own format, checked as it goes.
    let header = unsafe { &mut *core::ptr::addr_of_mut!(HEADER) };
    let mut have = 0;
    let end = loop {
        if have == header.len() { mind::println!("blocks: models: {}: no header", name); return; }
        let k = match dag::read_at(remote, &root, have as u64, &mut header[have..], buffer) { Ok(k) => k, Err(e) => return report("models get", e, remote) };
        if k == 0 { mind::println!("blocks: models: {}: no header", name); return; }
        have += k;
        if let Some(at) = find_data(&header[..have]) { break at; }
    };
    let Some((offset, size, digest)) = locate(&header[..end], path) else { mind::println!("blocks: models: {} has no file {}", name, path); return };
    let Ok(mut file) = File::create(dest) else { mind::println!("blocks: cannot create {}", dest); return };
    let (mut at, mut hash) = (0u64, Sha256::new());
    while at < size {
        let want = (size - at).min(CHUNK as u64) as usize;
        let k = match dag::read_at(remote, &root, end as u64 + offset + at, &mut data[..want], buffer) { Ok(k) => k, Err(e) => return report("models get", e, remote) };
        if k == 0 { mind::println!("blocks: models: {}: the object ends early", name); return; }
        hash.update(&data[..k]);
        if file.write(&data[..k]).ok() != Some(k) { mind::println!("blocks: writing {} failed", dest); return; }
        at += k as u64;
    }
    let _ = file.flush();
    if hash.finish() != digest { mind::println!("blocks: models: {} of {} does not match its SHA-256", path, name); return; }
    mind::println!("GOT {} OF {}: {} BYTES, SHA-256 {}", path, name, size, hex(&digest));
}

// Where the file lines start: after the format, id and entry lines and the entry's bytes.
fn files_start(header: &[u8]) -> Option<usize> {
    if !header.starts_with(b"MIND-MODEL 1\nid ") { return None; }
    let at = header.windows(7).position(|w| w == b"\nentry ")? + 7;
    let n_end = at + header[at..].iter().position(|&b| b == b'\n')?;
    let entry: usize = core::str::from_utf8(&header[at..n_end]).ok()?.parse().ok()?;
    (header.get(n_end + 1 + entry) == Some(&b'\n')).then_some(n_end + 1 + entry + 1)
}

// Where the files' bytes start: just after the `data` line that follows the file lines.
fn find_data(header: &[u8]) -> Option<usize> {
    let mut at = files_start(header)?;
    loop {
        let line_end = at + header.get(at..)?.iter().position(|&b| b == b'\n')?;
        if &header[at..line_end] == b"data" { return Some(line_end + 1); }
        at = line_end + 1;
    }
}

// The file `path`'s offset from the start of the data, its size and its SHA-256, by the header's file lines in order.
fn locate(header: &[u8], path: &str) -> Option<(u64, u64, [u8; 32])> {
    let text = core::str::from_utf8(&header[files_start(header)?..]).ok()?;
    let mut offset = 0u64;
    for line in text.lines().filter(|l| l.starts_with("file ")) {
        let mut parts = line[5..].splitn(3, ' ');
        let (size, digest, p) = (parts.next()?.parse::<u64>().ok()?, parts.next()?, parts.next()?);
        if p == path {
            let mut out = [0u8; 32];
            for (i, pair) in digest.as_bytes().chunks_exact(2).enumerate().take(32) { out[i] = u8::from_str_radix(core::str::from_utf8(pair).ok()?, 16).ok()?; }
            return Some((offset, size, out));
        }
        offset += size;
    }
    None
}
