#![no_std]
#![no_main]
// Block store (issue 300-STO-0002, docs/storage; MC-4.2, 4.8): immutable blocks named by their CID in an append-only
// log on a block device (store.rs); nothing stored is overwritten and every block read is checked against its CID.
// Serves idl/blockstore.wit to the rights in each client's badge (mind::blockstore, 300-STO-0004); every refusal is
// logged. Leases are measured on the monotonic clock (303-STO-0001); a commit changes several names at once (304-STO-0007);
// the current names are listed by prefix, 16 at a time (300-STO-0018); each write says how far it reached (307-STO-0019).
// A medium it cannot mount leaves it running and answering every request with the reason (305-STO-0008). Holds: a block client with the write badge in slot 2, a RAM disk of its own (issues/requests-KRN.md).
// Records are sealed with ChaCha20-Poly1305 under a volume key (308-STO-0021), kept by keystore as the store's secret
// where init lends its client, else in the superblock in the clear (308-STO-0022).
// A memory domain's calls come through a client the store mints for it, and work under the domain's own key (308-STO-0023).
// Between requests it scrubs the medium, a step at a time, a pass a day; a client may ask for a whole pass (309-STO-0025).
mod store;

// store.rs names these as crate::cid, crate::dag, crate::sha256 and crate::tree, so the host tests build it from the
// libmind files.
use mind::{cid, dag, sha256, tree};

use chacha20poly1305::aead::{AeadInPlace, KeyInit};
use chacha20poly1305::ChaCha20Poly1305;
use cid::Cid;
use mind::abi::{BootInfo, CAP_GRANT, CAP_KIND_ENDPOINT, CAP_WRITE, SLOT_SERVICE};
use mind::block::Device as Client;
use mind::blockstore::{allowed, Operation, FIRST_DOMAIN_CLIENT};
use mind::idl::blockstore::{self, Collected, Committed, Durability, Error, Head as Current, Kept, Named, PinTaken, Pinned, Published, Request, Stats, Usage};
use mind::idl::codec::{List, Text};
use mind::idl::wire;
use mind::ipc::{self, Endpoint};
use store::{slots_for, Crypto, Device, Entry, Extent, Head, Keeper, Pin, Store, Update, BLOCK_MAX, BUFFER, COMMIT_MAX, DOMAINS, KEY_KEPT, LAYOUT, SECTOR, VOLUME};

const RECEIVED: usize = 9;
/// The block client init grants.
const BLOCK: usize = 2;
/// The secrets client init lends the store, when keystore gives one (308-KRN-0183).
const SECRETS: usize = 3;
/// Names the store holds (120 bytes each).
const NAMES: usize = 256;
/// Runs of blank sectors the store keeps track of between scans.
const HOLES: usize = 512;
/// Pins the store holds.
const PINS: usize = 64;
/// Clients of memory domains the store keeps at once.
const DOMAIN_CLIENTS: usize = 64;
/// The scrub between requests (MC-5.1): the sectors a step covers (it ends at the end of the record or window it is in),
/// after a pause this long without a request, and a pass this long after the last one ended (the mount counts as one).
const SCRUB_STEP: u64 = 256;
const SCRUB_PAUSE_MS: u32 = 20;
const SCRUB_EVERY_NS: u64 = 24 * 3600 * 1_000_000_000;

static mut HEADS: [Head; NAMES] = [Head::EMPTY; NAMES];
static mut RUNS: [Extent; HOLES] = [Extent::EMPTY; HOLES];
static mut PINNED: [Pin; PINS] = [Pin::EMPTY; PINS];
static mut WALK: [u8; dag::CHUNK] = [0; dag::CHUNK];
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

