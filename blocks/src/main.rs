#![no_std]
#![no_main]
// blocks: the block store from the shell (300-STO-0003, docs/storage). Puts a file or a test pattern as an object
// (mind::dag), reads one back with every block checked, publishes and resolves names, fills the store. A console
// program: it asks the shell for the store's client (REQUEST_BLOCKSTORE) and the user's files (REQUEST_FILES).
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_BLOCKSTORE, SLOT_FILE};
use mind::cid::{Cid, Codec};
use mind::dag::{self, Blocks, Builder, CHUNK};
use mind::fs::File;
use mind::idl::blockstore::{self, Error};
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_FILES | REQUEST_BLOCKSTORE);

const STORE: Endpoint = Endpoint(SLOT_BLOCKSTORE);

static mut BUILDER: Builder = Builder::new();
static mut BUFFER: [u8; CHUNK] = [0; CHUNK];
static mut DATA: [u8; CHUNK] = [0; CHUNK];

/// The store's client as the blocks of `dag`; what the store says is kept for the message.
struct Remote { last: Option<Error> }
impl Remote {
    fn fail(&mut self, error: Error) -> dag::Error {
        self.last = Some(error);
        match error { Error::NotFound => dag::Error::NotFound, Error::Corrupt => dag::Error::Corrupt, Error::Full => dag::Error::Full, _ => dag::Error::Store }
    }
}
impl Blocks for Remote {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> {
        let codec = match codec { Codec::Raw => blockstore::Codec::Raw, Codec::DagCbor => blockstore::Codec::DagCbor };
        let mut out = [0u8; 36];
        match blockstore::put(STORE, codec, data, &mut out) {
            Ok(Ok(n)) => Cid::from_bytes(&out[..n]).map_err(|_| dag::Error::Store),
            Ok(Err(e)) => Err(self.fail(e)),
            Err(_) => Err(dag::Error::Store),
        }
    }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> {
        match blockstore::get(STORE, &cid.to_bytes(), out) { Ok(Ok(n)) => Ok(n), Ok(Err(e)) => Err(self.fail(e)), Err(_) => Err(dag::Error::Store) }
    }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> {
        match blockstore::has(STORE, &cid.to_bytes()) { Ok(Ok(h)) => Ok(h), Ok(Err(e)) => Err(self.fail(e)), Err(_) => Err(dag::Error::Store) }
    }
}

// The reference pattern of tests/dag_host.rs and the Python reference: byte i is (i * 31 + 7) mod 251.
fn pattern(at: u64, out: &mut [u8]) { for (i, b) in out.iter_mut().enumerate() { *b = ((at + i as u64) * 31 + 7).rem_euclid(251) as u8; } }

fn report(what: &str, error: dag::Error, remote: &Remote) {
    match remote.last { Some(e) => mind::println!("blocks: {}: {:?} (the store: {:?})", what, error, e), None => mind::println!("blocks: {}: {:?}", what, error) }
}

