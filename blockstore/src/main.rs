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
// A client badged BADGE_RECOVER makes, keeps and forgets recovery keys, and opens with one a medium whose volume key no
// keeper of this machine holds, which is then mounted (308-STO-0028); no key is ever logged.
// A block client of a second medium in slot 4, when init grants one beside a disk of the store's own, holds the copy of
// chosen names, a store of its own that each of their changes reaches after the store; a block one medium finds damaged
// is repaired from the other, and the names behind on the copy are brought in step between requests (309-STO-0040).
// The copy's names are listed with how each stands, and a name only the copy holds is brought back into the store from
// it; beside a store that does not mount, the copy is mounted read-only, so that it says what it would bring back
// (309-STO-0041).
mod copies;
mod store;

// store.rs names these as crate::cid, crate::dag, crate::sha256 and crate::tree, so the host tests build it from the
// libmind files.
use mind::{cid, dag, journal, sha256, tree};

use chacha20poly1305::aead::{AeadInPlace, KeyInit};
use chacha20poly1305::ChaCha20Poly1305;
use cid::Cid;
use mind::abi::{BootInfo, BLOCK_MAX_SECTORS, CAP_GRANT, CAP_KIND_ENDPOINT, CAP_WRITE, SLOT_RTC, SLOT_SERVICE};
use mind::block::Device as Client;
use mind::blockstore::{allowed, recovery_parse, recovery_text, Operation, FIRST_DOMAIN_CLIENT, OWNER_BITS, OWNER_UPDATER};
use copies::{Copies, Event, Gate, Medium as Side, Standing, Step};
use mind::idl::blockstore::{self, Collected, Committed, CopyName, CopyStats, Durability, Error, Head as Current, Kept, Named, PinTaken, Pinned, Published, Request, Stats, Usage, Walked};
use mind::idl::codec::{List, Text};
use mind::idl::wire;
use mind::ipc::{self, Endpoint};
use store::{slots_for, Crypto, Device, Entry, Extent, Head, Keeper, Pin, Store, Update, BLOCK_MAX, BUFFER, COMMIT_MAX, DOMAINS, KEY_KEPT, LAYOUT, SECTOR, VOLUME};

const RECEIVED: usize = 9;
/// The block client init grants.
const BLOCK: usize = 2;
/// The secrets client init lends the store, when keystore gives one (308-KRN-0183).
const SECRETS: usize = 3;
/// The block client of the copy's medium, when init grants one (309-STO-0040).
const COPY: usize = 4;
/// After a catch-up the copy did not take, the next waits this long.
const CATCH_UP_RETRY_NS: u64 = 10_000_000_000;
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
// How often the calendar time is read and the terms acted on (312-STO-0050).
const TERMS_EVERY_NS: u64 = 60 * 1_000_000_000;
const SCRUB_EVERY_NS: u64 = 24 * 3600 * 1_000_000_000;

static mut HEADS: [Head; NAMES] = [Head::EMPTY; NAMES];
static mut RUNS: [Extent; HOLES] = [Extent::EMPTY; HOLES];
static mut PINNED: [Pin; PINS] = [Pin::EMPTY; PINS];
static mut WALK: [u8; dag::CHUNK] = [0; dag::CHUNK];
static mut RECORD: [u8; BUFFER] = [0; BUFFER];
static mut SCRATCH: [u8; blockstore::REQUEST_MAX] = [0; blockstore::REQUEST_MAX];
static mut OUT: [u8; BLOCK_MAX] = [0; BLOCK_MAX];
// The copy's own tables and buffers, and a block on its way between the media (309-STO-0040).
static mut COPY_HEADS: [Head; NAMES] = [Head::EMPTY; NAMES];
static mut COPY_RUNS: [Extent; HOLES] = [Extent::EMPTY; HOLES];
static mut COPY_PINNED: [Pin; PINS] = [Pin::EMPTY; PINS];
static mut COPY_WALK: [u8; dag::CHUNK] = [0; dag::CHUNK];
static mut COPY_RECORD: [u8; BUFFER] = [0; BUFFER];
static mut COPY_BLOCK: [u8; BLOCK_MAX] = [0; BLOCK_MAX];

// What a read takes from the driver at once: its whole transfer buffer, so a pass over the medium, which reads a record's
// sectors at a time, asks the driver a quarter as often.
const AHEAD: usize = BLOCK_MAX_SECTORS * SECTOR;
static mut READ_AHEAD: [u8; AHEAD] = [0; AHEAD];
static mut COPY_READ_AHEAD: [u8; AHEAD] = [0; AHEAD];