/// ChaCha20-Poly1305 (RFC 8439) of the RustCrypto crate, and the processor's random source.
struct Sealing;
impl Crypto for Sealing {
    fn seal(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] {
        // Fails only for data past 256 GiB.
        ChaCha20Poly1305::new(key.into()).encrypt_in_place_detached(nonce.into(), aad, data).expect("a record fits the cipher").into()
    }
    fn open(&self, key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], tag: &[u8; 16]) -> bool {
        ChaCha20Poly1305::new(key.into()).decrypt_in_place_detached(nonce.into(), aad, data, tag.into()).is_ok()
    }
    fn random(&self, out: &mut [u8]) -> bool { mind::random::fill(out) }
}

// The volume key as the store's secret `volume-<id in hex>` in keystore (308-STO-0022).
struct Secrets(Endpoint);
impl Keeper for Secrets {
    fn put(&mut self, id: &[u8; 8], key: &[u8; 32]) -> bool {
        matches!(mind::idl::keystore::secret_put(self.0, secret_name(id).as_str(), key), Ok(Ok(())))
    }
    fn get(&mut self, id: &[u8; 8]) -> Option<[u8; 32]> {
        let mut key = [0u8; 32];
        matches!(mind::idl::keystore::secret_get(self.0, secret_name(id).as_str(), &mut key), Ok(Ok(32))).then_some(key)
    }
}

// The name a volume key is kept under: `volume-` and its id in hex.
fn secret_name(id: &[u8; 8]) -> Text<32> {
    let mut name = [0u8; 23];
    name[..7].copy_from_slice(b"volume-");
    for (k, b) in id.iter().enumerate() {
        name[7 + 2 * k] = b"0123456789abcdef"[(b >> 4) as usize];
        name[8 + 2 * k] = b"0123456789abcdef"[(b & 15) as usize];
    }
    Text::new(core::str::from_utf8(&name).unwrap_or("")).unwrap_or_default()
}

// A client minted for a memory domain (308-STO-0023): its badge and handle, its domain, and the rights and owner of the
// client that asked for it.
#[derive(Clone, Copy)]
struct DomainClient { badge: u16, cap: usize, domain: u16, rights: u16, owner: u16 }

struct DomainClients([Option<DomainClient>; DOMAIN_CLIENTS]);
impl DomainClients {
    // The rights, owner and domain a request's badge carries; a domain badge the store did not mint carries no right.
    fn of(&self, badge: u16) -> (u16, u16, u16) {
        if badge < FIRST_DOMAIN_CLIENT { return (badge, badge, VOLUME); }
        self.0.iter().flatten().find(|c| c.badge == badge).map_or((0, badge, VOLUME), |c| (c.rights, c.owner, c.domain))
    }
    // A client of `domain` with `rights` for `owner`: the one minted already, or a new one.
    fn client(&mut self, domain: u16, rights: u16, owner: u16) -> Result<usize, Error> {
        if let Some(c) = self.0.iter().flatten().find(|c| (c.domain, c.rights, c.owner) == (domain, rights, owner)) { return Ok(c.cap); }
        let slot = self.0.iter().position(Option::is_none).ok_or(Error::Full)?;
        let badge = FIRST_DOMAIN_CLIENT + slot as u16;
        // Minting a badged client needs the badge right on the store's own endpoint (init's grant, requests-KRN.md).
        let cap = ipc::mint_badged(SLOT_SERVICE, CAP_WRITE | CAP_GRANT, badge).map_err(|e| {
            mind::println!("[BLOCKSTORE] NO CLIENT FOR DOMAIN {}: {:?}", domain, e);
            Error::Device
        })?;
        self.0[slot] = Some(DomainClient { badge, cap, domain, rights, owner });
        Ok(cap)
    }
    // Every client of a forgotten domain revoked, wherever it was copied, and dropped.
    fn forget(&mut self, domain: u16) {
        for slot in self.0.iter_mut() {
            let Some(c) = slot.filter(|c| c.domain == domain) else { continue };
            let _ = ipc::revoke(c.cap);
            let _ = ipc::drop_cap(c.cap);
            *slot = None;
        }
    }
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
        store::Error::Quota => Error::Quota,
        store::Error::Rights => Error::Rights,
        store::Error::Key | store::Error::Kept(_) => Error::Key,
        store::Error::NoRandom => Error::NoRandom,
        store::Error::Device | store::Error::Foreign | store::Error::Layout(_) => Error::Device,
    }
}