fn cid(text: &str) -> Option<Cid> {
    let cid = Cid::from_text(text.as_bytes()).ok();
    if cid.is_none() { mind::println!("blocks: not a CID: {}", text); }
    cid
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("blocks — the block store: objects by content (CID), names, statistics.\nUsage: blocks stat | put <file> | pattern <bytes> | get <cid> <file> | check <cid> [pattern] | publish <name> <cid> [expected version] | resolve <name> | fill");
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { mind::fs::use_endpoint(Endpoint(SLOT_FILE)); }
    if mind::dev::cap_info(SLOT_BLOCKSTORE).0 != CAP_KIND_ENDPOINT { mind::println!("blocks: no client of the block store"); return; }
    let (builder, buffer, data) = unsafe { (&mut *core::ptr::addr_of_mut!(BUILDER), &mut *core::ptr::addr_of_mut!(BUFFER), &mut *core::ptr::addr_of_mut!(DATA)) };
    let mut remote = Remote { last: None };
    let mut words = mind::process::args_str().split_whitespace();
    match (words.next(), words.next(), words.next(), words.next()) {
        (Some("stat"), None, ..) => match blockstore::stat(STORE) {
            Ok(Ok(s)) => mind::println!("BLOCKS={} NAMES={} BYTES={} SECTORS={}/{} CORRUPT={} DAMAGED={} CAPACITY={}", s.blocks, s.names, s.bytes, s.used, s.sectors, s.corrupt, s.damaged, s.capacity),
            Ok(Err(e)) => mind::println!("blocks: stat: {:?}", e),
            Err(e) => mind::println!("blocks: the store does not answer: {:?}", e),
        },
        (Some("put"), Some(path), None, _) => {
            let Ok(mut file) = File::open(path) else { mind::println!("blocks: cannot open {}", path); return };
            loop {
                let n = match file.read(data) { Ok(n) => n, Err(e) => { mind::println!("blocks: reading {}: {:?}", path, e); return } };
                if n == 0 { break; }
                if let Err(e) = builder.write(&mut remote, &data[..n]) { return report("put", e, &remote); }
            }
            match builder.finish(&mut remote) { Ok((root, size)) => mind::println!("PUT {} SIZE {}", root, size), Err(e) => report("put", e, &remote) }
        }
        (Some("pattern"), Some(bytes), None, _) => {
            let Ok(total) = bytes.parse::<u64>() else { mind::println!("blocks: not a size: {}", bytes); return };
            let mut at = 0;
            while at < total {
                let n = (total - at).min(CHUNK as u64) as usize;
                pattern(at, &mut data[..n]);
                if let Err(e) = builder.write(&mut remote, &data[..n]) { return report("pattern", e, &remote); }
                at += n as u64;
            }
            match builder.finish(&mut remote) { Ok((root, size)) => mind::println!("PUT {} SIZE {}", root, size), Err(e) => report("pattern", e, &remote) }
        }
        (Some("get"), Some(text), Some(path), None) => {
            let Some(root) = cid(text) else { return };
            let size = match dag::size(&mut remote, &root, buffer) { Ok(size) => size, Err(e) => return report("get", e, &remote) };
            let Ok(mut file) = File::create(path) else { mind::println!("blocks: cannot create {}", path); return };
            let mut at = 0;
            while at < size {
                let n = match dag::read_at(&mut remote, &root, at, data, buffer) { Ok(n) => n, Err(e) => return report("get", e, &remote) };
                if file.write(&data[..n]).ok() != Some(n) { mind::println!("blocks: writing {} failed", path); return; }
                at += n as u64;
            }
            let _ = file.flush();
            mind::println!("GOT {} BYTES", size);
        }
        (Some("check"), Some(text), compare, None) => {
            let Some(root) = cid(text) else { return };
            let against = match compare { None => false, Some("pattern") => true, Some(other) => { mind::println!("blocks: check <cid> [pattern], not {}", other); return } };
            let size = match dag::complete(&mut remote, &root, buffer) { Ok(size) => size, Err(e) => return report("check", e, &remote) };
            let mut at = 0;
            while at < size {
                let n = match dag::read_at(&mut remote, &root, at, data, buffer) { Ok(n) => n, Err(e) => return report("check", e, &remote) };
                if against {
                    let mut expected = [0u8; 256];
                    for (k, part) in data[..n].chunks(256).enumerate() {
                        pattern(at + (k * 256) as u64, &mut expected[..part.len()]);
                        if part != &expected[..part.len()] { mind::println!("blocks: check: differs from the pattern near byte {}", at + (k * 256) as u64); return; }
                    }
                }
                at += n as u64;
            }
            mind::println!("CHECKED {} BYTES{}", size, if against { " = PATTERN" } else { "" });
        }
        (Some("publish"), Some(name), Some(text), expected) => {
            let Some(root) = cid(text) else { return };
            let expected = match expected.map(|e| e.parse::<u64>()) { None => 0, Some(Ok(v)) => v, Some(Err(_)) => { mind::println!("blocks: not a version"); return } };
            match blockstore::publish(STORE, name, expected, &root.to_bytes()) {
                Ok(Ok(version)) => mind::println!("PUBLISHED {} VERSION {}", name, version),
                Ok(Err(e)) => mind::println!("blocks: publish {}: {:?}", name, e),
                Err(e) => mind::println!("blocks: the store does not answer: {:?}", e),
            }
        }
        (Some("resolve"), Some(name), None, _) => match blockstore::resolve(STORE, name) {
            Ok(Ok(head)) => match Cid::from_bytes(head.root.as_slice()) {
                Ok(root) => mind::println!("{} VERSION {} ROOT {}", name, head.version, root),
                Err(_) => mind::println!("blocks: resolve {}: the store answered no CID", name),
            },
            Ok(Err(e)) => mind::println!("blocks: resolve {}: {:?}", name, e),
            Err(e) => mind::println!("blocks: the store does not answer: {:?}", e),
        },
        (Some("fill"), None, ..) => {
            // Distinct blocks of CHUNK bytes until the store refuses one: what a full medium answers.
            let mut count = 0u64;
            loop {
                data.fill(0xA5);
                data[..8].copy_from_slice(&count.to_le_bytes());
                match remote.put(Codec::Raw, data) {
                    Ok(_) => count += 1,
                    Err(e) => { mind::println!("FILLED {} BLOCKS, THEN {:?} ({:?})", count, e, remote.last); break; }
                }
            }
        }
        _ => mind::println!("Usage: blocks stat | put <file> | pattern <bytes> | get <cid> <file> | check <cid> [pattern] | publish <name> <cid> [expected version] | resolve <name> | fill"),
    }
}