/// The block client as the store's medium, with the sectors last read ahead: a read inside them is answered from
/// there, a write over them drops them, and `forget` drops them where a pass or a scrub starts.
struct Medium { client: Client, ahead: &'static mut [u8; AHEAD], at: u64, held: usize }
impl Medium {
    fn new(client: Client, ahead: &'static mut [u8; AHEAD]) -> Self { Medium { client, ahead, at: 0, held: 0 } }
    // Sectors `count` from `lba` as the driver gives them, `out` filled exactly.
    fn direct(&mut self, lba: u64, out: &mut [u8]) -> bool {
        match self.client.read(lba, out.len() / SECTOR) {
            Ok(data) if data.len() == out.len() => { out.copy_from_slice(data); true }
            _ => false,
        }
    }
}
impl Device for Medium {
    fn sectors(&self) -> u64 { self.client.sectors() }
    fn writable(&self) -> bool { !self.client.read_only() }
    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool {
        let count = (out.len() / SECTOR) as u64;
        if lba < self.at || lba + count > self.at + self.held as u64 {
            self.held = 0;
            let ahead = self.client.sectors().saturating_sub(lba).min(BLOCK_MAX_SECTORS as u64) as usize;
            // A read past what was asked that fails (a bad sector further on) leaves the read as asked.
            if (ahead as u64) < count { return self.direct(lba, out); }
            match self.client.read(lba, ahead) {
                Ok(data) if data.len() >= out.len() => { self.ahead[..data.len()].copy_from_slice(data); (self.at, self.held) = (lba, data.len() / SECTOR); }
                _ => return self.direct(lba, out),
            }
        }
        let from = (lba - self.at) as usize * SECTOR;
        out.copy_from_slice(&self.ahead[from..from + out.len()]);
        true
    }
    fn write(&mut self, lba: u64, data: &[u8]) -> bool {
        if lba < self.at + self.held as u64 && lba + (data.len() / SECTOR) as u64 > self.at { self.held = 0; }
        self.client.write(lba, data) == Ok(data.len() / SECTOR)
    }
    fn flush(&mut self) -> bool { self.client.flush().is_ok() }
    fn forget(&mut self) { self.held = 0; }
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
// halving down to the least; none if not even that can be had. The copy's comes from what the store's left (309-STO-0040).
fn index_for(sectors: u64, copy: bool) -> &'static mut [Entry] {
    let mut slots = slots_for(sectors);
    let whose = if copy { "THE COPY'S " } else { "" };
    loop {
        if let Some(mut pages) = mind::mem::Pages::new(slots * core::mem::size_of::<Entry>()) {
            // Page-aligned memory the store keeps for its life: all zeros is an empty slot, and mount empties it anyway.
            let index = unsafe { core::slice::from_raw_parts_mut(pages.as_mut_slice().as_mut_ptr() as *mut Entry, slots) };
            core::mem::forget(pages);
            mind::println!("[BLOCKSTORE] {}INDEX: {} SLOTS ({} KiB) FOR {} SECTORS", whose, slots, slots * core::mem::size_of::<Entry>() / 1024, sectors);
            return index;
        }
        if slots == slots_for(0) { break; }
        slots = (slots / 2).max(slots_for(0));
    }
    mind::println!("[BLOCKSTORE] NO MEMORY FOR {}AN INDEX OF {} SLOTS", whose, slots);
    &mut []
}

fn text(name: &[u8]) -> &str { core::str::from_utf8(name).unwrap_or("?") }
// The log line of a domain made, or why none was (308-STO-0023); its limit when it has one (312-STO-0046).
fn domain_made(made: Result<u16, Error>, owner: u16, pid: u64, kib: u32) {
    match made {
        Ok(id) if kib > 0 => mind::println!("[BLOCKSTORE] DOMAIN {} MADE FOR BADGE {} BY PID {}, RETAINING AT MOST {} KIB", id, owner, pid, kib),
        Ok(id) => mind::println!("[BLOCKSTORE] DOMAIN {} MADE FOR BADGE {} BY PID {}", id, owner, pid),
        Err(Error::NoRandom) => mind::println!("[BLOCKSTORE] NO DOMAIN FOR PID {}: NO {}, AND A KEY DERIVED FROM ANYTHING KEPT COULD NOT BE DESTROYED", pid, mind::random::SOURCE),
        Err(_) => {}
    }
}

// The calendar time (seconds since 1970 UTC) from `rtc`, where init gave a client of it in SLOT_RTC; none without one,
// or if it does not answer (312-STO-0050).
fn calendar() -> Option<u64> {
    if mind::dev::cap_info(SLOT_RTC).0 != CAP_KIND_ENDPOINT { return None; }
    let (days, seconds) = mind::rtc::local_now_in(mind::rtc::Zone::UTC)?;
    Some((days as u64 + 10957) * 86_400 + seconds as u64)
}

// A term's mode as the wire carries it, and as the store keeps it (the maintainer's numbers).
fn mode_in(mode: blockstore::TermMode) -> store::TermMode {
    match mode { blockstore::TermMode::Forget => store::TermMode::Forget, blockstore::TermMode::Release => store::TermMode::Release, blockstore::TermMode::Report => store::TermMode::Report }
}
fn mode_out(mode: store::TermMode) -> blockstore::TermMode {
    match mode { store::TermMode::Forget => blockstore::TermMode::Forget, store::TermMode::Release => blockstore::TermMode::Release, store::TermMode::Report => blockstore::TermMode::Report }
}
fn mode_name(mode: store::TermMode) -> &'static str { match mode { store::TermMode::Forget => "FORGET", store::TermMode::Release => "RELEASE", store::TermMode::Report => "REPORT" } }

// The terms acted on at the calendar time heard: a domain past its term in mode forget forgotten and its clients
// revoked; one in release or report told once. Without a clock, once, why none acts (312-STO-0050).
fn act_on_terms(s: &mut Store<'static, Medium>, clients: &mut DomainClients, told: &mut [u16; DOMAINS], clock: &mut Option<bool>) {
    let now = calendar();
    s.set_calendar(now);
    if clock.replace(now.is_some()) != Some(now.is_some()) {
        match now {
            Some(t) => mind::println!("[BLOCKSTORE] CALENDAR TIME {} FROM RTC: TERMS ACT", t),
            None => mind::println!("[BLOCKSTORE] NO CALENDAR CLOCK (NO RTC CLIENT, OR IT DOES NOT ANSWER): NO TERM ACTS, EVERY DOMAIN AS IN REPORT"),
        }
    }
    loop {
        match s.expire() {
            Ok(Some(id)) => {
                clients.forget(id);
                mind::println!("[BLOCKSTORE] DOMAIN {} FORGOTTEN: ITS TERM PASSED (MODE FORGET); ITS KEY DESTROYED, ITS RECORDS LEFT FOR THE NEXT COLLECTION", id);
            }
            Ok(None) => break,
            Err(e) => { mind::println!("[BLOCKSTORE] A DOMAIN PAST ITS TERM NOT FORGOTTEN: {:?}", e); break }
        }
    }
    for d in s.past_terms() {
        if told.contains(&d.id) { continue; }
        if let Some(slot) = told.iter_mut().find(|t| **t == 0) { *slot = d.id; }
        mind::println!("[BLOCKSTORE] DOMAIN {}'S TERM {} PASSED (MODE {})", d.id, d.until, mode_name(d.mode));
    }
    // A domain no longer past its term may be told again later.
    for t in told.iter_mut() { if *t != 0 && !s.past_terms().any(|d| d.id == *t) { *t = 0; } }
}

// A client of the owner's live domain `id` with `rights`: not-found for none, rights for another owner's (308-STO-0023).
fn lend_domain(store: &Result<Store<'static, Medium>, Error>, clients: &mut DomainClients, id: u16, rights: u16, owner: u16) -> Result<usize, Error> {
    store.as_ref().map_err(|e| *e).and_then(|s| match s.domains().find(|d| d.id == id && !d.forgotten) {
        None => Err(Error::NotFound),
        Some(d) if d.owner != owner => Err(Error::Rights),
        Some(_) => clients.client(id, rights, owner),
    })
}

fn medium(side: Side) -> &'static str { match side { Side::Store => "THE STORE'S MEDIUM", Side::Copy => "THE COPY" } }

// What the copy did, logged (309-STO-0040).
fn told(e: Event) {
    let step = |s: Step| match s { Step::Blocks => "ITS BLOCKS NOT PUT THERE", Step::Name => "ITS NAME NOT MOVED THERE" };
    match e {
        Event::Copied { name, version, blocks } => mind::println!("[BLOCKSTORE] COPIED {} VERSION {} ON THE COPY, {} BLOCKS PUT THERE", text(name), version, blocks),
        Event::Removed { name, version } => mind::println!("[BLOCKSTORE] {} REMOVED ON THE COPY, VERSION {} THERE", text(name), version),
        Event::Behind { name, step: s, error } => mind::println!("[BLOCKSTORE] {} BEHIND ON THE COPY, {}: {:?}", text(name), step(s), error),
        Event::NotCopied { name, step: s, error } => mind::println!("[BLOCKSTORE] {} NOT COPIED, {}: {:?}", text(name), step(s), error),
        Event::OnlyOnCopy { name, version, root } => mind::println!("[BLOCKSTORE] {} ONLY ON THE COPY, VERSION {} ROOT {}: LEFT AS IT IS", text(name), version, root),
        Event::Repaired { cid, into: Side::Store } => mind::println!("[BLOCKSTORE] REPAIRED {} FROM THE COPY", cid),
        Event::Repaired { cid, into: Side::Copy } => mind::println!("[BLOCKSTORE] REPAIRED {} ON THE COPY FROM THE STORE", cid),
        Event::NotRepaired { cid, on, error } => mind::println!("[BLOCKSTORE] NOT REPAIRED {} ON {}: {:?}", cid, medium(on), error),
        Event::Collected { on, freed: Ok(n) } => mind::println!("[BLOCKSTORE] COLLECTED {} BLOCKS ON {} AFTER REPAIRS: THE DAMAGED RECORDS FREED", n, medium(on)),
        Event::Collected { on, freed: Err(e) } => mind::println!("[BLOCKSTORE] NO COLLECTION ON {} AFTER REPAIRS: {:?}; ASKED FOR AFTER ITS NEXT PASS", medium(on), e),
        Event::Rebuilt { name, version, root, put } => mind::println!("[BLOCKSTORE] REBUILT {} VERSION {} ROOT {} FROM THE COPY, {} BLOCKS PUT", text(name), version, root, put),
        Event::NotRebuilt { name, error, at: Some(at) } => mind::println!("[BLOCKSTORE] NOT REBUILT {}: {:?} AT {}", text(name), error, at),
        Event::NotRebuilt { name, error, at: None } => mind::println!("[BLOCKSTORE] NOT REBUILT {}: {:?}", text(name), error),
    }
}

// How a name on the copy stands against the store, on the wire (309-STO-0041).
fn stands(s: Standing) -> blockstore::Standing {
    match s { Standing::InStep => blockstore::Standing::InStep, Standing::Behind => blockstore::Standing::Behind, Standing::OnlyOnCopy => blockstore::Standing::OnlyOnCopy, Standing::Unknown => blockstore::Standing::Unknown }
}

// A record a scrub of either medium found that does not open or is not whole.
fn found_on(side: Side, lba: u64, block: Option<Cid>) {
    match (side, block) {
        (Side::Store, _) => found_corrupt(lba, block),
        (Side::Copy, Some(cid)) => mind::println!("[BLOCKSTORE] COPY SCRUB: BLOCK {} AT SECTOR {} CORRUPT", cid, lba),
        (Side::Copy, None) => mind::println!("[BLOCKSTORE] COPY SCRUB: RECORD AT SECTOR {} CORRUPT", lba),
    }
}

// A whole pass of the copy's medium ended: between requests, or asked for by `pid`.
fn copy_scrubbed(found: store::Scrubbed, pid: Option<u64>) {
    let (records, corrupt, damaged) = (found.records, found.corrupt, found.damaged);
    match pid {
        Some(pid) => mind::println!("[BLOCKSTORE] COPY SCRUBBED {} RECORDS: {} CORRUPT, {} DAMAGED SECTORS, ASKED BY PID {}", records, corrupt, damaged, pid),
        None => mind::println!("[BLOCKSTORE] COPY SCRUBBED {} RECORDS: {} CORRUPT, {} DAMAGED SECTORS, BETWEEN REQUESTS", records, corrupt, damaged),
    }
}

// Mounts the copy on the medium init lends in slot COPY (309-STO-0040): a whole store of its own, its volume key kept by
// keystore as the store's is. Beside the mounted store `main`; beside one that did not mount, through a shut gate, so
// that it is read and nothing of it is written until the store mounts (309-STO-0041). None beside a RAM disk, whose
// writes outlive no reset, nor on one. Called only while no copy is mounted, which holds the statics; its index is made
// once.
fn mount_copy(main: Option<&mut Store<'static, Medium>>, ram: bool, secrets: &mut Secrets, lent: bool, index: &mut Option<*mut [Entry]>) -> Option<Copies<'static, Medium>> {
    if mind::dev::cap_info(COPY).0 != CAP_KIND_ENDPOINT { return None; }
    if ram { mind::println!("[BLOCKSTORE] NO COPY: THE STORE IS ON A RAM DISK, WHICH OUTLIVES NO RESET"); return None; }
    let client = match Client::open(Endpoint(COPY)) {
        Ok(client) => client,
        Err(e) => { mind::println!("[BLOCKSTORE] NO COPY: ITS MEDIUM DOES NOT ANSWER: {:?}", e); return None; }
    };
    if client.kind() == mind::block::KIND_RAM { mind::println!("[BLOCKSTORE] NO COPY: ITS MEDIUM IS A RAM DISK"); return None; }
    let slots = match *index {
        Some(slots) => slots,
        None => {
            let slots = index_for(client.sectors(), true);
            if slots.is_empty() { mind::println!("[BLOCKSTORE] NO COPY: NO MEMORY FOR ITS INDEX"); return None; }
            *index.insert(slots as *mut [Entry])
        }
    };
    let (heads, pinned, runs, record, walk, block, index) = unsafe {
        (&mut *core::ptr::addr_of_mut!(COPY_HEADS), &mut *core::ptr::addr_of_mut!(COPY_PINNED), &mut *core::ptr::addr_of_mut!(COPY_RUNS),
         &mut *core::ptr::addr_of_mut!(COPY_RECORD), &mut *core::ptr::addr_of_mut!(COPY_WALK), &mut *core::ptr::addr_of_mut!(COPY_BLOCK), &mut *slots)
    };
    let beside = main.is_some();
    let keeper = lent.then_some(secrets as &mut dyn Keeper);
    let ahead = unsafe { &mut *core::ptr::addr_of_mut!(COPY_READ_AHEAD) };
    match Store::mount(Gate::new(Medium::new(client, ahead), beside), &Sealing, keeper, index, heads, pinned, runs, record, walk, mind::time::monotonic_ns()) {
        Ok(mut copy) => {
            copy.renew(mind::time::monotonic_ns());
            let kept = copy.key_source() == KEY_KEPT;
            match copy.made() {
                Some(random) => mind::println!("[BLOCKSTORE] THE COPY'S VOLUME KEY MADE {}{}, {}", if random { "FROM " } else { "WITHOUT " }, mind::random::SOURCE,
                    if kept { "KEPT BY KEYSTORE, NOT ON THE MEDIUM" } else { "IN ITS SUPERBLOCK" }),
                None => {}
            }
            let s = copy.stats();
            mind::println!("[BLOCKSTORE] COPY READY BLOCKS={} NAMES={} SECTORS={}/{} CORRUPT={} DAMAGED={} SEALED, {}",
                s.blocks, s.names, s.used, s.sectors, s.corrupt, s.damaged, if kept { "KEY KEPT BY KEYSTORE" } else { "KEY IN THE CLEAR" });
            let copies = Copies::new(copy, block, main, &mut told);
            if beside { mind::println!("[BLOCKSTORE] COPY MOUNTED: {} NAMES WITH A COPY, {} BEHIND", copies.stats().names, copies.behind()); }
            else { mind::println!("[BLOCKSTORE] COPY READ BESIDE A STORE NOT MOUNTED: {} NAMES ON IT, NOTHING WRITTEN THERE UNTIL THE STORE MOUNTS", copies.stats().names); }
            Some(copies)
        }
        // A blank medium, or one without a key yet, is formatted only beside a store that mounted.
        Err(store::Error::ReadOnly) if !beside => { mind::println!("[BLOCKSTORE] NO COPY YET: ITS MEDIUM HOLDS NO STORE TO READ, AND NONE IS MADE BESIDE A STORE NOT MOUNTED"); None }
        Err(store::Error::Kept(id)) => { mind::println!("[BLOCKSTORE] NO COPY: NOT MOUNTED: ITS VOLUME KEY IS KEPT BY KEYSTORE AS {}, WHICH KEEPS NONE UNDER IT", secret_name(&id)); None }
        Err(store::Error::Layout(found)) => { mind::println!("[BLOCKSTORE] NO COPY: NOT MOUNTED: A STORE OF LAYOUT {}; THIS BUILD READS LAYOUT {}", found, LAYOUT); None }
        Err(e) => { mind::println!("[BLOCKSTORE] NO COPY: NOT MOUNTED: {:?}", e); None }
    }
}

// Mounts the store on the driver's medium, at boot and again after a recovery (308-STO-0028), its index made once for
// the medium's size; whether the medium is a RAM disk. Called only while no store is mounted, which holds the statics.
fn mount(index: &mut Option<*mut [Entry]>, secrets: &mut Secrets, lent: bool) -> (Result<Store<'static, Medium>, store::Error>, bool) {
    let Ok(client) = Client::open(Endpoint(BLOCK)) else { return (Err(store::Error::Device), true) };
    let ram = client.kind() == mind::block::KIND_RAM;
    let slots = *index.get_or_insert_with(|| index_for(client.sectors(), false) as *mut [Entry]);
    let (heads, pinned, runs, record, walk, index) = unsafe {
        (&mut *core::ptr::addr_of_mut!(HEADS), &mut *core::ptr::addr_of_mut!(PINNED), &mut *core::ptr::addr_of_mut!(RUNS),
         &mut *core::ptr::addr_of_mut!(RECORD), &mut *core::ptr::addr_of_mut!(WALK), &mut *slots)
    };
    let keeper = (!ram && lent).then_some(secrets as &mut dyn Keeper);
    let ahead = unsafe { &mut *core::ptr::addr_of_mut!(READ_AHEAD) };
    (Store::mount(Medium::new(client, ahead), &Sealing, keeper, index, heads, pinned, runs, record, walk, mind::time::monotonic_ns()), ram)
}

// The updater's owner, whose releases a quarter of a lasting medium is kept for (351-STO-0043, MC-9.3).
fn updater(owner: u16) -> bool { owner < FIRST_DOMAIN_CLIENT && owner & OWNER_BITS == OWNER_UPDATER }

// What a mount gave, logged: the store, or the reason every request is answered with.
fn ready(mounted: Result<Store<'static, Medium>, store::Error>, ram: bool, lent: bool) -> Result<Store<'static, Medium>, Error> {
    let durability = if ram { Durability::Volatile } else { Durability::Flushed };
    match mounted {
        Ok(mut store) => {
            store.renew(mind::time::monotonic_ns());
            // On a RAM disk nothing is kept for later, so nothing is set aside.
            if !ram {
                store.reserve_for(Some(updater));
                mind::println!("[BLOCKSTORE] A QUARTER OF THE MEDIUM KEPT FOR THE UPDATER'S RELEASES: THE OTHER OWNERS TOGETHER RETAIN AT MOST THREE QUARTERS");
            }
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
    }
}

// Zeroes a key's bytes, so they do not outlive their use in memory the compiler might leave alone.
fn wipe(bytes: &mut [u8]) { for b in bytes { unsafe { core::ptr::write_volatile(b, 0) } } }
// Zeroes a typed key decoded from a request.
fn wipe_text(text: &mut Text<{ mind::blockstore::RECOVERY_TEXT }>) { unsafe { core::ptr::write_volatile(text, Text::default()) } }

// Opens the medium with a recovery key typed by its owner, then mounts it (308-STO-0028): the volume key goes to
// keystore under the medium's id, and the store mounts as this machine's own. Called only while none is mounted.
fn recover(key: &str, index: &mut Option<*mut [Entry]>, secrets: &mut Secrets, lent: bool) -> Result<Result<Store<'static, Medium>, Error>, Error> {
    let mut recovery = recovery_parse(key.as_bytes()).map_err(|_| Error::Invalid)?;
    // Without keystore's client the key could only go to the superblock, in the clear.
    if !lent { wipe(&mut recovery); return Err(Error::Key); }
    let found = match Client::open(Endpoint(BLOCK)) {
        Ok(client) => {
            // No store is mounted: its buffers are free.
            let (buffer, ahead) = unsafe { (&mut *core::ptr::addr_of_mut!(RECORD), &mut *core::ptr::addr_of_mut!(READ_AHEAD)) };
            store::recover(&mut Medium::new(client, ahead), &Sealing, secrets, &recovery, buffer).map_err(error)
        }
        Err(_) => Err(Error::Device),
    };
    wipe(&mut recovery);
    found?;
    let (mounted, ram) = mount(index, secrets, lent);
    Ok(ready(mounted, ram, lent))
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (scratch, out) = unsafe { (&mut *core::ptr::addr_of_mut!(SCRATCH), &mut *core::ptr::addr_of_mut!(OUT)) };
    // A RAM disk keeps its key in its own superblock, as nothing of it outlives a reset; any other medium's key is kept
    // by keystore, when init lent its client (308-STO-0022).
    let lent = mind::dev::cap_info(SECRETS).0 == CAP_KIND_ENDPOINT;
    let mut secrets = Secrets(Endpoint(SECRETS));
    let mut index = None;
    let (mounted, ram) = mount(&mut index, &mut secrets, lent);
    // How far a write reaches is the medium's: nothing outlives a reset on a RAM disk; elsewhere the driver acknowledged
    // a flush after it, and what that reaches is the driver's to state (307-STO-0019).
    let durability = if ram { Durability::Volatile } else { Durability::Flushed };
    // Without a medium every request is answered with the reason, so clients are not left waiting.
    let mut store = ready(mounted, ram, lent);
    // The copy of chosen names on a second medium (309-STO-0040): read-only beside a store that did not mount (309-STO-0041).
    let mut copy_index = None;
    let mut copies = mount_copy(store.as_mut().ok(), ram, &mut secrets, lent, &mut copy_index);
    // When the next catch-up of a name behind on the copy may run.
    let mut retry = 0u64;
    let mut clients = DomainClients([None; DOMAIN_CLIENTS]);
    let mut scrubs = Scrubs { passes: 0, last: store::Scrubbed::default(), ended: 0, next: mind::time::monotonic_ns() + SCRUB_EVERY_NS };
    // When the terms are next acted on, the domains past theirs told already, and whether a clock was heard (312-STO-0050).
    let (mut terms_next, mut told_terms, mut clock) = (0u64, [0u16; DOMAINS], None::<bool>);
    loop {
        // A pass under way, or a name behind on the copy, goes on after a pause without requests; the next pass starts
        // when it is due.
        let now = mind::time::monotonic_ns();
        if now >= terms_next {
            if let Ok(s) = store.as_mut() { act_on_terms(s, &mut clients, &mut told_terms, &mut clock); }
            terms_next = now + TERMS_EVERY_NS;
        }
        let wait = match store.as_ref() {
            Ok(s) if copies.as_ref().map_or(s.scrubbing(), |c| c.scrubbing(s) || c.behind() > 0 && now >= retry) => SCRUB_PAUSE_MS,
            Ok(_) => {
                let due = match copies.as_ref() { Some(c) if c.behind() > 0 => scrubs.next.min(retry), _ => scrubs.next };
                (due.saturating_sub(now) / 1_000_000).clamp(1, u32::MAX as u64) as u32
            }
            Err(_) => 0,
        };
        let wait = if store.is_ok() { wait.min((terms_next.saturating_sub(now) / 1_000_000).clamp(1, u32::MAX as u64) as u32) } else { wait };
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, wait) else {
            let now = mind::time::monotonic_ns();
            match (store.as_mut(), copies.as_mut()) {
                // One name behind brought in step, then a step of the scrub of either medium (MC-5.1).
                (Ok(s), Some(c)) => {
                    s.set_time(now);
                    c.set_time(now);
                    if c.behind() > 0 && now >= retry && c.catch_up(s, &mut told) == Some(false) { retry = now + CATCH_UP_RETRY_NS; }
                    if c.scrubbing(s) || now >= scrubs.next {
                        match c.scrub(s, SCRUB_STEP, &mut found_on, &mut told) {
                            Ok(Some((Side::Store, found))) => scrubs.ended(found, now, None),
                            Ok(Some((Side::Copy, found))) => copy_scrubbed(found, None),
                            Ok(None) => {}
                            Err((side, e)) => {
                                match side { Side::Store => mind::println!("[BLOCKSTORE] SCRUB STOPPED: {:?}", e), Side::Copy => mind::println!("[BLOCKSTORE] SCRUB STOPPED ON THE COPY: {:?}", e) }
                                c.restart_scrub(s);
                                scrubs.next = now + SCRUB_EVERY_NS;
                            }
                        }
                    }
                }
                (Ok(s), None) if s.scrubbing() || now >= scrubs.next => match s.scrub(SCRUB_STEP, &mut found_corrupt) {
                    Ok(Some(found)) => scrubs.ended(found, now, None),
                    Ok(None) => {}
                    Err(e) => {
                        mind::println!("[BLOCKSTORE] SCRUB STOPPED: {:?}", e);
                        s.restart_scrub();
                        scrubs.next = now + SCRUB_EVERY_NS;
                    }
                },
                _ => {}
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
        if let Some(c) = copies.as_mut() { c.set_time(mind::time::monotonic_ns()); }
        // How far a change reached: both media, or the store's own.
        let reach = |copied: bool| if copied { Durability::Copied } else { durability };
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
        // A request that carried a recovery key; its copy is zeroed once it is answered.
        let mut secret = false;
        let _ = match blockstore::decode(&request, RECEIVED, &mut *scratch) {
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
            Ok((Request::DomainClientWith { .. }, call)) if refused(Operation::DomainClient) || through_domain(Operation::DomainClient) => blockstore::reply_domain_client_with(call, Err(Error::Rights)),
            // A term is retention, as forgetting is; reading it is the owner's, as `domains` is (312-STO-0050).
            Ok((Request::DomainSetTerm { .. }, call)) if refused(Operation::DomainForget) || through_domain(Operation::DomainForget) => blockstore::reply_domain_set_term(call, Err(Error::Rights)),
            Ok((Request::DomainTerm { .. }, call)) if refused(Operation::Domains) || through_domain(Operation::Domains) => blockstore::reply_domain_term(call, Err(Error::Rights)),
            Ok((Request::DomainForget { .. }, call)) if refused(Operation::DomainForget) || through_domain(Operation::DomainForget) => blockstore::reply_domain_forget(call, Err(Error::Rights)),
            Ok((Request::Domains, call)) if refused(Operation::Domains) || through_domain(Operation::Domains) => blockstore::reply_domains(call, Err(Error::Rights)),
            Ok((Request::DomainCreateLimited { .. }, call)) if refused(Operation::DomainCreate) || through_domain(Operation::DomainCreate) => blockstore::reply_domain_create_limited(call, Err(Error::Rights)),
            Ok((Request::DomainUsage { .. }, call)) if refused(Operation::Domains) || through_domain(Operation::Domains) => blockstore::reply_domain_usage(call, Err(Error::Rights)),
            Ok((Request::Scrub, call)) if refused(Operation::Scrub) => blockstore::reply_scrub(call, Err(Error::Rights)),
            Ok((Request::Scrubbed, call)) if refused(Operation::Scrubbed) => blockstore::reply_scrubbed(call, Err(Error::Rights)),
            // Recovery keys are the volume's, never a domain's (308-STO-0028).
            Ok((Request::RecoveryNew, call)) if refused(Operation::RecoveryNew) || through_domain(Operation::RecoveryNew) => blockstore::reply_recovery_new(call, Err(Error::Rights)),
            Ok((Request::RecoveryKeep { .. }, call)) if refused(Operation::RecoveryKeep) || through_domain(Operation::RecoveryKeep) => blockstore::reply_recovery_keep(call, Err(Error::Rights)),
            Ok((Request::RecoveryForget, call)) if refused(Operation::RecoveryForget) || through_domain(Operation::RecoveryForget) => blockstore::reply_recovery_forget(call, Err(Error::Rights)),
            Ok((Request::Recoveries, call)) if refused(Operation::Recoveries) => blockstore::reply_recoveries(call, Err(Error::Rights)),
            Ok((Request::Recover { .. }, call)) if refused(Operation::Recover) || through_domain(Operation::Recover) => blockstore::reply_recover(call, Err(Error::Rights)),
            // The copy holds the volume's names alone: a domain's records are sealed under its key (309-STO-0040).
            Ok((Request::Copy { .. }, call)) if refused(Operation::Copy) || through_domain(Operation::Copy) => blockstore::reply_copy(call, Err(Error::Rights)),
            Ok((Request::CopyStat, call)) if refused(Operation::CopyStat) => blockstore::reply_copy_stat(call, Err(Error::Rights)),
            // The copy's names and a name brought back from it are the volume's too (309-STO-0041).
            Ok((Request::CopyNames { .. }, call)) if refused(Operation::CopyNames) || through_domain(Operation::CopyNames) => blockstore::reply_copy_names(call, Err(Error::Rights)),
            Ok((Request::Rebuild { .. }, call)) if refused(Operation::Rebuild) || through_domain(Operation::Rebuild) => blockstore::reply_rebuild(call, Err(Error::Rights)),
            Ok((Request::Put { codec, data }, call)) => {
                let codec = match codec { blockstore::Codec::Raw => cid::Codec::Raw, blockstore::Codec::DagCbor => cid::Codec::DagCbor };
                let cid = store.as_mut().map_err(|e| *e).and_then(|s| s.put(codec, data).map_err(error)).map(|c| c.to_bytes());
                blockstore::reply_put(call, cid.as_ref().map(|c| &c[..]).map_err(|e| *e))
            }
            Ok((Request::Get { cid }, call)) => {
                let got = parse(cid).and_then(|cid| {
                    let s = store.as_mut().map_err(|e| *e)?;
                    // With a copy, a block the store finds damaged, or lacks, is read from it and put back.
                    let got = match copies.as_mut() { Some(c) => c.get(s, &cid, out, &mut told), None => s.get(&cid, out) };
                    got.map_err(|e| {
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
                    let name_bytes = name.as_str().as_bytes();
                    let (version, copied) = match copies.as_mut() {
                        Some(c) => c.publish(s, name_bytes, expected, &root, owner, &mut told),
                        None => s.publish(name_bytes, expected, &root, owner).map(|v| (v, false)),
                    }.map_err(error)?;
                    mind::println!("[BLOCKSTORE] PUBLISHED {} VERSION {} ROOT {} BY PID {}", name, version, root, pid);
                    Ok(Published { version, durability: reach(copied) })
                });
                blockstore::reply_publish(call, version.as_ref().map_err(|e| *e))
            }
            Ok((Request::Resolve { name }, call)) => {
                let head = store.as_ref().map_err(|e| *e).and_then(|s| s.resolve(name.as_str().as_bytes()).map_err(error));
                let head = head.map(|(version, root)| Current { version, root: List::from_slice(&root.to_bytes()).unwrap_or_default() });
                blockstore::reply_resolve(call, head.as_ref().map_err(|e| *e))
            }
            Ok((Request::Unpublish { name, expected }, call)) => {
                let version = store.as_mut().map_err(|e| *e).and_then(|s| match copies.as_mut() {
                    Some(c) => c.unpublish(s, name.as_str().as_bytes(), expected, owner, &mut told),
                    None => s.unpublish(name.as_str().as_bytes(), expected, owner).map(|v| (v, false)),
                }.map_err(error));
                if let Ok((v, _)) = version { mind::println!("[BLOCKSTORE] REMOVED {} VERSION {} BY PID {}", name, v, pid); }
                blockstore::reply_unpublish(call, version.map(|(version, copied)| Published { version, durability: reach(copied) }).as_ref().map_err(|e| *e))
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
                    let changes = &changes[..updates.len()];
                    let (versions, copied) = match copies.as_mut() {
                        Some(c) => c.commit(s, changes, owner, &mut told),
                        None => s.commit(changes, owner).map(|v| (v, false)),
                    }.map_err(error)?;
                    for (u, v) in updates.iter().zip(versions) {
                        match changes.iter().find(|c| c.name == u.name.as_str().as_bytes()).and_then(|c| c.root) {
                            Some(root) => mind::println!("[BLOCKSTORE] COMMITTED {} VERSION {} ROOT {} BY PID {}", u.name, v, root, pid),
                            None => mind::println!("[BLOCKSTORE] COMMITTED {} VERSION {} REMOVED BY PID {}", u.name, v, pid),
                        }
                    }
                    Ok((versions, copied))
                });
                let committed = versions.map(|(v, copied)| Committed { versions: List::from_slice(&v[..updates.len()]).unwrap_or_default(), durability: reach(copied) });
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
                domain_made(made, owner, pid, 0);
                blockstore::reply_domain_create(call, made)
            }
            Ok((Request::DomainCreateLimited { kib }, call)) => {
                let made = store.as_mut().map_err(|e| *e).and_then(|s| s.domain_create_limited(owner, kib).map_err(error));
                domain_made(made, owner, pid, kib);
                blockstore::reply_domain_create_limited(call, made)
            }
            Ok((Request::DomainUsage { id }, call)) => {
                let usage = store.as_ref().map_err(|e| *e).and_then(|s| s.domain_usage(id, owner).map_err(error))
                    .map(|u| Usage { retained: u.retained, quota: u.quota, names: u.names, pins: u.pins });
                blockstore::reply_domain_usage(call, usage.as_ref().map_err(|e| *e))
            }
            Ok((Request::DomainClient { id }, call)) => {
                let cap = lend_domain(&store, &mut clients, id, rights, owner);
                if cap.is_ok() { mind::println!("[BLOCKSTORE] DOMAIN {}'S CLIENT LENT TO PID {} (BADGE {})", id, pid, badge); }
                blockstore::reply_domain_client(call, cap)
            }
            // A client of the domain with fewer rights than the caller's (312-STO-0049).
            Ok((Request::DomainClientWith { id, rights: wanted }, call)) => {
                let narrowed = mind::blockstore::narrowed(rights, wanted);
                let cap = narrowed.ok_or(Error::Invalid).and_then(|r| lend_domain(&store, &mut clients, id, r, owner));
                if cap.is_ok() { mind::println!("[BLOCKSTORE] DOMAIN {}'S CLIENT LENT TO PID {} (BADGE {}) WITH RIGHTS {}", id, pid, badge, narrowed.unwrap_or(0)); }
                blockstore::reply_domain_client_with(call, cap)
            }
            Ok((Request::DomainSetTerm { id, mode, until }, call)) => {
                let done = store.as_mut().map_err(|e| *e).and_then(|s| s.domain_set_term(id, owner, until, mode_in(mode)).map_err(error));
                if done.is_ok() {
                    let heard = if clock == Some(true) { "" } else { "; NO CALENDAR CLOCK, SO NONE ACTS UNTIL ONE IS HEARD" };
                    mind::println!("[BLOCKSTORE] DOMAIN {}'S TERM SET BY PID {}: UNTIL {}, MODE {}{}", id, pid, until, mode_name(mode_in(mode)), heard);
                    // Acted on at once, with the calendar time heard last.
                    terms_next = 0;
                }
                blockstore::reply_domain_set_term(call, done)
            }
            Ok((Request::DomainTerm { id }, call)) => {
                let term = store.as_ref().map_err(|e| *e).and_then(|s| s.domain_term(id, owner).map_err(error))
                    .map(|t| blockstore::Term { until: t.until, mode: mode_out(t.mode), clock: t.clock, passed: t.passed });
                blockstore::reply_domain_term(call, term.as_ref().map_err(|e| *e))
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
                // With a copy, the copy's medium after the store's, each repairing the other (309-STO-0040).
                let found = store.as_mut().map_err(|e| *e).and_then(|s| match copies.as_mut() {
                    Some(c) => c.scrub_whole(s, SCRUB_STEP, &mut found_on, &mut told).map(|(mine, theirs)| (mine, Some(theirs))).map_err(error),
                    None => {
                        s.restart_scrub();
                        Ok((s.scrub(u64::MAX, &mut found_corrupt).map_err(error)?.ok_or(Error::Device)?, None))
                    }
                });
                let now = mind::time::monotonic_ns();
                let report = found.map(|(f, theirs)| {
                    scrubs.ended(f, now, Some(pid as u64));
                    match theirs {
                        Some(Ok(f)) => copy_scrubbed(f, Some(pid as u64)),
                        Some(Err(e)) => mind::println!("[BLOCKSTORE] SCRUB STOPPED ON THE COPY: {:?}", e),
                        None => {}
                    }
                    scrubs.report(now)
                });
                blockstore::reply_scrub(call, report.as_ref().map_err(|e| *e))
            }
            Ok((Request::Scrubbed, call)) => {
                let report = store.as_ref().map_err(|e| *e).map(|_| scrubs.report(mind::time::monotonic_ns()));
                blockstore::reply_scrubbed(call, report.as_ref().map_err(|e| *e))
            }
            Ok((Request::RecoveryNew, call)) => {
                // The key goes to the caller alone, in the form its owner writes down; no line logs it.
                let mut made = store.as_mut().map_err(|e| *e).and_then(|s| {
                    let mut key = s.recovery_new().map_err(error)?;
                    let text = recovery_text(&key);
                    wipe(&mut key);
                    mind::println!("[BLOCKSTORE] RECOVERY KEY MADE BY PID {}: {} RECORDS UNTIL IT IS TYPED BACK", pid, s.recovery());
                    Ok(text)
                });
                let replied = blockstore::reply_recovery_new(call, made.as_ref().map(|t| core::str::from_utf8(t).unwrap_or("")).map_err(|e| *e));
                if let Ok(text) = made.as_mut() { wipe(text); }
                replied
            }
            Ok((Request::RecoveryKeep { mut key }, call)) => {
                let kept = store.as_mut().map_err(|e| *e).and_then(|s| {
                    let mut recovery = recovery_parse(key.as_str().as_bytes()).map_err(|_| Error::Invalid)?;
                    let erased = s.recovery_keep(&recovery).map_err(error);
                    wipe(&mut recovery);
                    let erased = erased?;
                    mind::println!("[BLOCKSTORE] RECOVERY KEY KEPT BY PID {}: {} OTHER RECORDS ERASED, {} LEFT", pid, erased, s.recovery());
                    Ok(erased as u32)
                });
                wipe_text(&mut key);
                secret = true;
                blockstore::reply_recovery_keep(call, kept)
            }
            Ok((Request::RecoveryForget, call)) => {
                let erased = store.as_mut().map_err(|e| *e).and_then(|s| s.recovery_forget().map_err(error)).map(|n| n as u32);
                if let Ok(n) = erased { mind::println!("[BLOCKSTORE] RECOVERY KEYS FORGOTTEN BY PID {}: {} RECORDS ERASED", pid, n); }
                blockstore::reply_recovery_forget(call, erased)
            }
            Ok((Request::Recoveries, call)) => {
                blockstore::reply_recoveries(call, store.as_ref().map_err(|e| *e).map(|s| s.recovery() as u32))
            }
            Ok((Request::Recover { mut key }, call)) => {
                // Only a medium that no keeper of this machine opens is recovered: not a mounted one, nor one refused for another reason.
                secret = true;
                let done = match store.as_ref().err().copied() {
                    None => Err(Error::Invalid),
                    Some(Error::Key) => match recover(key.as_str(), &mut index, &mut secrets, lent) {
                        Ok(mounted) => {
                            let answer = mounted.as_ref().map(|_| ()).map_err(|e| *e);
                            match answer {
                                Ok(()) => mind::println!("[BLOCKSTORE] RECOVERED BY PID {}: THE VOLUME KEY KEPT BY KEYSTORE UNDER THE MEDIUM'S ID, AND MOUNTED", pid),
                                Err(e) => mind::println!("[BLOCKSTORE] RECOVERED BY PID {}: THE VOLUME KEY KEPT BY KEYSTORE, BUT NOT MOUNTED: {:?}", pid, e),
                            }
                            store = mounted;
                            // A medium recovered keeps its key off it, so it is no RAM disk. A copy read beside none is
                            // kept in step from now on, its names compared as a mount compares them (309-STO-0041).
                            match (store.as_mut(), copies.as_mut()) {
                                (Ok(s), Some(c)) => {
                                    c.attach(s, &mut told);
                                    mind::println!("[BLOCKSTORE] COPY MOUNTED: {} NAMES WITH A COPY, {} BEHIND", c.stats().names, c.behind());
                                }
                                (Ok(s), None) => copies = mount_copy(Some(s), false, &mut secrets, lent, &mut copy_index),
                                (Err(_), _) => {}
                            }
                            answer
                        }
                        Err(e) => { mind::println!("[BLOCKSTORE] NOT RECOVERED FOR PID {}: {:?}", pid, e); Err(e) }
                    },
                    Some(e) => Err(e),
                };
                wipe_text(&mut key);
                blockstore::reply_recover(call, done)
            }
            Ok((Request::Copy { name, on }, call)) => {
                let done = store.as_mut().map_err(|e| *e).and_then(|s| {
                    let c = copies.as_mut().ok_or(Error::NoCopy)?;
                    let copied = c.copy(s, name.as_str().as_bytes(), on, owner, &mut told).map_err(error)?;
                    let reached = reach(copied);
                    mind::println!("[BLOCKSTORE] COPY {} {} BY PID {}: {}", if on { "ON" } else { "OFF" }, name, pid, mind::blockstore::class(reached as u8));
                    Ok(reached)
                });
                blockstore::reply_copy(call, done)
            }
            Ok((Request::CopyStat, call)) => {
                // A copy read beside a store not mounted answers too; without one, an unmounted store says why (309-STO-0041).
                let stats = copies.as_ref().ok_or_else(|| store.as_ref().err().copied().unwrap_or(Error::NoCopy)).map(|c| {
                    let s = c.stats();
                    CopyStats { sectors: s.sectors, used: s.used, blocks: s.blocks, names: s.names, behind: s.behind, from_copy: s.from_copy, from_store: s.from_store }
                });
                blockstore::reply_copy_stat(call, stats.as_ref().map_err(|e| *e))
            }
            Ok((Request::CopyNames { after }, call)) => {
                let mut page = [CopyName::default(); copies::PAGE];
                let n = match copies.as_ref() {
                    // Unmounted and without a copy, the store says why, as to every request.
                    None => Err(store.as_ref().err().copied().unwrap_or(Error::NoCopy)),
                    Some(c) => c.names(store.as_ref().ok(), after.as_str().as_bytes()).map_err(error).map(|names| {
                        let mut n = 0;
                        for (slot, (name, held)) in page.iter_mut().zip(names) {
                            let root = List::from_slice(&held.root.to_bytes()).unwrap_or_default();
                            *slot = CopyName { name: Text::new(text(name)).unwrap_or_default(), version: held.version, root, standing: stands(held.standing) };
                            n += 1;
                        }
                        n
                    }),
                };
                blockstore::reply_copy_names(call, n.map(|n| &page[..n]))
            }
            Ok((Request::Rebuild { name, check }, call)) => {
                let done = match copies.as_mut() {
                    None => Err(store.as_ref().err().copied().unwrap_or(Error::NoCopy)),
                    Some(c) => c.rebuild(store.as_mut().ok(), name.as_str().as_bytes(), check, &mut told).map_err(error).map(|r| {
                        let (walked, at) = match r.stopped {
                            None => (Walked::Whole, List::default()),
                            Some((at, e)) => (if e == store::Error::Corrupt { Walked::Corrupt } else { Walked::Incomplete }, List::from_slice(&at.to_bytes()).unwrap_or_default()),
                        };
                        blockstore::Rebuilt { version: r.version, visited: r.visited, lacked: r.lacked, put: r.put, durability: reach(r.copied), walked, at }
                    }),
                };
                blockstore::reply_rebuild(call, done.as_ref().map_err(|e| *e))
            }
        };
        if secret { wipe(scratch); }
    }
}