fn parse(bytes: &[u8]) -> Result<Cid, Error> { Cid::from_bytes(bytes).map_err(|_| Error::Unsupported) }

// The scrubs' account: whole passes, the last one's findings and when it ended, and when the next starts (ns).
struct Scrubs { passes: u32, last: store::Scrubbed, ended: u64, next: u64 }
impl Scrubs {
    // A whole pass ended: between requests, or asked for by `pid`.
    fn ended(&mut self, found: store::Scrubbed, now: u64, pid: Option<u64>) {
        (self.passes, self.last, self.ended, self.next) = (self.passes + 1, found, now, now + SCRUB_EVERY_NS);
        let (records, corrupt, damaged) = (found.records, found.corrupt, found.damaged);
        match pid {
            Some(pid) => mind::println!("[BLOCKSTORE] SCRUBBED {} RECORDS: {} CORRUPT, {} DAMAGED SECTORS, ASKED BY PID {}", records, corrupt, damaged, pid),
            None => mind::println!("[BLOCKSTORE] SCRUBBED {} RECORDS: {} CORRUPT, {} DAMAGED SECTORS, BETWEEN REQUESTS", records, corrupt, damaged),
        }
    }
    fn report(&self, now: u64) -> blockstore::Scrubbed {
        let ago_ms = if self.passes == 0 { 0 } else { now.saturating_sub(self.ended) / 1_000_000 };
        blockstore::Scrubbed { passes: self.passes, records: self.last.records, corrupt: self.last.corrupt, damaged: self.last.damaged, ago_ms }
    }
}

// A record a scrub found that does not open or is not whole.
fn found_corrupt(lba: u64, block: Option<Cid>) {
    match block {
        Some(cid) => mind::println!("[BLOCKSTORE] SCRUB: BLOCK {} AT SECTOR {} CORRUPT", cid, lba),
        None => mind::println!("[BLOCKSTORE] SCRUB: RECORD AT SECTOR {} CORRUPT", lba),
    }
}

