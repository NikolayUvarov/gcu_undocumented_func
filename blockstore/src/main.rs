#![no_std]
#![no_main]
// Block store (issue 300-STO-0002, docs/storage; MC-4.2, 4.8): immutable blocks named by their CID in an append-only
// log on a block device (store.rs); nothing stored is overwritten and every block read is checked against its CID.
// Serves idl/blockstore.wit to the rights in each client's badge (mind::blockstore, 300-STO-0004); every refusal is
// logged. Holds: a block client with the write badge in slot 2, a RAM disk of its own (issues/requests-KRN.md).
mod store;

// store.rs names these as crate::cid, crate::dag and crate::sha256, so the host tests build it from the libmind files.
use mind::{cid, dag, sha256};

use cid::Cid;
use mind::abi::BootInfo;
use mind::block::Device as Client;
use mind::blockstore::{allowed, Operation};
use mind::idl::blockstore::{self, Error, Head as Current, Request, Stats};
use mind::idl::codec::List;
use mind::idl::wire;
use mind::ipc::Endpoint;
use store::{Device, Entry, Head, Store, BLOCK_MAX, BUFFER, SECTOR};

const RECEIVED: usize = 9;
/// The block client init grants.
const BLOCK: usize = 2;
/// Blocks the index holds (48 bytes each).
const CAPACITY: usize = 4096;
/// Names the store holds (120 bytes each).
const NAMES: usize = 256;

static mut INDEX: [Entry; CAPACITY] = [Entry::EMPTY; CAPACITY];
static mut HEADS: [Head; NAMES] = [Head::EMPTY; NAMES];
static mut RECORD: [u8; BUFFER] = [0; BUFFER];
static mut SCRATCH: [u8; blockstore::REQUEST_MAX] = [0; blockstore::REQUEST_MAX];
static mut OUT: [u8; BLOCK_MAX] = [0; BLOCK_MAX];

/// The block client as the store's medium.
struct Medium(Client);
impl Device for Medium {
    fn sectors(&self) -> u64 { self.0.sectors() }
    fn writable(&self) -> bool { !self.0.read_only() }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool {
        match self.0.read(lba, out.len() / SECTOR) {
            Ok(data) if data.len() == out.len() => { out.copy_from_slice(data); true }
            _ => false,
        }
    }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool { self.0.write(lba, data) == Ok(data.len() / SECTOR) }
    fn flush(&mut self) -> bool { self.0.flush().is_ok() }
}

fn error(e: store::Error) -> Error {
    match e {
        store::Error::NotFound => Error::NotFound,
        store::Error::Corrupt => Error::Corrupt,
        store::Error::Full => Error::Full,
        store::Error::TooLarge => Error::TooLarge,
        store::Error::ReadOnly => Error::ReadOnly,
        store::Error::Invalid => Error::Invalid,
        store::Error::Conflict => Error::Conflict,
        store::Error::Incomplete => Error::Incomplete,
        store::Error::Device | store::Error::Foreign | store::Error::Layout => Error::Device,
    }
}