// The index's slots for a medium of `sectors` (251-STO-0013): as many as it asks for and the task's memory quota allows,
// halving down to the least; none if not even that can be had.
fn index_for(sectors: u64) -> &'static mut [Entry] {
    let mut slots = slots_for(sectors);
    loop {
        if let Some(mut pages) = mind::mem::Pages::new(slots * core::mem::size_of::<Entry>()) {
            // Page-aligned memory the store keeps for its life: all zeros is an empty slot, and mount empties it anyway.
            let index = unsafe { core::slice::from_raw_parts_mut(pages.as_mut_slice().as_mut_ptr() as *mut Entry, slots) };
            core::mem::forget(pages);
            mind::println!("[BLOCKSTORE] INDEX: {} SLOTS ({} KiB) FOR {} SECTORS", slots, slots * core::mem::size_of::<Entry>() / 1024, sectors);
            return index;
        }
        if slots == slots_for(0) { break; }
        slots = (slots / 2).max(slots_for(0));
    }
    mind::println!("[BLOCKSTORE] NO MEMORY FOR AN INDEX OF {} SLOTS", slots);
    &mut []
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (heads, pinned, runs, record, walk, scratch, out) = unsafe {
        (&mut *core::ptr::addr_of_mut!(HEADS), &mut *core::ptr::addr_of_mut!(PINNED), &mut *core::ptr::addr_of_mut!(RUNS),
         &mut *core::ptr::addr_of_mut!(RECORD), &mut *core::ptr::addr_of_mut!(WALK), &mut *core::ptr::addr_of_mut!(SCRATCH),
         &mut *core::ptr::addr_of_mut!(OUT))
    };
    // How far a write reaches is the medium's: nothing outlives a reset on a RAM disk; elsewhere the driver acknowledged
    // a flush after it, and what that reaches is the driver's to state (307-STO-0019).
    let mut durability = Durability::Volatile;
    // A RAM disk keeps its key in its own superblock, as nothing of it outlives a reset; any other medium's key is kept
    // by keystore, when init lent its client (308-STO-0022).
    let lent = mind::dev::cap_info(SECRETS).0 == CAP_KIND_ENDPOINT;
    let mut ram = true;
    let mut secrets = Secrets(Endpoint(SECRETS));
    let mounted = match Client::open(Endpoint(BLOCK)) {
        Ok(client) => {
            ram = client.kind() == mind::block::KIND_RAM;
            if !ram { durability = Durability::Flushed; }
            let index = index_for(client.sectors());
            let keeper = (!ram && lent).then_some(&mut secrets as &mut dyn Keeper);
            Store::mount(Medium(client), &Sealing, keeper, index, heads, pinned, runs, record, walk, mind::time::monotonic_ns())
        }
        Err(_) => Err(store::Error::Device),
    };
    // Without a medium every request is answered with the reason, so clients are not left waiting.
    let mut store = match mounted {
        Ok(mut store) => {
            store.renew(mind::time::monotonic_ns());
            let s = store.stats();
            // Where a key made now went, and why not off the medium.
            let kept = store.key_source() == KEY_KEPT;
            let place = match (kept, ram, lent, store.made()) {
                (true, ..) => "KEPT BY KEYSTORE, NOT ON THE MEDIUM",
                (_, true, ..) => "IN THE SUPERBLOCK: A RAM DISK OUTLIVES NO RESET",
                (_, _, false, _) => "IN THE SUPERBLOCK: NO SECRETS CLIENT",
                (.., Some(false)) => "IN THE SUPERBLOCK: A DERIVED KEY IS NOT KEPT",
                _ => "IN THE SUPERBLOCK: KEYSTORE DID NOT KEEP IT",
            };
            match store.made() {
                Some(true) => mind::println!("[BLOCKSTORE] VOLUME KEY MADE FROM {}, {}", mind::random::SOURCE, place),
                Some(false) => mind::println!("[BLOCKSTORE] VOLUME KEY MADE WITHOUT {}: DERIVED FROM THE MEDIUM AND THE TIME, {}", mind::random::SOURCE, place),
                None => {}
            }
            // A key in the superblock opens the records for whoever reads the medium; a kept one needs keystore too.
            mind::println!("[BLOCKSTORE] READY BLOCKS={} NAMES={} SECTORS={}/{} CORRUPT={} DAMAGED={} DURABILITY={} SEALED, {}",
                s.blocks, s.names, s.used, s.sectors, s.corrupt, s.damaged, mind::blockstore::class(durability as u8),
                if kept { "KEY KEPT BY KEYSTORE" } else { "KEY IN THE CLEAR" });
            for d in store.domains() {
                if d.forgotten { mind::println!("[BLOCKSTORE] DOMAIN {} FORGOTTEN: {} RECORDS LEFT FOR THE NEXT COLLECTION", d.id, d.records); }
                else { mind::println!("[BLOCKSTORE] DOMAIN {} OF BADGE {}: {} RECORDS", d.id, d.owner, d.records); }
            }
            Ok(store)
        }
        Err(store::Error::Kept(id)) => {
            let why = if lent { "WHICH KEEPS NONE UNDER IT: ANOTHER MACHINE'S MEDIUM, OR ITS SECRET LOST" } else { "AND NO SECRETS CLIENT WAS LENT" };
            mind::println!("[BLOCKSTORE] NOT MOUNTED: ITS VOLUME KEY IS KEPT BY KEYSTORE AS {}, {}", secret_name(&id), why);
            Err(Error::Key)
        }
        Err(store::Error::Layout(found)) => {
            mind::println!("[BLOCKSTORE] NOT MOUNTED: A STORE OF LAYOUT {}; THIS BUILD READS LAYOUT {} AND CONVERTS NONE", found, LAYOUT);
            Err(Error::Device)
        }
        Err(e) => { mind::println!("[BLOCKSTORE] NOT MOUNTED: {:?}", e); Err(error(e)) }
    };
    let mut clients = DomainClients([None; DOMAIN_CLIENTS]);
    let mut scrubs = Scrubs { passes: 0, last: store::Scrubbed::default(), ended: 0, next: mind::time::monotonic_ns() + SCRUB_EVERY_NS };
    loop {
        // A pass under way goes on after a pause without requests; the next one starts when it is due.
        let now = mind::time::monotonic_ns();
        let wait = match store.as_ref() {
            Ok(s) if s.scrubbing() => SCRUB_PAUSE_MS,
            Ok(_) => (scrubs.next.saturating_sub(now) / 1_000_000).clamp(1, u32::MAX as u64) as u32,
            Err(_) => 0,
        };
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, wait) else {
            let now = mind::time::monotonic_ns();
            if let Some(s) = store.as_mut().ok().filter(|s| s.scrubbing() || now >= scrubs.next) {
                match s.scrub(SCRUB_STEP, &mut found_corrupt) {
                    Ok(Some(found)) => scrubs.ended(found, now, None),
                    Ok(None) => {}
                    Err(e) => {
                        mind::println!("[BLOCKSTORE] SCRUB STOPPED: {:?}", e);
                        s.restart_scrub();
                        scrubs.next = now + SCRUB_EVERY_NS;
                    }
                }
            }
            continue;
        };
        if !request.is_call { continue; }
        let (badge, pid) = (request.badge, request.sender);
        let (mut rights, owner, domain) = clients.of(badge);
        // Each call works in its client's domain; one that cannot (its domain is gone) may do nothing.
        if let Ok(s) = store.as_mut() {
            s.set_time(mind::time::monotonic_ns());
            if s.set_domain(domain).is_err() { rights = 0; }
        }
        let refused = |operation| {
            if allowed(rights, operation) { return false; }
            mind::println!("[BLOCKSTORE] REFUSED {:?} FOR PID {} (BADGE {})", operation, pid, badge);
            true
        };
        // Domains are made, lent and forgotten only through a client of the volume.
        let through_domain = |operation| {
            if badge < FIRST_DOMAIN_CLIENT { return false; }
            mind::println!("[BLOCKSTORE] REFUSED {:?} FOR PID {}: THROUGH DOMAIN {}'S CLIENT", operation, pid, domain);
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
            Ok((Request::Collect, call)) if refused(Operation::Collect) => blockstore::reply_collect(call, Err(Error::Rights)),
            Ok((Request::Unpublish { .. }, call)) if refused(Operation::Unpublish) => blockstore::reply_unpublish(call, Err(Error::Rights)),
            Ok((Request::History { .. }, call)) if refused(Operation::History) => blockstore::reply_history(call, Err(Error::Rights)),
            Ok((Request::Pin { .. }, call)) if refused(Operation::Pin) => blockstore::reply_pin(call, Err(Error::Rights)),
            Ok((Request::Unpin { .. }, call)) if refused(Operation::Unpin) => blockstore::reply_unpin(call, Err(Error::Rights)),
            Ok((Request::Pins, call)) if refused(Operation::Pins) => blockstore::reply_pins(call, Err(Error::Rights)),
            Ok((Request::Usage, call)) if refused(Operation::Usage) => blockstore::reply_usage(call, Err(Error::Rights)),
            Ok((Request::Commit { .. }, call)) if refused(Operation::Commit) => blockstore::reply_commit(call, Err(Error::Rights)),
            Ok((Request::Snapshot { .. }, call)) if refused(Operation::Snapshot) => blockstore::reply_snapshot(call, Err(Error::Rights)),
            Ok((Request::Names { .. }, call)) if refused(Operation::Names) => blockstore::reply_names(call, Err(Error::Rights)),
            Ok((Request::DomainCreate, call)) if refused(Operation::DomainCreate) || through_domain(Operation::DomainCreate) => blockstore::reply_domain_create(call, Err(Error::Rights)),
            Ok((Request::DomainClient { .. }, call)) if refused(Operation::DomainClient) || through_domain(Operation::DomainClient) => blockstore::reply_domain_client(call, Err(Error::Rights)),
            Ok((Request::DomainForget { .. }, call)) if refused(Operation::DomainForget) || through_domain(Operation::DomainForget) => blockstore::reply_domain_forget(call, Err(Error::Rights)),
            Ok((Request::Domains, call)) if refused(Operation::Domains) || through_domain(Operation::Domains) => blockstore::reply_domains(call, Err(Error::Rights)),
            Ok((Request::Scrub, call)) if refused(Operation::Scrub) => blockstore::reply_scrub(call, Err(Error::Rights)),
            Ok((Request::Scrubbed, call)) if refused(Operation::Scrubbed) => blockstore::reply_scrubbed(call, Err(Error::Rights)),
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
                // Unmounted, the store says why rather than report an empty store (305-STO-0008).
                let stats = store.as_ref().map_err(|e| *e).map(|s| {
                    let s = s.stats();
                    Stats { blocks: s.blocks, bytes: s.bytes, used: s.used, sectors: s.sectors, corrupt: s.corrupt, damaged: s.damaged, capacity: s.capacity, names: s.names, durability }
                });
                blockstore::reply_stat(call, stats.as_ref().map_err(|e| *e))
            }
            Ok((Request::Collect, call)) => {
                let collected = store.as_mut().map_err(|e| *e).and_then(|s| s.collect().map_err(error)).map(|c| {
                    mind::println!("[BLOCKSTORE] COLLECTED {} BLOCKS {} NAMES {} SECTORS, {} FREE, BY PID {}", c.blocks, c.names, c.sectors, c.free, pid);
                    Collected { blocks: c.blocks, names: c.names, sectors: c.sectors, free: c.free }
                });
                blockstore::reply_collect(call, collected.as_ref().map_err(|e| *e))
            }
            Ok((Request::Publish { name, expected, root }, call)) => {
                let version = parse(root).and_then(|root| {
                    let s = store.as_mut().map_err(|e| *e)?;
                    let version = s.publish(name.as_str().as_bytes(), expected, &root, owner).map_err(error)?;
                    mind::println!("[BLOCKSTORE] PUBLISHED {} VERSION {} ROOT {} BY PID {}", name, version, root, pid);
                    Ok(Published { version, durability })
                });
                blockstore::reply_publish(call, version.as_ref().map_err(|e| *e))
            }
            Ok((Request::Resolve { name }, call)) => {
                let head = store.as_ref().map_err(|e| *e).and_then(|s| s.resolve(name.as_str().as_bytes()).map_err(error));
                let head = head.map(|(version, root)| Current { version, root: List::from_slice(&root.to_bytes()).unwrap_or_default() });
                blockstore::reply_resolve(call, head.as_ref().map_err(|e| *e))
            }
            Ok((Request::Unpublish { name, expected }, call)) => {
                let version = store.as_mut().map_err(|e| *e).and_then(|s| s.unpublish(name.as_str().as_bytes(), expected, owner).map_err(error));
                if let Ok(v) = version { mind::println!("[BLOCKSTORE] REMOVED {} VERSION {} BY PID {}", name, v, pid); }
                blockstore::reply_unpublish(call, version.map(|version| Published { version, durability }).as_ref().map_err(|e| *e))
            }
            Ok((Request::History { name }, call)) => {
                let mut kept = [Kept::default(); store::HISTORY];
                let n = store.as_ref().map_err(|e| *e).and_then(|s| s.history(name.as_str().as_bytes()).map_err(error)).map(|versions| {
                    for (k, v) in kept.iter_mut().zip(versions) {
                        *k = Kept { version: v.version, root: v.root().map(|r| List::from_slice(&r.to_bytes()).unwrap_or_default()).unwrap_or_default() };
                    }
                    versions.len()
                });
                blockstore::reply_history(call, n.map(|n| &kept[..n]))
            }
            Ok((Request::Pin { root }, call)) => {
                let id = parse(root).and_then(|root| {
                    let id = store.as_mut().map_err(|e| *e)?.pin(&root, owner).map_err(error)?;
                    mind::println!("[BLOCKSTORE] PINNED {} AS {} BY PID {} (BADGE {})", root, id, pid, badge);
                    Ok(PinTaken { id, durability })
                });
                blockstore::reply_pin(call, id.as_ref().map_err(|e| *e))
            }
            Ok((Request::Unpin { id }, call)) => {
                let done = store.as_mut().map_err(|e| *e).and_then(|s| s.unpin(id, owner).map_err(error));
                if done.is_ok() { mind::println!("[BLOCKSTORE] UNPINNED {} BY PID {}", id, pid); }
                blockstore::reply_unpin(call, done)
            }
            Ok((Request::Pins, call)) => {
                let mut list = [Pinned::default(); 32];
                let n = store.as_ref().map_err(|e| *e).map(|s| {
                    let mut n = 0;
                    for (slot, p) in list.iter_mut().zip(s.pins(owner)) {
                        *slot = Pinned { id: p.id, root: List::from_slice(&p.root().to_bytes()).unwrap_or_default(), size: p.size };
                        n += 1;
                    }
                    n
                });
                blockstore::reply_pins(call, n.map(|n| &list[..n]))
            }
            Ok((Request::Commit { updates }, call)) => {
                let updates = updates.as_slice();
                // An empty root removes the name; any other must be a supported CID.
                let mut changes = [Update { name: &[], expected: 0, root: None }; COMMIT_MAX];
                let parsed = updates.iter().zip(changes.iter_mut()).try_for_each(|(u, c)| {
                    let root = if u.root.is_empty() { None } else { Some(parse(u.root.as_slice())?) };
                    *c = Update { name: u.name.as_str().as_bytes(), expected: u.expected, root };
                    Ok(())
                });
                let versions = parsed.and_then(|()| {
                    let s = store.as_mut().map_err(|e| *e)?;
                    let versions = s.commit(&changes[..updates.len()], owner).map_err(error)?;
                    for (u, v) in updates.iter().zip(versions) {
                        match changes.iter().find(|c| c.name == u.name.as_str().as_bytes()).and_then(|c| c.root) {
                            Some(root) => mind::println!("[BLOCKSTORE] COMMITTED {} VERSION {} ROOT {} BY PID {}", u.name, v, root, pid),
                            None => mind::println!("[BLOCKSTORE] COMMITTED {} VERSION {} REMOVED BY PID {}", u.name, v, pid),
                        }
                    }
                    Ok(versions)
                });
                let committed = versions.map(|v| Committed { versions: List::from_slice(&v[..updates.len()]).unwrap_or_default(), durability });
                blockstore::reply_commit(call, committed.as_ref().map_err(|e| *e))
            }
            Ok((Request::Snapshot { names }, call)) => {
                let mut heads = [Current::default(); COMMIT_MAX];
                let read = store.as_ref().map_err(|e| *e).and_then(|s| {
                    for (h, name) in heads.iter_mut().zip(names.as_slice()) {
                        let (version, root) = s.snapshot(name.as_str().as_bytes()).map_err(error)?;
                        *h = Current { version, root: root.map(|r| List::from_slice(&r.to_bytes()).unwrap_or_default()).unwrap_or_default() };
                    }
                    Ok(names.len())
                });
                blockstore::reply_snapshot(call, read.map(|n| &heads[..n]))
            }
            Ok((Request::Names { prefix, after }, call)) => {
                let mut page = [Named::default(); 16];
                let n = store.as_ref().map_err(|e| *e).and_then(|s| s.names(prefix.as_str().as_bytes(), after.as_str().as_bytes()).map_err(error)).map(|names| {
                    let mut n = 0;
                    // A stored name is 1 to 64 bytes of ASCII, so it is always text of at most 64.
                    for (slot, (name, version, root)) in page.iter_mut().zip(names) {
                        *slot = Named { name: Text::new(core::str::from_utf8(name).unwrap_or("")).unwrap_or_default(), version, root: List::from_slice(&root.to_bytes()).unwrap_or_default() };
                        n += 1;
                    }
                    n
                });
                blockstore::reply_names(call, n.map(|n| &page[..n]))
            }
            Ok((Request::Usage, call)) => {
                let usage = store.as_ref().map_err(|e| *e).map(|s| {
                    let u = s.usage(owner);
                    Usage { retained: u.retained, quota: u.quota, names: u.names, pins: u.pins }
                });
                blockstore::reply_usage(call, usage.as_ref().map_err(|e| *e))
            }
            Ok((Request::DomainCreate, call)) => {
                let made = store.as_mut().map_err(|e| *e).and_then(|s| s.domain_create(owner).map_err(error));
                match made {
                    Ok(id) => mind::println!("[BLOCKSTORE] DOMAIN {} MADE FOR BADGE {} BY PID {}", id, owner, pid),
                    Err(Error::NoRandom) => mind::println!("[BLOCKSTORE] NO DOMAIN FOR PID {}: NO {}, AND A KEY DERIVED FROM ANYTHING KEPT COULD NOT BE DESTROYED", pid, mind::random::SOURCE),
                    Err(_) => {}
                }
                blockstore::reply_domain_create(call, made)
            }
            Ok((Request::DomainClient { id }, call)) => {
                let cap = store.as_ref().map_err(|e| *e).and_then(|s| match s.domains().find(|d| d.id == id && !d.forgotten) {
                    None => Err(Error::NotFound),
                    Some(d) if d.owner != owner => Err(Error::Rights),
                    Some(_) => clients.client(id, rights, owner),
                });
                if cap.is_ok() { mind::println!("[BLOCKSTORE] DOMAIN {}'S CLIENT LENT TO PID {} (BADGE {})", id, pid, badge); }
                blockstore::reply_domain_client(call, cap)
            }
            Ok((Request::DomainForget { id }, call)) => {
                let done = store.as_mut().map_err(|e| *e).and_then(|s| s.domain_forget(id, owner).map_err(error));
                if done.is_ok() {
                    clients.forget(id);
                    mind::println!("[BLOCKSTORE] DOMAIN {} FORGOTTEN BY PID {}: ITS KEY DESTROYED, ITS RECORDS LEFT FOR THE NEXT COLLECTION", id, pid);
                }
                blockstore::reply_domain_forget(call, done)
            }
            Ok((Request::Domains, call)) => {
                let mut list = [blockstore::Domain::default(); DOMAINS];
                let n = store.as_ref().map_err(|e| *e).map(|s| {
                    let mut n = 0;
                    for (slot, d) in list.iter_mut().zip(s.domains().filter(|d| d.owner == owner)) {
                        *slot = blockstore::Domain { id: d.id, forgotten: d.forgotten };
                        n += 1;
                    }
                    n
                });
                blockstore::reply_domains(call, n.map(|n| &list[..n]))
            }
            Ok((Request::Scrub, call)) => {
                // A whole pass from the first record, on the caller's behalf; one under way between requests is dropped.
                let found = store.as_mut().map_err(|e| *e).and_then(|s| {
                    s.restart_scrub();
                    s.scrub(u64::MAX, &mut found_corrupt).map_err(error)?.ok_or(Error::Device)
                });
                let now = mind::time::monotonic_ns();
                let report = found.map(|f| {
                    scrubs.ended(f, now, Some(pid as u64));
                    scrubs.report(now)
                });
                blockstore::reply_scrub(call, report.as_ref().map_err(|e| *e))
            }
            Ok((Request::Scrubbed, call)) => {
                let report = store.as_ref().map_err(|e| *e).map(|_| scrubs.report(mind::time::monotonic_ns()));
                blockstore::reply_scrubbed(call, report.as_ref().map_err(|e| *e))
            }
        };
    }
}