fn parse(bytes: &[u8]) -> Result<Cid, Error> { Cid::from_bytes(bytes).map_err(|_| Error::Unsupported) }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (index, heads, record, scratch, out) = unsafe {
        (&mut *core::ptr::addr_of_mut!(INDEX), &mut *core::ptr::addr_of_mut!(HEADS), &mut *core::ptr::addr_of_mut!(RECORD),
         &mut *core::ptr::addr_of_mut!(SCRATCH), &mut *core::ptr::addr_of_mut!(OUT))
    };
    let mounted = match Client::open(Endpoint(BLOCK)) {
        Ok(client) => Store::mount(Medium(client), index, heads, record),
        Err(_) => Err(store::Error::Device),
    };
    // Without a medium every request is answered with the reason, so clients are not left waiting.
    let mut store = match mounted {
        Ok(store) => {
            let s = store.stats();
            mind::println!("[BLOCKSTORE] READY BLOCKS={} NAMES={} SECTORS={}/{} CORRUPT={} DAMAGED={}", s.blocks, s.names, s.used, s.sectors, s.corrupt, s.damaged);
            Ok(store)
        }
        Err(e) => { mind::println!("[BLOCKSTORE] NOT MOUNTED: {:?}", e); Err(error(e)) }
    };
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        if !request.is_call { continue; }
        let (badge, pid) = (request.badge, request.sender);
        let refused = |operation| {
            if allowed(badge, operation) { return false; }
            mind::println!("[BLOCKSTORE] REFUSED {:?} FOR PID {} (BADGE {})", operation, pid, badge);
            true
        };
        let _ = match blockstore::decode(&request, RECEIVED, scratch) {
            Err(reason) => wire::reject(reason),
            Ok((Request::Put { .. }, call)) if refused(Operation::Put) => blockstore::reply_put(call, Err(Error::Rights)),
            Ok((Request::Get { .. }, call)) if refused(Operation::Get) => blockstore::reply_get(call, Err(Error::Rights)),
            Ok((Request::Has { .. }, call)) if refused(Operation::Has) => blockstore::reply_has(call, Err(Error::Rights)),
            Ok((Request::Stat, call)) if refused(Operation::Stat) => blockstore::reply_stat(call, Err(Error::Rights)),
            Ok((Request::Publish { .. }, call)) if refused(Operation::Publish) => blockstore::reply_publish(call, Err(Error::Rights)),
            Ok((Request::Resolve { .. }, call)) if refused(Operation::Resolve) => blockstore::reply_resolve(call, Err(Error::Rights)),
            Ok((Request::Put { codec, data }, call)) => {
                let codec = match codec { blockstore::Codec::Raw => cid::Codec::Raw, blockstore::Codec::DagCbor => cid::Codec::DagCbor };
                let cid = store.as_mut().map_err(|e| *e).and_then(|s| s.put(codec, data).map_err(error)).map(|c| c.to_bytes());
                blockstore::reply_put(call, cid.as_ref().map(|c| &c[..]).map_err(|e| *e))
            }
            Ok((Request::Get { cid }, call)) => {
                let got = parse(cid).and_then(|cid| {
                    let s = store.as_mut().map_err(|e| *e)?;
                    s.get(&cid, out).map_err(|e| {
                        if e == store::Error::Corrupt { mind::println!("[BLOCKSTORE] CORRUPT {}", cid); }
                        error(e)
                    })
                });
                blockstore::reply_get(call, got.map(|len| &out[..len]))
            }
            Ok((Request::Has { cid }, call)) => {
                let held = parse(cid).and_then(|cid| store.as_ref().map_err(|e| *e).map(|s| s.has(&cid)));
                blockstore::reply_has(call, held)
            }
            Ok((Request::Stat, call)) => {
                let s = store.as_ref().map(|s| s.stats()).unwrap_or_default();
                let stats = Stats { blocks: s.blocks, bytes: s.bytes, used: s.used, sectors: s.sectors, corrupt: s.corrupt, damaged: s.damaged, capacity: s.capacity, names: s.names };
                blockstore::reply_stat(call, Ok(&stats))
            }
            Ok((Request::Publish { name, expected, root }, call)) => {
                let version = parse(root).and_then(|root| {
                    let s = store.as_mut().map_err(|e| *e)?;
                    let version = s.publish(name.as_str().as_bytes(), expected, &root, out).map_err(error)?;
                    mind::println!("[BLOCKSTORE] PUBLISHED {} VERSION {} ROOT {} BY PID {}", name, version, root, pid);
                    Ok(version)
                });
                blockstore::reply_publish(call, version)
            }
            Ok((Request::Resolve { name }, call)) => {
                let head = store.as_ref().map_err(|e| *e).and_then(|s| s.resolve(name.as_str().as_bytes()).map_err(error));
                let head = head.map(|(version, root)| Current { version, root: List::from_slice(&root.to_bytes()).unwrap_or_default() });
                blockstore::reply_resolve(call, head.as_ref().map_err(|e| *e))
            }
        };
    }
}
