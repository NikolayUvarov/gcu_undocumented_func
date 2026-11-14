#![no_std]
#![no_main]
// The updater (351-UPD-0007, idl/update.wit, docs/update/updater.md; MC-9.2–9.4, MC-3.11): turns a published release
// into a staged, verified slot and nothing more. It takes each key file as the next version of the one it trusts, signed
// by that one's root and its own, the first by the root keys built into it, keeps the highest in system/updater and
// refuses a lower one (351-UPD-0026, 351-UPD-0027); it checks a channel's signatures, a slot's kept one included, with the
// release keys of the key file it trusts, a manifest's with the boot key, and every file it stages by size and SHA-256;
// the parser service reads the metadata (MC-11.11). It writes only the slot that did not boot and the boot records
// (vfs_server's update zone) and its key file, and confirms a trial boot on the disk once the kernel's deadline has passed
// without a restart. The Effector gateway may hand
// it a release as a bundle (751-UPD-0032), which it checks as any source. It keeps the running, the last known good and
// a staged release in the block store, pinned by its own owner, and a rollback writes the last known good back into a
// damaged slot from there (351-UPD-0033, kept.rs). Holds: its endpoint, the clock, the boot disk's client badged for the
// update zone and its private directory, the parser, init's client badged for a restart, the log, the flow grant the
// policy gives it, the TLS client and the block store's client badged OWNER_UPDATER.
extern crate alloc;

#[allow(dead_code)]
#[path = "../../bootloader/src/slots.rs"]
mod slots;
// The bootloader's check of a slot's key files, which the updater makes of a slot it writes (351-UPD-0029).
#[path = "../../bootloader/src/keychain.rs"]
mod keychain;
mod plan;
mod source;
mod bundle;
mod kept;
mod store;
mod keys { include!(concat!(env!("OUT_DIR"), "/keys.rs")); }

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use ed25519_dalek::{Signature, VerifyingKey};
use mind::abi::*;
use mind::fs::{Dir, File, MODE_CREATE, MODE_WRITE};
use mind::idl::update::{self as api, Error, Metadata, Offer, State};
use mind::idl::codec::{List, Text};
use mind::idl::{init, wire};
use mind::ipc::Endpoint;
use mind::parse::{read_channel, read_keys, read_manifest, read_order, read_timestamp, Refused, Session};
use mind::release::{self, Channel, Keys, Order, Step, Timestamp, CHANNEL_MAX, KEYS_MAX, MANIFEST_MAX, ORDER_MAX, TIMESTAMP_MAX};
use mind::zip; // the names a bundle may hold (bundle.rs)
use mind::{cid, dag, tree}; // kept.rs names them as crate::cid, crate::dag and crate::tree, as the host tests build it
use cid::Cid;
use dag::{Builder, CHUNK};
use slots::{Record, RECORD};
use plan::Kind;
use source::{copy, sha256, Bundle, Source};
use store::Store;

#[cfg(target_arch = "x86_64")]
const ARCH: &str = "x86_64";
#[cfg(target_arch = "aarch64")]
const ARCH: &str = "aarch64";
// The loader as a manifest lists it for this processor; a slot carries its copy (351-UPD-0010).
#[cfg(target_arch = "x86_64")]
const LOADER: &str = "EFI/BOOT/BOOTX64.EFI";
#[cfg(target_arch = "aarch64")]
const LOADER: &str = "EFI/BOOT/BOOTAA64.EFI";
const CONFIG: &str = "update.txt";
const RECORDS: [&str; 2] = ["MIND/BOOT0", "MIND/BOOT1"];
// The last trial init refused, and why (801-UPD-0037): in the updater's own directory, which every slot's updater reads.
const REFUSED: &str = "system/updater/refused";
const REFUSED_MAX: usize = 512;
const PARSE: Endpoint = Endpoint(SLOT_PARSE);
const ASK_MS: usize = 1000; // while a trial waits, init is asked this often how the boot went (init.wit 1.4, 351-UPD-0021)
const TRIES_MAX: u8 = 9;
const RETRY_MS: usize = 30_000; // an automatic run the source did not answer is tried again after this
const RECEIVED: usize = 9; // the buffer of an install call (update.wit 1.3) lands here
// A bundle the gateway hands over lies in the other slot's INSTALL directory, which neither the bootloader nor a fetch
// reads, until it is staged or dropped (751-UPD-0032).
const INSTALL: &str = "INSTALL";
const BUNDLE_MAX: u64 = 256 << 20;
// The blobs of kept releases read at most, to find those a release shares with them (351-UPD-0033).
const BLOBS_MAX: u64 = 4096;
// The key file it trusts (351-UPD-0027), in its private directory: written whole beside it, then put in its place, and
// read back from either.
const TRUSTED_DIR: &str = "system/updater";
const TRUSTED: &str = "system/updater/keys";
const TRUSTED_NEW: &str = "system/updater/keys.new";
// The highest timestamp version it took for its channel (351-UPD-0028): `<channel> <version>`, kept the same way.
const STAMP: &str = "system/updater/timestamp";
const STAMP_NEW: &str = "system/updater/timestamp.new";
// Every recovery order applied (351-UPD-0030): a line `<channel> <sequence> <version> <floor> <its SHA-256>` each.
const ORDERS: &str = "system/updater/recovery";
const ORDERS_NEW: &str = "system/updater/recovery.new";
const ORDERS_MAX: usize = 16384;
// The key files taken that changed the root (351-UPD-0029), `roots/<version>`: a slot carries those below its own key
// file as KEYS.1, KEYS.2, …, so that the bootloader follows its root from the one built in.
const ROOTS: &str = "system/updater/roots";

// About 110 KiB: kept here rather than on the stack; only `builder` takes it, and no call of it is reentrant.
static mut BUILDER: Builder = Builder::new();
fn builder() -> &'static mut Builder { unsafe { &mut *core::ptr::addr_of_mut!(BUILDER) } }

fn chunk() -> alloc::boxed::Box<[u8; CHUNK]> { alloc::vec![0u8; CHUNK].into_boxed_slice().try_into().unwrap() }

/// What the updater does by itself, from update.txt.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Automatic { None, Check, Fetch, Apply }

struct Config { source: Option<Source>, channel: String, automatic: Automatic, every_ms: usize, tries: u8 }

// update.txt: `source URL-OR-DIRECTORY`, `pin HEX`, `channel NAME`, `automatic none|check|fetch|apply`, `every SECONDS`,
// `tries N`; `#` starts a comment. A line it does not know is logged and left out.
fn config() -> Config {
    let mut config = Config { source: None, channel: String::from("stable"), automatic: Automatic::None, every_ms: 0, tries: 3 };
    let Ok(file) = File::open(CONFIG) else { mind::println!("[UPDATER] NO {}: IT ACTS ONLY WHEN ASKED, AND HAS NO SOURCE", CONFIG); return config };
    let mut text = alloc::vec![0u8; file.size().min(4096)];
    if file.read_at(0, &mut text).is_err() { return config; }
    let (mut url, mut pin) = (None, None);
    for line in String::from_utf8_lossy(&text).lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut words = line.split_whitespace();
        let (Some(key), Some(value), None) = (words.next(), words.next(), words.next()) else {
            if !line.is_empty() { mind::println!("[UPDATER] {}: LINE LEFT OUT: {}", CONFIG, line); }
            continue;
        };
        let known = match key {
            "source" => { url = Some(String::from(value)); true }
            "pin" => match hex::<32>(value.as_bytes()) { Some(key) => { pin = Some(key); true } None => false },
            "channel" => { config.channel = String::from(value); release::Text::<{ release::NAME_MAX }>::new(value.as_bytes()).is_some() }
            "automatic" => match value { "none" => { config.automatic = Automatic::None; true } "check" => { config.automatic = Automatic::Check; true }
                                         "fetch" => { config.automatic = Automatic::Fetch; true } "apply" => { config.automatic = Automatic::Apply; true } _ => false },
            "every" => value.parse::<usize>().map(|seconds| config.every_ms = seconds.saturating_mul(1000)).is_ok(),
            "tries" => value.parse::<u8>().ok().filter(|t| (1..=TRIES_MAX).contains(t)).map(|t| config.tries = t).is_some(),
            _ => false,
        };
        if !known { mind::println!("[UPDATER] {}: LINE LEFT OUT: {}", CONFIG, line); }
    }
    config.source = url.map(|url| if url.starts_with("https://") { Source::Https { base: url, pin } } else { Source::Directory(url) });
    if pin.is_some() && !matches!(config.source, Some(Source::Https { .. })) { mind::println!("[UPDATER] {}: A PIN IS FOR AN https:// SOURCE", CONFIG); }
    config
}

fn hex<const N: usize>(text: &[u8]) -> Option<[u8; N]> {
    if text.len() != 2 * N { return None; }
    let digit = |c: u8| (c as char).to_digit(16);
    let mut out = [0u8; N];
    for (i, pair) in text.chunks_exact(2).enumerate() { out[i] = (digit(pair[0])? * 16 + digit(pair[1])?) as u8; }
    Some(out)
}

fn short(digest: &[u8; 32]) -> String { digest[..8].iter().map(|b| format!("{:02x}", b)).collect() }

fn verified(key: &[u8; 32], message: &[u8], signature: &[u8]) -> bool {
    let (Ok(key), Ok(signature)) = (VerifyingKey::from_bytes(key), <[u8; 64]>::try_from(signature)) else { return false };
    key.verify_strict(message, &Signature::from_bytes(&signature)).is_ok()
}

/// Whether a boot key of `keys` signed `manifest` (351-UPD-0029): its boot role's, which asks for the one signature a
/// manifest carries.
fn boot_signed(keys: &Keys, manifest: &[u8], signature: &[u8]) -> bool {
    keys.role("boot").is_some_and(|boot| boot.threshold == 1 && boot.keys().iter().any(|key| verified(key, manifest, signature)))
}

/// The key files taken that changed the root, below version `below`, in order: those a slot carries before its own.
fn kept_roots(below: u64) -> Vec<Vec<u8>> {
    let mut versions = Vec::new();
    let _ = mind::fs::list(ROOTS, |entry| if let Ok(v) = entry.name_str().parse::<u64>() { if !entry.is_dir && v < below { versions.push(v); } });
    versions.sort_unstable();
    versions.iter().filter_map(|v| {
        let file = File::open(&format!("{}/{}", ROOTS, v)).ok().filter(|f| f.size() <= KEYS_MAX)?;
        let mut data = alloc::vec![0u8; file.size()];
        (file.read_at(0, &mut data) == Ok(data.len())).then_some(data)
    }).collect()
}

/// Key file `file` of `version`, which changed the root, kept for the slots it writes.
fn keep_root(version: u64, file: &[u8]) -> Result<(), mind::fs::Error> {
    match mind::fs::mkdir(TRUSTED_DIR) { Ok(()) | Err(mind::fs::Error::Exists) => {} Err(e) => return Err(e) }
    match mind::fs::mkdir(ROOTS) { Ok(()) | Err(mind::fs::Error::Exists) => {} Err(e) => return Err(e) }
    put_kept(&format!("{}/{}", ROOTS, version), &format!("{}/{}.new", ROOTS, version), file)
}

/// A slot's key files checked as the bootloader checks them (keychain.rs): under the root keys built in, its key file
/// not below `held`, the records' version. Its key file, or why not.
fn slot_keys(slot: &str, held: u64, parser: Endpoint) -> Result<Keys, keychain::Refusal> {
    let root = keychain::built_in(&keys::ROOT_KEYS, keys::ROOT_THRESHOLD);
    let read = |name: &str, buffer: &mut [u8]| -> Result<usize, &'static str> {
        let file = File::open(&format!("MIND/{}/{}", slot, name)).map_err(|_| "file not found")?;
        if file.size() >= buffer.len() { return Err("file larger than its buffer"); }
        if file.read_at(0, &mut buffer[..file.size()]) != Ok(file.size()) { return Err("read error"); }
        Ok(file.size())
    };
    keychain::check(root, held, read, |file| read_keys(parser, file).ok(), verify).map(|checked| checked.keys)
}

fn refused(why: Refused) -> Error { match why { Refused::Service => Error::NoParser, Refused::Malformed | Refused::Lied => Error::Malformed } }

/// A parser of the check's own, which the loader starts as a helper holding nothing but its endpoint, and which ends
/// when the check does (109-NET-0010). Only with a loader client in SLOT_LOADER (`init`'s, 109-KRN-0117); the shared
/// service without one, or where the loader starts none (logged with the reason).
fn parser() -> Session {
    if mind::dev::cap_info(SLOT_LOADER).0 != CAP_KIND_ENDPOINT { return Session::shared(PARSE); }
    Session::own(Endpoint::LOADER).unwrap_or_else(|why| { mind::println!("[UPDATER] PARSER: THE SHARED SERVICE ({:?})", why); Session::shared(PARSE) })
}

fn verify(key: &[u8; 32], body: &[u8], signature: &[u8; 64]) -> bool { verified(key, body, signature) }

/// A channel file whose fields `parser` read and which at least the threshold of the release keys `keys` names signed:
/// each key counted once, a line of any other key passed over (351-UPD-0026), so a key `keys` no longer names counts for
/// nothing (351-UPD-0027).
fn channel_of(file: &[u8], keys: &Keys, parser: Endpoint) -> Result<Channel, Error> {
    let (channel, signed) = read_channel(parser, file).map_err(refused)?;
    let role = keys.role("release").ok_or(Error::Signature)?;
    if !release::signed_by(role.keys(), role.threshold, &file[..signed.body], &signed, &verify) { return Err(Error::Signature); }
    Ok(channel)
}

/// The key file kept in system/updater (351-UPD-0027): of `keys` and `keys.new`, the highest version that its own root's
/// threshold signed, with its bytes and path; None without one.
fn kept_keys(parser: Endpoint) -> Option<(Keys, Vec<u8>, &'static str)> {
    [TRUSTED, TRUSTED_NEW].into_iter().filter_map(|path| {
        let file = File::open(path).ok().filter(|f| f.size() <= KEYS_MAX)?;
        let mut data = alloc::vec![0u8; file.size()];
        if file.read_at(0, &mut data) != Ok(data.len()) { return None; }
        let (keys, signed) = read_keys(parser, &data).ok()?;
        let root = keys.root();
        release::signed_by(root.keys(), root.threshold, &data[..signed.body], &signed, &verify).then_some((keys, data, path))
    }).max_by_key(|(keys, _, _)| keys.version)
}

/// `data` kept in system/updater as `path`: written whole as `new` beside it, then put in its place.
fn put_kept(path: &str, new: &str, data: &[u8]) -> Result<(), mind::fs::Error> {
    match mind::fs::mkdir(TRUSTED_DIR) { Ok(()) | Err(mind::fs::Error::Exists) => {} Err(e) => return Err(e) }
    let mut file = File::create(new)?;
    if file.write_at(0, data)? != data.len() { return Err(mind::fs::Error::Io); }
    file.flush()?;
    drop(file);
    match mind::fs::remove(path) { Ok(()) | Err(mind::fs::Error::NotFound) => {} Err(e) => return Err(e) }
    mind::fs::rename(new, path)
}

/// Key file `file` kept as the one trusted.
fn keep_keys(file: &[u8]) -> Result<(), mind::fs::Error> { put_kept(TRUSTED, TRUSTED_NEW, file) }

/// The account of the recovery orders applied (351-UPD-0030), as kept: `recovery`, else `recovery.new`.
fn kept_orders() -> String {
    [ORDERS, ORDERS_NEW].into_iter().find_map(|path| {
        let file = File::open(path).ok().filter(|f| f.size() <= ORDERS_MAX)?;
        let mut data = alloc::vec![0u8; file.size()];
        (file.read_at(0, &mut data) == Ok(data.len())).then(|| String::from_utf8(data).ok()).flatten()
    }).unwrap_or_default()
}

/// The highest sequence of a recovery order applied for channel `channel`, in the account `text`; 0 with none.
fn order_applied(text: &str, channel: &str) -> u64 {
    text.lines().filter_map(|line| {
        let mut words = line.split(' ');
        (words.next()? == channel).then(|| words.next()?.parse::<u64>().ok()).flatten()
    }).max().unwrap_or(0)
}

/// The highest timestamp version kept for channel `channel` (351-UPD-0028): of `timestamp` and `timestamp.new`, 0 without one.
fn kept_stamp(channel: &str) -> u64 {
    [STAMP, STAMP_NEW].into_iter().filter_map(|path| {
        let file = File::open(path).ok().filter(|f| f.size() <= 64)?;
        let mut data = alloc::vec![0u8; file.size()];
        if file.read_at(0, &mut data) != Ok(data.len()) { return None; }
        let text = core::str::from_utf8(&data).ok()?;
        let (name, version) = text.strip_suffix('\n')?.split_once(' ')?;
        (name == channel).then(|| version.parse::<u64>().ok()).flatten()
    }).max().unwrap_or(0)
}

/// The files of a manifest a slot holds, by name, size and SHA-256: the kernel, the services the bootloader loads, its
/// loader (351-UPD-0010) and the applications, which go with them (351-UPD-0023). The licences and data stay at the root.
fn slot_files(text: &[u8], parser: Endpoint) -> Result<Vec<release::File>, Error> {
    let mut files = Vec::new();
    read_manifest(parser, text, |file| files.push(*file)).map_err(refused)?;
    files.retain(|f| kind(f.path.as_str()) != Kind::Shared);
    // The bootloader loads neither a slot without these.
    if !["kernel.elf", "init.elf"].iter().all(|name| files.iter().any(|f| f.path.as_str() == *name)) { return Err(Error::Malformed); }
    Ok(files)
}

fn kind(path: &str) -> Kind { plan::kind(path, &BOOT_FILES, LOADER) }

fn slot_name(slot: u8) -> &'static str { if slot == b'A' { "A" } else { "B" } }

struct Updater {
    config: Config,
    /// The running slot (`A`, `B`, or 0 booted from the root), whether it booted on trial, its manifest's digest.
    slot: u8, trial: bool, manifest: [u8; 32],
    /// The running version (0: not known), the version staged in the other slot (0: none), the minimum last seen.
    running: u64, staged: u64, minimum: u64,
    /// The channel it holds (351-UPD-0025): the last one read from the source, else the running slot's; and which.
    held: Option<(Channel, Held)>,
    failure: String,
    /// The bundle the gateway is handing over, and the last handle given.
    install: Option<Install>, handles: u32,
    /// The block store, where it keeps releases (351-UPD-0033): None without one that outlives a reset.
    store: Option<Store>,
    /// The key file it trusts and its bytes (351-UPD-0027): the highest version it took; None before the first.
    trusted: Option<(Keys, Vec<u8>)>,
    /// The highest timestamp version it took for its channel (351-UPD-0028): a lower one is refused.
    stamp_seen: u64,
    /// The highest recovery order applied for its channel (351-UPD-0030), and the last one refused, said once.
    order_seen: u64, order_refused: Option<(u64, String)>,
}

/// Where the channel the updater holds came from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Held { Source, Bundle, Slot(u8) }

/// A bundle begun (`install-begin`): its size and SHA-256 as the gateway named them, the gateway's command, its file
/// in the staging and the bytes written so far.
struct Install { handle: u32, size: u64, sha256: [u8; 32], command: String, file: File, written: u64 }

/// Whether a channel's expiry has passed, or the clock cannot be read (MC-9.4).
fn expired(channel: &Channel) -> bool { plan::expired(release::expires_unix(&channel.expires), mind::rtc::unix_time()) }
fn keys_expired(keys: &Keys) -> bool { plan::expired(release::expires_unix(&keys.expires), mind::rtc::unix_time()) }

// A small file of a slot, at most `limit` bytes.
fn slot_file(slot: &str, name: &str, limit: usize) -> Option<Vec<u8>> {
    let file = File::open(&format!("MIND/{}/{}", slot, name)).ok().filter(|f| f.size() <= limit)?;
    let mut data = alloc::vec![0u8; file.size()];
    (file.read_at(0, &mut data) == Ok(data.len())).then_some(data)
}

impl Updater {
    fn records(&self) -> [Option<Record>; 2] {
        RECORDS.map(|path| {
            let file = File::open(path).ok()?;
            let mut data = [0u8; RECORD];
            (file.size() == RECORD && file.read_at(0, &mut data) == Ok(RECORD)).then(|| Record::parse(&data)).flatten()
        })
    }

    // Records of another format are not read (351-UPD-0024): said once at start; the next write replaces one.
    fn say_other_formats(&self) {
        for path in RECORDS {
            let Ok(file) = File::open(path) else { continue };
            let mut data = [0u8; RECORD];
            if file.size() != RECORD || file.read_at(0, &mut data) != Ok(RECORD) { continue; }
            if let Some(format) = Record::other_format(&data) { mind::println!("[UPDATER] RECORD {} OF FORMAT {}, NOT READ: WRITE THE DISK AGAIN", path, format); }
        }
    }

    // A record written whole into its file, in place, and read back.
    fn write_record(&self, (file, record): (usize, Record)) -> Result<(), Error> {
        let data = record.encode();
        let mut out = File::open_mode(RECORDS[file], MODE_WRITE).map_err(|_| Error::Write)?;
        if out.write_at(0, &data) != Ok(RECORD) { return Err(Error::Write); }
        drop(out);
        let mut back = [0u8; RECORD];
        let ok = File::open(RECORDS[file]).is_ok_and(|f| f.read_at(0, &mut back) == Ok(RECORD)) && Record::parse(&back) == Some(record);
        if !ok { return Err(Error::Write); }
        mind::println!("[UPDATER] RECORD {} SEQUENCE {}: SLOT {}, FALLBACK {}, {} TRIES, {}, FLOOR {}, KEY FILE {}", RECORDS[file], record.sequence, record.slot as char,
            if record.fallback == 0 { '-' } else { record.fallback as char }, record.tries, if record.confirmed { "CONFIRMED" } else { "NOT CONFIRMED" }, record.floor, record.keys);
        Ok(())
    }

    fn other(&self) -> &'static str { slot_name(plan::other(self.slot)) }

    /// The version of the key file it trusts (0: none) and the root that signs the next: that file's root role, else
    /// the root keys built in.
    fn root(&self) -> (u64, &[[u8; 32]], u8) {
        match &self.trusted { Some((k, _)) => (k.version, k.root().keys(), k.root().threshold), None => (0, &keys::ROOT_KEYS, keys::ROOT_THRESHOLD) }
    }

    /// Key file `file`, from `from`, taken as the one it trusts when it is the next version of the one trusted, signed by
    /// that one's root and its own (`release::step`, 351-UPD-0027), and kept in system/updater; the same one again is
    /// taken as it is. Otherwise refused, and said in the log unless `quiet`: a lower version brings revoked keys back.
    fn take(&mut self, file: &[u8], from: &str, quiet: bool, parser: Endpoint) -> Result<Keys, Error> {
        let (next, signed) = read_keys(parser, file).map_err(refused)?;
        let (held, keys, threshold) = self.root();
        let (refusal, error) = match release::step(held, keys, threshold, &next, &file[..signed.body], &signed, verify) {
            Step::Next => (None, Error::Signature),
            Step::Same if self.trusted.as_ref().is_some_and(|(k, _)| *k == next) => return Ok(next),
            Step::Same => (Some(String::from("ANOTHER FILE OF THE VERSION TAKEN BEFORE")), Error::Signature),
            Step::Lower => (Some(format!("BELOW VERSION {}, TAKEN BEFORE: IT WOULD BRING REVOKED KEYS BACK", held)), Error::Older),
            Step::Skips => (Some(format!("NOT THE NEXT OF VERSION {}", held)), Error::Missing),
            Step::Unsigned if !release::signed_by(keys, threshold, &file[..signed.body], &signed, &verify) =>
                (Some(if held == 0 { format!("FEWER THAN {} OF THE ROOT KEYS SIGNED IT", threshold) } else { format!("FEWER THAN {} OF VERSION {}'S ROOT KEYS SIGNED IT", threshold, held) }), Error::Signature),
            Step::Unsigned => (Some(format!("FEWER THAN {} OF ITS OWN ROOT KEYS SIGNED IT", next.root().threshold)), Error::Signature),
        };
        if let Some(why) = refusal {
            if !quiet { mind::println!("[UPDATER] KEY FILE VERSION {} REFUSED ({}): {}", next.version, from, why); }
            return Err(error);
        }
        // A file that changes the root, the one built in or the one before, is kept for the slots (351-UPD-0029).
        let before = self.trusted.as_ref().map_or(keychain::built_in(&keys::ROOT_KEYS, keys::ROOT_THRESHOLD), |(k, _)| *k.root());
        let rotated = before != *next.root();
        self.trusted = Some((next, Vec::from(file)));
        let kept = match keep_keys(file) { Ok(()) => format!("KEPT IN {}", TRUSTED), Err(e) => format!("NOT KEPT ({:?}): HELD UNTIL THE UPDATER ENDS", e) };
        let root = match rotated.then(|| keep_root(next.version, file)) {
            None => String::new(),
            Some(Ok(())) => format!(" AND {}", ROOTS),
            Some(Err(e)) => format!("; NOT IN {} ({:?}): A SLOT WRITTEN UNDER IT IS NOT BOOTED", ROOTS, e),
        };
        mind::println!("[UPDATER] KEY FILE VERSION {} TAKEN ({}){}, {}{}", next.version, from, if rotated { ", THE ROOT ROTATED" } else { "" }, kept, root);
        Ok(next)
    }

    /// The key file it trusts at start (351-UPD-0027): the one kept in system/updater, put in its place if a restart cut
    /// its write; then a slot's key file, the running one's first, when it is the next version (an image as written
    /// holds version 1 there).
    fn trust_kept(&mut self, parser: Endpoint) {
        match kept_keys(parser) {
            Some((keys, file, path)) => {
                mind::println!("[UPDATER] KEY FILE VERSION {} FROM {}, TAKEN BEFORE", keys.version, path);
                if path == TRUSTED_NEW { let _ = keep_keys(&file); }
                self.trusted = Some((keys, file));
            }
            None => mind::println!("[UPDATER] NO KEY FILE KEPT IN {}: THE ROOT KEYS BUILT IN SIGN THE FIRST", TRUSTED_DIR),
        }
        if self.slot == 0 { return; }
        for slot in [slot_name(self.slot), self.other()] {
            if let Some(file) = slot_file(slot, "KEYS", KEYS_MAX) { let _ = self.take(&file, &format!("SLOT {}", slot), true, parser); }
        }
    }

    /// Timestamp file `file` of the channel update.txt names (351-UPD-0028), if the timestamp keys of `keys` signed it to
    /// their threshold, its version is not below the one seen before and it has not expired; refused and said otherwise.
    fn timestamp_of(&self, file: &[u8], keys: &Keys, parser: Endpoint) -> Result<Timestamp, Error> {
        let (stamp, signed) = read_timestamp(parser, file).map_err(refused)?;
        let role = keys.role("timestamp").ok_or(Error::Signature)?;
        let refusal = if !release::signed_by(role.keys(), role.threshold, &file[..signed.body], &signed, &verify) {
            Some((format!("FEWER THAN {} OF THE TIMESTAMP KEYS OF KEY FILE VERSION {} SIGNED IT", role.threshold, keys.version), Error::Signature))
        } else if stamp.channel.as_str() != self.config.channel {
            Some((format!("IT IS CHANNEL {}'S", stamp.channel.as_str()), Error::Malformed))
        } else if stamp.version < self.stamp_seen {
            Some((format!("BELOW VERSION {}, TAKEN BEFORE", self.stamp_seen), Error::Older))
        } else if plan::expired(release::expires_unix(&stamp.expires), mind::rtc::unix_time()) {
            Some((format!("EXPIRED AT {}, OR THE CLOCK CANNOT BE READ", core::str::from_utf8(&stamp.expires).unwrap_or("")), Error::Expired))
        } else {
            None
        };
        if let Some((why, error)) = refusal {
            mind::println!("[UPDATER] TIMESTAMP VERSION {} REFUSED: {}", stamp.version, why);
            return Err(error);
        }
        Ok(stamp)
    }

    /// The recovery order `source` holds for the channel, `recovery/<channel>` (351-UPD-0030): applied when the recovery
    /// keys of `keys` signed it to their threshold, it is this channel's, its sequence is above the last one applied, it
    /// has not expired, the running slot is not on trial, and the other slot holds its version, checked as the
    /// bootloader will check it. Then it is kept in the account first, the floor lowered to its own and that slot booted
    /// on trial through init's restart. A refusal is said once and the check goes on; without an order, nothing.
    fn recovery(&mut self, source: &Source, keys: &Keys, parser: &mut Session) -> Result<(), Error> {
        let file = match source.small(&format!("recovery/{}", self.config.channel), ORDER_MAX, parser) {
            Ok(file) => file, Err(Error::Missing) => return Ok(()), Err(error) => return Err(error),
        };
        let (order, signed) = read_order(parser.endpoint(), &file).map_err(refused)?;
        let role = keys.role("recovery").ok_or(Error::Signature)?;
        let expires = core::str::from_utf8(&order.expires).unwrap_or("");
        let refusal = if !release::signed_by(role.keys(), role.threshold, &file[..signed.body], &signed, &verify) {
            Some(format!("FEWER THAN {} OF THE RECOVERY KEYS OF KEY FILE VERSION {} SIGNED IT", role.threshold, keys.version))
        } else if order.channel.as_str() != self.config.channel {
            Some(format!("IT IS CHANNEL {}'S", order.channel.as_str()))
        } else if order.sequence <= self.order_seen {
            Some(format!("NOT ABOVE ORDER {}, APPLIED BEFORE", self.order_seen))
        } else if plan::expired(release::expires_unix(&order.expires), mind::rtc::unix_time()) {
            Some(format!("EXPIRED AT {}, OR THE CLOCK CANNOT BE READ", expires))
        } else if self.may_stage().is_err() {
            Some(String::from("THE RUNNING SLOT IS ON TRIAL, OR NO SLOT RUNS"))
        } else {
            match self.verify_other() {
                Ok(channel) if channel.version == order.version => None,
                Ok(channel) => Some(format!("SLOT {} HOLDS VERSION {}, NOT {}", self.other(), channel.version, order.version)),
                Err(error) => Some(format!("SLOT {} IS NOT AS ITS MANIFEST LISTS IT ({:?})", self.other(), error)),
            }
        };
        if let Some(why) = refusal {
            if self.order_refused.as_ref() != Some(&(order.sequence, why.clone())) {
                mind::println!("[UPDATER] RECOVERY ORDER {} REFUSED: {}", order.sequence, why);
                self.order_refused = Some((order.sequence, why));
            }
            return Ok(());
        }
        let write = plan::recover(self.records(), self.slot, order.floor).ok_or(Error::Write)?;
        // The account first: an order a restart cuts is applied once, never again.
        self.keep_order(&order, &file)?;
        let floor = plan::floor(self.records());
        mind::println!("[UPDATER] RECOVERY ORDER {} OF CHANNEL {} APPLIED: VERSION {} IN SLOT {} ON TRIAL, THE FLOOR {} LOWERED TO {}; SIGNED BY THE RECOVERY KEYS OF KEY FILE VERSION {}, EXPIRES {}",
                       order.sequence, order.channel.as_str(), order.version, self.other(), floor, order.floor, keys.version, expires);
        self.write_record(write)?;
        self.restart()
    }

    /// The account of recovery orders with `order` added, kept in system/updater before the order acts.
    fn keep_order(&mut self, order: &Order, file: &[u8]) -> Result<(), Error> {
        let mut text = kept_orders();
        let digest: String = mind::sha256::digest(file).iter().map(|b| format!("{:02x}", b)).collect();
        text.push_str(&format!("{} {} {} {} {}\n", order.channel.as_str(), order.sequence, order.version, order.floor, digest));
        if text.len() > ORDERS_MAX { return Err(Error::Write); }
        put_kept(ORDERS, ORDERS_NEW, text.as_bytes()).map_err(|e| { mind::println!("[UPDATER] RECOVERY ORDER {} NOT KEPT ({:?}): NOT APPLIED", order.sequence, e); Error::Write })?;
        self.order_seen = order.sequence;
        Ok(())
    }

    /// The timestamp version taken, kept when it is above the one seen.
    fn keep_stamp(&mut self, stamp: &Timestamp) {
        if stamp.version <= self.stamp_seen { return; }
        self.stamp_seen = stamp.version;
        let text = format!("{} {}\n", stamp.channel.as_str(), stamp.version);
        if let Err(e) = put_kept(STAMP, STAMP_NEW, text.as_bytes()) { mind::println!("[UPDATER] TIMESTAMP VERSION {} NOT KEPT ({:?})", stamp.version, e); }
    }

    /// The key file `source` holds as `keys/current`, taken as the one it trusts (351-UPD-0027): first each version
    /// between the one trusted and it, from `keys/<version>`, each the next of the one before; then its expiry. With
    /// the file as read.
    fn trust_source(&mut self, source: &Source, parser: &mut Session) -> Result<(Keys, Vec<u8>), Error> {
        let current = source.small("keys/current", KEYS_MAX, parser)?;
        let (keys, _) = read_keys(parser.endpoint(), &current).map_err(refused)?;
        for version in self.root().0.saturating_add(1)..keys.version {
            let path = format!("keys/{}", version);
            let file = source.small(&path, KEYS_MAX, parser)?;
            if self.take(&file, &path, false, parser.endpoint())?.version != version { return Err(Error::Malformed); }
        }
        let keys = self.take(&current, "keys/current", false, parser.endpoint())?;
        if keys_expired(&keys) { mind::println!("[UPDATER] KEY FILE VERSION {} EXPIRED", keys.version); return Err(Error::Expired); }
        Ok((keys, current))
    }

    /// The channel kept in a slot, if the release keys of the key file it trusts signed it (351-UPD-0027: a slot whose
    /// channel only a key that file no longer names signed is no fallback, and its version is not known), and it names,
    /// for this architecture, the manifest `digest` (None: read the slot's MANIFEST); its version is the slot's. The
    /// caller's parser reads them, so that one session serves several reads (109-KRN-0133).
    fn kept(&self, slot: &str, digest: Option<[u8; 32]>, parser: Endpoint) -> Option<Channel> {
        let bytes = slot_file(slot, "CHANNEL", CHANNEL_MAX)?;
        let Some((keys, _)) = &self.trusted else { mind::println!("[UPDATER] SLOT {}: NO KEY FILE TRUSTED: ITS CHANNEL IS NOT READ", slot); return None };
        let channel = match channel_of(&bytes, keys, parser) {
            Ok(channel) => channel,
            Err(Error::Signature) => {
                let role = keys.role("release").map_or(0, |r| r.threshold);
                mind::println!("[UPDATER] SLOT {}: ITS CHANNEL IS SIGNED BY FEWER THAN {} OF THE RELEASE KEYS OF KEY FILE VERSION {}: NOT A FALLBACK", slot, role, keys.version);
                return None;
            }
            Err(_) => return None,
        };
        let digest = digest.or_else(|| {
            let manifest = File::open(&format!("MIND/{}/MANIFEST", slot)).ok()?;
            (manifest.size() <= MANIFEST_MAX).then(|| sha256(&manifest)).flatten()
        });
        (digest.is_some() && channel.manifest(ARCH) == digest).then_some(channel)
    }

    /// The period the running slot's release carries (351-UPD-0036) in place of update.txt's: its period.txt as the
    /// manifest booted lists it, holding `every` alone. Otherwise update.txt's stays, and the log says why.
    fn slot_period(&mut self, parser: Endpoint) {
        let slot = slot_name(self.slot);
        let root = self.config.every_ms / 1000;
        let read = |name: &str, limit: usize| -> Option<Vec<u8>> {
            let file = File::open(&format!("MIND/{}/{}", slot, name)).ok().filter(|f| f.size() <= limit)?;
            let mut data = alloc::vec![0u8; file.size()];
            (file.read_at(0, &mut data) == Ok(data.len())).then_some(data)
        };
        // The manifest the bootloader checked and booted: its digest is the launch record's.
        let Some(manifest) = read("MANIFEST", MANIFEST_MAX).filter(|m| mind::sha256::digest(m) == self.manifest) else {
            return mind::println!("[UPDATER] SLOT {}'S MANIFEST IS NOT THE ONE BOOTED: {}'S PERIOD ({} S) KEPT", slot, CONFIG, root);
        };
        let mut listed = None;
        if read_manifest(parser, &manifest, |f| if kind(f.path.as_str()) == Kind::Period { listed = Some(*f) }).is_err() {
            return mind::println!("[UPDATER] SLOT {}'S MANIFEST NOT READ: {}'S PERIOD ({} S) KEPT", slot, CONFIG, root);
        }
        let Some(f) = listed else { return };
        let Some(text) = read(plan::PERIOD, 4096).filter(|t| t.len() as u64 == f.size && mind::sha256::digest(t) == f.digest) else {
            return mind::println!("[UPDATER] SLOT {}: {} IS NOT AS THE MANIFEST LISTS IT: {}'S PERIOD ({} S) KEPT", slot, plan::PERIOD, CONFIG, root);
        };
        match plan::period(&String::from_utf8_lossy(&text)) {
            Ok(seconds) => {
                self.config.every_ms = (seconds as usize).saturating_mul(1000);
                mind::println!("[UPDATER] EVERY {} S, BY SLOT {}'S {} ({}'S: {} S)", seconds, slot, plan::PERIOD, CONFIG, root);
            }
            Err(line) => mind::println!("[UPDATER] SLOT {}: {} REFUSED ({}): {}'S PERIOD ({} S) KEPT", slot, plan::PERIOD, line, CONFIG, root),
        }
    }

    /// Holds `channel`, and says so in the log when it is another one or came from elsewhere.
    fn hold(&mut self, channel: Channel, from: Held) {
        if self.held == Some((channel, from)) { return; }
        self.held = Some((channel, from));
        let from = match from {
            Held::Source => String::from("FROM THE SOURCE"), Held::Bundle => String::from("FROM THE GATEWAY'S BUNDLE"),
            Held::Slot(slot) => format!("KEPT IN SLOT {}", slot as char),
        };
        let (expires, clock) = (core::str::from_utf8(&channel.expires).unwrap_or(""), mind::rtc::unix_time());
        mind::println!("[UPDATER] METADATA: CHANNEL {} VERSION {}, MINIMUM {}, EXPIRES {}, {}{}", channel.name.as_str(), channel.version,
            channel.minimum, expires, from, match clock {
                None => "; THE CLOCK CANNOT BE READ: FETCH AND APPLY WAIT FOR A CURRENT CHANNEL",
                Some(_) if expired(&channel) => "; EXPIRED: FETCH AND APPLY WAIT FOR A CURRENT CHANNEL",
                Some(_) => "",
            });
    }

    fn metadata(&self) -> Metadata {
        let Some((channel, from)) = self.held else { return Metadata::default() };
        Metadata { channel: Text::new(channel.name.as_str()).unwrap_or_default(), version: channel.version, minimum: channel.minimum,
                   expires: Text::new(core::str::from_utf8(&channel.expires).unwrap_or("")).unwrap_or_default(), expired: expired(&channel),
                   checked: !matches!(from, Held::Slot(_)) }
    }

    fn state(&self) -> State {
        let recorded = plan::newest(self.records());
        State { slot: self.slot, trial: self.trial, running: self.running, recorded: recorded.map_or(0, |(_, r)| r.slot), confirmed: recorded.is_some_and(|(_, r)| r.confirmed),
                staged: self.staged, failure: Text::new(&self.failure.chars().take(64).collect::<String>()).unwrap_or_default() }
    }

    fn fail<T>(&mut self, step: &str, error: Error) -> Result<T, Error> {
        mind::println!("[UPDATER] {}: REFUSED: {:?}", step.to_ascii_uppercase(), error);
        self.failure = format!("{}: {:?}", step, error);
        Err(error)
    }

    /// The source update.txt names, lent to `f` and put back.
    fn with_source<T>(&mut self, f: impl FnOnce(&mut Self, &Source) -> Result<T, Error>) -> Result<T, Error> {
        let Some(source) = self.config.source.take() else { return Err(Error::NoSource) };
        let result = f(self, &source);
        self.config.source = Some(source);
        result
    }

    /// The channel update.txt names as `source` holds it: the key file first, taken as the next of the one trusted, and
    /// its expiry (351-UPD-0026, 351-UPD-0027); then the channel's timestamp, and the channel file it names by SHA-256
    /// (351-UPD-0028); then the channel's release signatures, form and expiry, the shorter of its own and its
    /// timestamp's, and its version against the running one; with the channel's file and the key file as read.
    fn read_channel(&mut self, source: &Source, from: Held, parser: &mut Session) -> Result<(Channel, Vec<u8>, Vec<u8>), Error> {
        let (keys, keys_file) = self.trust_source(source, parser)?;
        // A recovery order for the channel first (351-UPD-0030): the authority's word over what the channel says.
        self.recovery(source, &keys, parser)?;
        let stamp_file = source.small(&format!("timestamps/{}", self.config.channel), TIMESTAMP_MAX, parser)?;
        let stamp = self.timestamp_of(&stamp_file, &keys, parser.endpoint())?;
        let bytes = source.small(&format!("channels/{}", self.config.channel), CHANNEL_MAX, parser)?;
        if mind::sha256::digest(&bytes) != stamp.digest {
            mind::println!("[UPDATER] TIMESTAMP VERSION {} NAMES ANOTHER FILE THAN channels/{}", stamp.version, self.config.channel);
            return Err(Error::Digest);
        }
        let mut channel = channel_of(&bytes, &keys, parser.endpoint())?;
        if channel.name.as_str() != self.config.channel { return Err(Error::Malformed); }
        self.keep_stamp(&stamp);
        // A channel is current for as long as its timestamp, at most (351-UPD-0028).
        if release::expires_unix(&stamp.expires) < release::expires_unix(&channel.expires) { channel.expires = stamp.expires; }
        self.hold(channel, from);
        // A clock that cannot be read cannot show the channel current (MC-9.4: expiry handled by a set policy).
        if expired(&channel) { return Err(Error::Expired); }
        self.minimum = self.minimum.max(channel.minimum);
        self.raise_floor();
        Ok((channel, bytes, keys_file))
    }

    fn check(&mut self) -> Result<Offer, Error> {
        let mut parser = parser();
        let (channel, _, _) = match self.with_source(|u, source| u.read_channel(source, Held::Source, &mut parser)) { Ok(read) => read, Err(error) => return self.fail("check", error) };
        let expires = core::str::from_utf8(&channel.expires).unwrap_or("");
        mind::println!("[UPDATER] CHECK {}: VERSION {}, MINIMUM {}, EXPIRES {}; RUNNING {}{}", channel.name.as_str(), channel.version, channel.minimum, expires,
            self.running, if self.running != 0 && self.running < channel.minimum { " (BELOW THE MINIMUM)" } else { "" });
        let offer = Offer { channel: Text::new(channel.name.as_str()).unwrap_or_default(), version: channel.version, minimum: channel.minimum,
                            expires: Text::new(expires).unwrap_or_default(), running: self.running };
        if channel.version <= self.running { return self.fail("check", Error::Older); }
        self.failure.clear();
        Ok(offer)
    }

    /// Whether the other slot may be written now.
    fn may_stage(&self) -> Result<(), Error> {
        if self.slot == 0 { return Err(Error::NoSlot); }
        if !plan::may_stage(self.records(), self.slot, self.trial) { return Err(Error::Trial); }
        Ok(())
    }

    fn fetch(&mut self) -> Result<u64, Error> {
        match self.with_source(|u, source| u.stage(source, Held::Source)) { Ok(version) => { self.failure.clear(); Ok(version) } Err(error) => self.fail("fetch", error) }
    }

    /// The newer release `source` holds, checked and staged in the other slot; `from` names the source in the log. Then
    /// kept in the block store, and the releases it no longer keeps let go (351-UPD-0033).
    fn stage(&mut self, source: &Source, from: Held) -> Result<u64, Error> {
        self.may_stage()?;
        let step = if from == Held::Bundle { "INSTALL" } else { "FETCH" };
        let mut parser = parser();
        let (channel, channel_file, keys_file) = self.read_channel(source, from, &mut parser)?;
        if channel.version <= self.running { return Err(Error::Older); }
        let (fetched, manifest, signature, files) = self.write_slot(source, step, &channel, &channel_file, &keys_file, &mut parser)?;
        drop(parser);
        self.staged = channel.version;
        mind::println!("[UPDATER] {}: VERSION {} STAGED IN SLOT {}: {} FILES, {} BYTES {}", step, channel.version, self.other(), files.len(), fetched,
                       if from == Held::Bundle { "FROM THE BUNDLE" } else { "FETCHED" });
        self.keep(self.other(), &channel, &channel_file, &keys_file, &manifest, &signature, &files);
        self.trim();
        Ok(channel.version)
    }

    /// Release `channel` as `source` holds it written into the other slot: its manifest checked against the channel and
    /// the boot key, then every file it lists for a slot by size and SHA-256, then the manifest, its signature, the key
    /// file and the channel. Returns the bytes `source` gave, the manifest, its signature and the files.
    fn write_slot(&mut self, source: &Source, step: &str, channel: &Channel, channel_file: &[u8], keys_file: &[u8], parser: &mut Session)
        -> Result<(u64, Vec<u8>, Vec<u8>, Vec<release::File>), Error> {
        let digest = channel.manifest(ARCH).ok_or(Error::Missing)?;
        let release = format!("releases/{}/{}", channel.version, ARCH);
        let manifest = source.small(&format!("{}/MANIFEST", release), MANIFEST_MAX, parser)?;
        if mind::sha256::digest(&manifest) != digest { return Err(Error::Digest); }
        let signature = source.small(&format!("{}/MANIFEST.SIG", release), 64, parser)?;
        // Signed by a boot key of the key file it trusts, which goes into the slot with the root changes before it.
        let (keys, _) = self.trusted.as_ref().ok_or(Error::Signature)?;
        if !boot_signed(keys, &manifest, &signature) {
            mind::println!("[UPDATER] {}: VERSION {}: ITS MANIFEST IS SIGNED BY NO BOOT KEY OF KEY FILE VERSION {}", step, channel.version, keys.version);
            return Err(Error::Signature);
        }
        let roots = kept_roots(keys.version);
        let files = slot_files(&manifest, parser.endpoint())?;
        let slot = self.other();
        let applications = files.iter().filter(|f| kind(f.path.as_str()) == Kind::Application).count();
        let period = files.iter().any(|f| kind(f.path.as_str()) == Kind::Period);
        mind::println!("[UPDATER] {}: VERSION {} INTO SLOT {}: {} BOOT FILES AND {} APPLICATIONS{}, MANIFEST {}", step, channel.version, slot,
                       files.len() - applications - period as usize, applications, if period { ", ITS period.txt" } else { "" }, short(&digest));
        // The channel goes first and comes back last: a slot holds one only once all of it is there. A boot file the
        // manifest does not list goes too (the bootloader refuses a slot with one); a listed one is resumed. A directory
        // stays: INSTALL holds the bundle being read.
        let dir = Dir::create(&format!("MIND/{}", slot)).map_err(|_| Error::Write)?;
        self.staged = 0;
        let mut stale = Vec::new();
        dir.list(|entry| if !entry.is_dir { stale.push(String::from(entry.name_str())); }).map_err(|_| Error::Write)?;
        let listed = |name: &str| files.iter().any(|f| plan::in_slot(f.path.as_str()).eq_ignore_ascii_case(name));
        for name in stale.iter().filter(|name| ["CHANNEL", "KEYS", "MANIFEST", "MANIFEST.SIG"].iter().any(|kept| kept.eq_ignore_ascii_case(name)) || !listed(name)) {
            dir.remove(name).map_err(|_| Error::Write)?;
        }
        let mut fetched = 0;
        for (index, f) in files.iter().enumerate() {
            let started = mind::time::uptime_ms();
            let path = format!("MIND/{}/{}", slot, plan::in_slot(f.path.as_str()));
            let mut file = File::open_mode(&path, MODE_WRITE | MODE_CREATE).map_err(|_| Error::Write)?;
            let whole = |file: &File| file.size() as u64 == f.size && sha256(file) == Some(f.digest);
            // An application the volume holds as the manifest lists it is copied whole, not fetched (351-UPD-0023).
            if kind(f.path.as_str()) == Kind::Application && !whole(&file) {
                if let Some((from, held)) = self.held(f) {
                    file.truncate(0).map_err(|_| Error::Write)?;
                    if copy(&held, &mut file, 0, f.size as usize).is_ok() && file.flush().is_ok() && whole(&file) {
                        mind::println!("[UPDATER] {}: {} OF {}: {}, {} BYTES COPIED FROM {} IN {} MS", step, index + 1, files.len(), f.path.as_str(), f.size, from,
                                       mind::time::uptime_ms() - started);
                        continue;
                    }
                    file.truncate(0).map_err(|_| Error::Write)?;
                }
            }
            let blob = format!("blobs/{}", f.digest.iter().map(|b| format!("{:02x}", b)).collect::<String>());
            let resumed = file.size() > 0;
            fetched += source.file(&blob, &mut file, f.size, parser)?;
            // What was there before may be another file's start: fetched again whole, once.
            if !whole(&file) && resumed {
                file.truncate(0).map_err(|_| Error::Write)?;
                fetched += source.file(&blob, &mut file, f.size, parser)?;
            }
            if !whole(&file) {
                // Not the file: the next fetch starts it again.
                let _ = file.truncate(0);
                mind::println!("[UPDATER] {}: {} IS NOT AS THE MANIFEST LISTS IT", step, f.path.as_str());
                return Err(Error::Digest);
            }
            let _ = file.flush();
            // Each file in the log, so that a slow or stalled fetch shows how far it got (351-UPD-0018).
            mind::println!("[UPDATER] {}: {} OF {}: {}, {} BYTES IN {} MS", step, index + 1, files.len(), f.path.as_str(), f.size, mind::time::uptime_ms() - started);
        }
        let names: Vec<&str> = keychain::ROTATIONS.iter().take(roots.len()).copied().collect();
        if roots.len() > names.len() { mind::println!("[UPDATER] {}: MORE ROOT CHANGES THAN A SLOT HOLDS", step); return Err(Error::Signature); }
        let signed = [("MANIFEST", &manifest[..]), ("MANIFEST.SIG", &signature[..])].into_iter();
        let chain = names.iter().zip(&roots).map(|(name, data)| (*name, &data[..])).chain([("KEYS", keys_file), ("CHANNEL", channel_file)]);
        for (name, data) in signed.chain(chain) {
            let mut file = File::create(&format!("MIND/{}/{}", slot, name)).map_err(|_| Error::Write)?;
            if file.write_at(0, data) != Ok(data.len()) || file.flush().is_err() { return Err(Error::Write); }
        }
        Ok((fetched, manifest, signature, files))
    }

    /// A copy of file `f` the volume holds as the manifest lists it, the running slot's or the root's, and which it is.
    fn held(&self, f: &release::File) -> Option<(&'static str, File)> {
        let running = (self.slot != 0).then(|| (if self.slot == b'A' { "SLOT A" } else { "SLOT B" }, format!("MIND/{}/{}", slot_name(self.slot), f.path.as_str())));
        running.into_iter().chain([("THE ROOT", String::from(f.path.as_str()))]).find_map(|(from, path)| {
            let file = File::open(&path).ok()?;
            (file.size() as u64 == f.size && sha256(&file) == Some(f.digest)).then_some((from, file))
        })
    }

    /// The other slot checked as the bootloader will check it, and the channel kept there.
    fn verify_other(&self) -> Result<Channel, Error> {
        let slot = self.other();
        let read = |name: &str, limit: usize| -> Result<Vec<u8>, Error> {
            let file = File::open(&format!("MIND/{}/{}", slot, name)).map_err(|_| Error::NotStaged)?;
            if file.size() > limit { return Err(Error::NotStaged); }
            let mut data = alloc::vec![0u8; file.size()];
            if file.read_at(0, &mut data) != Ok(data.len()) { return Err(Error::NotStaged); }
            Ok(data)
        };
        let (manifest, signature) = (read("MANIFEST", MANIFEST_MAX)?, read("MANIFEST.SIG", 64)?);
        // The metadata first: a session's own parser ends after 30 s without a request, less than hashing a slot takes
        // on a slow machine (parse's IDLE_MS).
        let parser = parser();
        // Its key files as the bootloader checks them, and its manifest signed by a boot key they name (351-UPD-0029).
        let held = plan::keys(self.records());
        let checked = slot_keys(slot, held, parser.endpoint()).or_else(|refusal| match self.refresh_keys(slot, &manifest, &signature) {
            true => slot_keys(slot, held, parser.endpoint()),
            false => Err(refusal),
        });
        match checked {
            Ok(keys) if boot_signed(&keys, &manifest, &signature) => {}
            Ok(keys) => { mind::println!("[UPDATER] SLOT {}: ITS MANIFEST IS SIGNED BY NO BOOT KEY OF ITS KEY FILE, VERSION {}", slot, keys.version); return Err(Error::Signature); }
            Err(refusal) => { mind::println!("[UPDATER] SLOT {}: {}: {}", slot, refusal, refusal.reason()); return Err(Error::Signature); }
        }
        let files = slot_files(&manifest, parser.endpoint())?;
        let shown = self.kept(slot, Some(mind::sha256::digest(&manifest)), parser.endpoint());
        drop(parser);
        for name in BOOT_FILES.iter().chain(["kernel.elf"].iter()) {
            let path = format!("MIND/{}/{}", slot, name);
            match (files.iter().find(|f| f.path.as_str() == *name), File::open(&path)) {
                (Some(f), Ok(file)) if file.size() as u64 == f.size && sha256(&file) == Some(f.digest) => {}
                // A boot file the manifest does not list must not be there: the bootloader refuses the slot.
                (None, Err(_)) => {}
                _ => { mind::println!("[UPDATER] SLOT {}: {} IS NOT AS THE MANIFEST LISTS IT", slot, name); return Err(Error::Digest); }
            }
        }
        // Its loader as the manifest lists it, and none it does not list (351-UPD-0010): the bootloader starts no other.
        // A slot staged before holds none, and the loader at EFI/BOOT/ loads it.
        let loader = files.iter().find(|f| kind(f.path.as_str()) == Kind::Loader);
        match (loader, File::open(&format!("MIND/{}/{}", slot, plan::in_slot(LOADER)))) {
            (Some(f), Ok(file)) if file.size() as u64 == f.size && sha256(&file) == Some(f.digest) => {}
            (_, Err(_)) => {}
            _ => { mind::println!("[UPDATER] SLOT {}: {} IS NOT AS THE MANIFEST LISTS IT", slot, plan::in_slot(LOADER)); return Err(Error::Digest); }
        }
        // Its period as the manifest lists it, and none it does not list (351-UPD-0036): a release refused by its
        // period is not booted.
        let period = files.iter().find(|f| kind(f.path.as_str()) == Kind::Period);
        let period_text = File::open(&format!("MIND/{}/{}", slot, plan::PERIOD)).ok().filter(|file| file.size() <= 4096).and_then(|file| {
            let mut data = alloc::vec![0u8; file.size()];
            (file.read_at(0, &mut data) == Ok(data.len())).then_some(data)
        });
        match (period, period_text) {
            (Some(f), Some(text)) if text.len() as u64 == f.size && mind::sha256::digest(&text) == f.digest => {
                if let Err(line) = plan::period(&String::from_utf8_lossy(&text)) {
                    mind::println!("[UPDATER] SLOT {}: {} REFUSED ({})", slot, plan::PERIOD, line);
                    return Err(Error::Malformed);
                }
            }
            (None, None) => {}
            _ => { mind::println!("[UPDATER] SLOT {}: {} IS NOT AS THE MANIFEST LISTS IT", slot, plan::PERIOD); return Err(Error::Digest); }
        }
        // A slot that holds applications holds every one the manifest lists, and none it does not (351-UPD-0023). One
        // staged before holds none, and the root's serve it.
        let mut held = Vec::new();
        Dir::open(&format!("MIND/{}", slot)).and_then(|dir| dir.list(|entry| if !entry.is_dir && kind(entry.name_str()) == Kind::Application {
            held.push(String::from(entry.name_str()));
        })).map_err(|_| Error::NotStaged)?;
        if !held.is_empty() {
            let listed = |name: &str| files.iter().find(|f| kind(f.path.as_str()) == Kind::Application && f.path.as_str().eq_ignore_ascii_case(name));
            if let Some(name) = held.iter().find(|name| listed(name).is_none()) {
                mind::println!("[UPDATER] SLOT {}: {} IS NOT IN THE MANIFEST", slot, name);
                return Err(Error::Digest);
            }
            for f in files.iter().filter(|f| kind(f.path.as_str()) == Kind::Application) {
                if !File::open(&format!("MIND/{}/{}", slot, f.path.as_str())).is_ok_and(|file| file.size() as u64 == f.size && sha256(&file) == Some(f.digest)) {
                    mind::println!("[UPDATER] SLOT {}: {} IS NOT AS THE MANIFEST LISTS IT", slot, f.path.as_str());
                    return Err(Error::Digest);
                }
            }
        }
        shown.ok_or(Error::NotStaged)
    }

    // Asks init to restart the machine; init answers first.
    fn restart(&self) -> Result<(), Error> {
        mind::println!("[UPDATER] ASKING INIT TO RESTART THE MACHINE");
        match init::reboot(Endpoint(SLOT_LIFECYCLE)) { Ok(Ok(())) => Ok(()), _ => Err(Error::Denied) }
    }

    fn apply(&mut self, tries: u8) -> Result<(), Error> {
        let result = (|| {
            if !(1..=TRIES_MAX).contains(&tries) { return Err(Error::NotStaged); }
            self.may_stage()?;
            // A staged slot not as its manifest lists it is written again from the store's copy (351-UPD-0034).
            let channel = match self.verify_other() { Ok(channel) => channel, Err(why) => self.restage(why)? };
            let version = channel.version;
            if version <= self.running { return Err(Error::Older); }
            // Past its channel's expiry, or the timestamp's of the same channel as last read (351-UPD-0028), a staged release
            // waits for a current one (351-UPD-0025); a rollback does not.
            let stamped = self.held.is_some_and(|(c, from)| matches!(from, Held::Source | Held::Bundle) && c.version == version && expired(&c));
            if expired(&channel) || stamped { return Err(Error::Expired); }
            let write = plan::trial_of_other(self.records(), self.slot, tries).ok_or(Error::Write)?;
            mind::println!("[UPDATER] APPLY: VERSION {} IN SLOT {} ON TRIAL WITH {} TRIES, FALLING BACK TO {}", version, self.other(), tries, slot_name(self.slot));
            self.write_record(write)?;
            self.restart()
        })();
        result.or_else(|error| self.fail("apply", error))
    }

    fn rollback(&mut self) -> Result<(), Error> {
        let result = (|| {
            self.may_stage()?;
            // A slot not as its manifest lists it is written again from the store's last known good (351-UPD-0033).
            let version = match self.verify_other() { Ok(channel) => channel.version, Err(why) => self.restore(why)? };
            if version < self.minimum { return Err(Error::NoFallback); }
            let write = plan::trial_of_other(self.records(), self.slot, 1).ok_or(Error::Write)?;
            mind::println!("[UPDATER] ROLLBACK: VERSION {} IN SLOT {} ON TRIAL, FALLING BACK TO {}", version, self.other(), slot_name(self.slot));
            self.write_record(write)?;
            self.restart()
        })();
        result.or_else(|error| self.fail("rollback", error))
    }

    /// The releases the block store keeps for the updater: each pin's id and root, and the version its tree holds. A pin
    /// that holds no release is logged and left as it is.
    fn stored(&self) -> Vec<(u32, Cid, u64)> {
        let Some(store) = &self.store else { return Vec::new() };
        let pins = match store.pins() { Ok(pins) => pins, Err(e) => { mind::println!("[UPDATER] STORE: ITS PINS NOT READ ({:?})", e); return Vec::new() } };
        let mut buffer = chunk();
        pins.into_iter().filter_map(|(id, root, _)| match kept::version(&mut Store, &root, ARCH, &mut buffer) {
            Ok(version) => Some((id, root, version)),
            Err(e) => { mind::println!("[UPDATER] STORE: PIN {} HOLDS NO RELEASE ({:?}): LEFT AS IT IS", id, e); None }
        }).collect()
    }

    /// File `f` of slot `slot` put into the store as an object, checked by size and SHA-256 as it is read: the slot's
    /// copy, else the root's (an application a slot staged before 351-UPD-0023 does not hold).
    fn put_file(&self, slot: &str, f: &release::File) -> Result<kept::Blob, String> {
        let mut buffer = alloc::vec![0u8; mind::fs::CHUNK];
        for path in [format!("MIND/{}/{}", slot, plan::in_slot(f.path.as_str())), String::from(f.path.as_str())] {
            let Ok(file) = File::open(&path) else { continue };
            if file.size() as u64 != f.size { continue; }
            let builder = builder();
            builder.reset();
            let (mut hash, mut at) = (mind::sha256::Sha256::new(), 0);
            while at < file.size() {
                let got = file.read_at(at, &mut buffer).map_err(|_| format!("{} NOT READ", path))?;
                if got == 0 { return Err(format!("{} NOT READ", path)); }
                hash.update(&buffer[..got]);
                builder.write(&mut Store, &buffer[..got]).map_err(|e| format!("THE STORE REFUSED A BLOCK OF {} ({:?})", path, e))?;
                at += got;
            }
            if hash.finish() != f.digest { continue; }
            let (cid, size) = builder.finish(&mut Store).map_err(|e| format!("THE STORE REFUSED A BLOCK OF {} ({:?})", path, e))?;
            return Ok(kept::Blob { digest: f.digest, size, cid });
        }
        Err(format!("{} IS NOT AS THE MANIFEST LISTS IT", f.path.as_str()))
    }

    /// Release `channel` of slot `slot` kept in the block store, pinned, unless it is already (351-UPD-0033): each file
    /// its manifest lists for a slot put from the slot (`put_file`), or taken from a kept release that holds its blob.
    /// Logged; a failure changes nothing else.
    fn keep(&mut self, slot: &str, channel: &Channel, channel_file: &[u8], keys_file: &[u8], manifest: &[u8], signature: &[u8], files: &[release::File]) {
        let Some(store) = &self.store else { return };
        let (started, releases) = (mind::time::uptime_ms(), self.stored());
        if releases.iter().any(|r| r.2 == channel.version) { return; }
        let (mut buffer, mut known) = (chunk(), BTreeMap::new());
        let mut left = BLOBS_MAX;
        for (_, root, _) in &releases {
            if let Err(e) = kept::blobs(&mut Store, root, &mut buffer, &mut left, |b| { known.insert(b.digest, b); }) {
                mind::println!("[UPDATER] STORE: THE BLOBS OF {} NOT READ ({:?})", root, e);
            }
        }
        let (mut blobs, mut put, mut taken) = (Vec::with_capacity(files.len()), 0u64, 0);
        for f in files {
            if let Some(b) = known.get(&f.digest).filter(|b: &&kept::Blob| b.size == f.size) { blobs.push(*b); taken += 1; continue; }
            match self.put_file(slot, f) {
                Ok(b) => { put += b.size; known.insert(b.digest, b); blobs.push(b); }
                Err(why) => { mind::println!("[UPDATER] STORE: VERSION {} NOT KEPT: {}", channel.version, why); return; }
            }
        }
        let release = kept::Release { keys_file, channel: channel.name.as_str(), channel_file, version: channel.version, arch: ARCH, manifest, signature };
        let root = match kept::put(&mut Store, builder(), &release, &blobs, &mut buffer) {
            Ok(root) => root,
            Err(e) => { mind::println!("[UPDATER] STORE: VERSION {} NOT KEPT: ITS TREE REFUSED ({:?})", channel.version, e); return }
        };
        match store.pin(&root) {
            Ok(id) => mind::println!("[UPDATER] STORE: VERSION {} KEPT AS PIN {}, ROOT {}: {} FILES, {} BYTES PUT FROM SLOT {}, {} TAKEN FROM A KEPT RELEASE, IN {} MS",
                                     channel.version, id, root, files.len(), put, slot, taken, mind::time::uptime_ms() - started),
            // A block put over a minute before may have gone in a collection meanwhile: the next try puts it again.
            Err(e) => mind::println!("[UPDATER] STORE: VERSION {} NOT KEPT: THE PIN REFUSED ({:?})", channel.version, e),
        }
    }

    /// The running release kept in the store if it is not yet: its slot's manifest the one booted and signed by the
    /// boot key, and its channel signed by the release keys of the key file it trusts, naming it (351-UPD-0033,
    /// 351-UPD-0027). Not on trial: the confirmation comes first.
    fn keep_running(&mut self) {
        if self.store.is_none() || self.may_stage().is_err() || self.running == 0 || self.stored().iter().any(|r| r.2 == self.running) { return; }
        let slot = slot_name(self.slot);
        let read = |name: &str, limit: usize| slot_file(slot, name, limit);
        let (Some(manifest), Some(signature), Some(channel_file), Some(keys_file)) =
            (read("MANIFEST", MANIFEST_MAX), read("MANIFEST.SIG", 64), read("CHANNEL", CHANNEL_MAX), read("KEYS", KEYS_MAX)) else {
            return mind::println!("[UPDATER] STORE: SLOT {} LACKS ITS MANIFEST, SIGNATURE, CHANNEL OR KEY FILE: VERSION {} NOT KEPT", slot, self.running);
        };
        let parser = parser();
        let booted = slot_keys(slot, 0, parser.endpoint()).is_ok_and(|keys| boot_signed(&keys, &manifest, &signature));
        if mind::sha256::digest(&manifest) != self.manifest || !booted {
            return mind::println!("[UPDATER] STORE: SLOT {}'S MANIFEST IS NOT THE ONE BOOTED: VERSION {} NOT KEPT", slot, self.running);
        }
        let checked = self.trusted.as_ref().and_then(|(keys, _)| channel_of(&channel_file, keys, parser.endpoint()).ok())
            .filter(|c| c.version == self.running && c.manifest(ARCH) == Some(self.manifest))
            .map(|c| (c, slot_files(&manifest, parser.endpoint())));
        drop(parser);
        match checked {
            Some((channel, Ok(files))) => self.keep(slot, &channel, &channel_file, &keys_file, &manifest, &signature, &files),
            _ => mind::println!("[UPDATER] STORE: SLOT {}'S CHANNEL OR MANIFEST NOT READ: VERSION {} NOT KEPT", slot, self.running),
        }
    }

    /// The pins of the releases the store no longer keeps ended (`plan::keeps`): the running one, the last known good
    /// and a staged one stay. Never while the running slot is on trial, since the other one is then its fallback.
    fn trim(&mut self) {
        if self.store.is_none() || self.may_stage().is_err() { return; }
        let releases = self.stored();
        let versions: Vec<u64> = releases.iter().map(|r| r.2).collect();
        let minimum = self.minimum.max(plan::floor(self.records()));
        let mut held: Vec<u64> = Vec::new();
        for &(id, _, version) in &releases {
            if plan::keeps(version, &versions, self.running, minimum, self.staged) && !held.contains(&version) { held.push(version); continue; }
            let Some(store) = &self.store else { return };
            match store.unpin(id) {
                Ok(()) => mind::println!("[UPDATER] STORE: VERSION {} LET GO (PIN {})", version, id),
                Err(e) => mind::println!("[UPDATER] STORE: VERSION {}: PIN {} NOT ENDED ({:?})", version, id, e),
            }
        }
        held.sort_unstable_by(|a, b| b.cmp(a));
        let listed: Vec<String> = held.iter().map(|v| format!("{}", v)).collect();
        mind::println!("[UPDATER] STORE KEEPS {}", if listed.is_empty() { String::from("NO RELEASE") } else { format!("VERSIONS {}", listed.join(", ")) });
    }

    /// The last known good release the store keeps written into the other slot, as a fetch writes a source's, since
    /// that slot is not as its manifest lists it (`why`); its version, once the slot checks as the bootloader will
    /// check it (351-UPD-0033). `no-fallback` without such a release.
    fn restore(&mut self, why: Error) -> Result<u64, Error> {
        if self.store.is_none() { return Err(Error::NoFallback); }
        let releases = self.stored();
        let versions: Vec<u64> = releases.iter().map(|r| r.2).collect();
        let version = plan::last_good(&versions, self.running, self.minimum.max(plan::floor(self.records()))).ok_or(Error::NoFallback)?;
        let root = releases.iter().find(|r| r.2 == version).map(|r| r.1).ok_or(Error::NoFallback)?;
        mind::println!("[UPDATER] RESTORE: SLOT {} IS NOT AS ITS MANIFEST LISTS IT ({:?}): VERSION {} FROM THE STORE, ROOT {}", self.other(), why, version, root);
        self.from_store(&root, version, "RESTORE")?;
        self.verify_other().map(|c| c.version)
    }

    /// The release staged in the other slot written there again from the block store, since that slot is not as its
    /// manifest lists it (`why`): only the version staged, above the running one and kept by the store (351-UPD-0034).
    /// Its channel once the slot checks as the bootloader will check it; `why` again without such a release.
    fn restage(&mut self, why: Error) -> Result<Channel, Error> {
        let version = self.staged;
        if self.store.is_none() || version <= self.running { return Err(why); }
        let Some(root) = self.stored().iter().find(|r| r.2 == version).map(|r| r.1) else { return Err(why) };
        mind::println!("[UPDATER] APPLY: SLOT {} IS NOT AS ITS MANIFEST LISTS IT ({:?}): VERSION {} FROM THE STORE, ROOT {}", self.other(), why, version, root);
        self.from_store(&root, version, "APPLY")?;
        self.staged = version;
        self.verify_other()
    }

    /// Release `version`, kept in the store at `root`, written into the other slot as a fetch writes a source's: its
    /// channel the one kept with it, signed by the release keys of the key file it trusts (351-UPD-0027), and that key
    /// file beside it; every file checked by size and SHA-256; `step` names it in the log.
    fn from_store(&mut self, root: &Cid, version: u64, step: &str) -> Result<(), Error> {
        let name = kept::channel(&mut Store, root, &mut chunk()).map_err(|_| Error::Digest)?;
        let source = Source::Store(*root);
        let mut parser = parser();
        let (keys, keys_file) = self.trusted.clone().ok_or(Error::Signature)?;
        let channel_file = source.small(&format!("channels/{}", name), CHANNEL_MAX, &mut parser)?;
        let channel = channel_of(&channel_file, &keys, parser.endpoint())?;
        if channel.version != version || channel.name.as_str() != name { return Err(Error::Digest); }
        let (read, _, _, files) = self.write_slot(&source, step, &channel, &channel_file, &keys_file, &mut parser)?;
        drop(parser);
        mind::println!("[UPDATER] {}: VERSION {} WRITTEN INTO SLOT {} AGAIN: {} FILES, {} BYTES FROM THE STORE", step, version, self.other(), files.len(), read);
        Ok(())
    }

    /// The releases the block store keeps, newest first, at most 8 (update.wit 1.5 `kept`): each one's bytes those of
    /// its blobs, each counted once; 0 when its tree is not read, which the log says.
    fn releases_kept(&self) -> Vec<api::KeptRelease> {
        let mut releases = self.stored();
        releases.sort_unstable_by(|a, b| b.2.cmp(&a.2));
        let mut buffer = chunk();
        releases.into_iter().take(8).map(|(pin, root, version)| {
            let (mut left, mut seen, mut bytes) = (BLOBS_MAX, BTreeMap::new(), 0u64);
            let read = kept::blobs(&mut Store, &root, &mut buffer, &mut left, |b| if seen.insert(b.digest, ()).is_none() { bytes += b.size; });
            if let Err(e) = read { mind::println!("[UPDATER] STORE: THE BLOBS OF {} NOT READ ({:?})", root, e); bytes = 0; }
            api::KeptRelease { version, pin, root: List::from_slice(&root.to_bytes()).unwrap_or_default(), bytes }
        }).collect()
    }

    fn bundle_dir(&self) -> String { format!("MIND/{}/{}", self.other(), INSTALL) }

    /// The bundle in the staging and its directory removed; `why` in the log when there was one.
    fn drop_bundle(&mut self, why: &str) {
        self.install = None;
        let (dir, path) = (self.bundle_dir(), format!("{}/BUNDLE.ZIP", self.bundle_dir()));
        let had = File::open(&path).is_ok();
        let _ = mind::fs::remove(&path);
        let _ = mind::fs::remove(&dir);
        if had && !why.is_empty() { mind::println!("[UPDATER] INSTALL: THE BUNDLE IN {} REMOVED: {}", dir, why); }
    }

    /// A bundle of `size` bytes with SHA-256 `sha256` begun in the staging, for the gateway's command `command`.
    fn install_begin(&mut self, size: u64, sha256: &str, command: &str) -> Result<u32, Error> {
        let result = (|| {
            self.may_stage()?;
            let sha256 = hex::<32>(sha256.as_bytes()).ok_or(Error::Malformed)?;
            if size == 0 || size > BUNDLE_MAX { return Err(Error::Malformed); }
            self.drop_bundle("ANOTHER BEGUN");
            Dir::create(&format!("MIND/{}", self.other())).and_then(|_| Dir::create(&self.bundle_dir())).map_err(|_| Error::Write)?;
            let file = File::create(&format!("{}/BUNDLE.ZIP", self.bundle_dir())).map_err(|_| Error::Write)?;
            self.handles = self.handles.wrapping_add(1).max(1);
            mind::println!("[UPDATER] INSTALL: A BUNDLE OF {} BYTES, SHA-256 {}, FOR THE GATEWAY'S COMMAND {}: HANDLE {}", size, short(&sha256), command, self.handles);
            self.install = Some(Install { handle: self.handles, size, sha256, command: String::from(command), file, written: 0 });
            Ok(self.handles)
        })();
        result.or_else(|error| self.fail("install", error))
    }

    /// The bundle's bytes at `offset`, at most the bytes written so far: what lay at and past it goes.
    fn install_write(&mut self, handle: u32, offset: u64, bytes: &[u8]) -> Result<(), Error> {
        let result = (|| {
            let install = self.install.as_mut().filter(|i| i.handle == handle).ok_or(Error::Malformed)?;
            if offset > install.written || offset + bytes.len() as u64 > install.size { return Err(Error::Malformed); }
            if offset < install.written {
                mind::println!("[UPDATER] INSTALL: WRITTEN AGAIN FROM BYTE {} OF {}", offset, install.size);
                install.file.truncate(offset as usize).map_err(|_| Error::Write)?;
            }
            if install.file.write_at(offset as usize, bytes) != Ok(bytes.len()) { return Err(Error::Write); }
            install.written = offset + bytes.len() as u64;
            Ok(())
        })();
        result.or_else(|error| self.fail("install", error))
    }

    /// The bundle checked by size and SHA-256, its release staged as `fetch` stages a source's, and the bundle removed.
    fn install_commit(&mut self, handle: u32) -> Result<u64, Error> {
        if !self.install.as_ref().is_some_and(|i| i.handle == handle) { return self.fail("install", Error::Malformed); }
        let Some(install) = self.install.take() else { return self.fail("install", Error::Malformed) };
        let result = (|| {
            let _ = install.file.flush();
            let whole = install.written == install.size && sha256(&install.file) == Some(install.sha256);
            mind::println!("[UPDATER] INSTALL: COMMAND {}: {} OF {} BYTES{}", install.command, install.written, install.size,
                           if whole { ", SHA-256 AS BEGUN" } else { ": NOT THE SIZE OR SHA-256 BEGUN" });
            drop(install.file);
            if !whole { return Err(Error::Digest); }
            let bundle = Bundle::open(&format!("{}/BUNDLE.ZIP", self.bundle_dir()))?;
            mind::println!("[UPDATER] INSTALL: THE BUNDLE HOLDS {} FILES", bundle.len());
            self.stage(&Source::Bundle(bundle), Held::Bundle)
        })();
        self.drop_bundle("");
        match result { Ok(version) => { self.failure.clear(); Ok(version) } Err(error) => self.fail("install", error) }
    }

    /// On a trial boot the kernel restarts the machine at its deadline unless init confirmed the boot: once init says it
    /// did, or past that deadline, the record says so on the disk; `why` says which.
    // A trial init refused (801-UPD-0037): its reason kept in system/updater/refused for the slot before to report (S2), the
    // trial spent on the disk so that the next boot falls back to that slot at once, and init asked to restart.
    fn refused(&mut self, boot: &init::Boot) {
        let why = if boot.why.as_str().is_empty() { "NO REASON GIVEN" } else { boot.why.as_str() };
        mind::println!("[UPDATER] INIT REFUSED THE TRIAL BOOT: {}", why);
        let records = self.records();
        let fallback = plan::newest(records).map_or(0, |(_, r)| r.fallback);
        let version = if self.running == 0 { String::from("not known") } else { format!("{}", self.running) };
        let log = if boot.log == 0 { String::from("none") } else { format!("log:boot{:04}.log", boot.log) };
        let text = format!("refused version {} in slot {}, falling back to slot {}\nwhy {}\nlog {}\n", version, slot_name(self.slot), slot_name(fallback), why, log);
        let made = matches!(mind::fs::mkdir(TRUSTED_DIR), Ok(()) | Err(mind::fs::Error::Exists));
        let kept = made && File::create(REFUSED).is_ok_and(|mut file| file.write_at(0, &text.as_bytes()[..text.len().min(REFUSED_MAX)]).is_ok());
        mind::println!("[UPDATER] THE REASON {} {} (VERSION {}, THIS BOOT'S LOG {})", if kept { "KEPT IN" } else { "NOT WRITTEN TO" }, REFUSED, version, log);
        match plan::spend(records, self.slot) {
            Some(write) => match self.write_record(write) {
                Ok(()) => mind::println!("[UPDATER] SLOT {} SPENT: THE NEXT BOOT FALLS BACK TO SLOT {}", slot_name(self.slot), slot_name(fallback)),
                Err(_) => mind::println!("[UPDATER] SLOT {} NOT SPENT, ITS RECORD NOT WRITTEN: THE BOOTLOADER COUNTS ITS TRIES", slot_name(self.slot)),
            },
            None => mind::println!("[UPDATER] SLOT {}: NO TRY LEFT TO SPEND", slot_name(self.slot)),
        }
        let _ = self.restart();
    }

    // A trial refused before (801-UPD-0037): said at start, and the update state's failure until a trial is confirmed.
    fn refused_before(&mut self) {
        let Ok(file) = File::open(REFUSED) else { return };
        let mut data = [0u8; REFUSED_MAX];
        let n = file.read_at(0, &mut data).unwrap_or(0).min(file.size());
        let text = core::str::from_utf8(&data[..n]).unwrap_or("");
        let (what, why) = (text.lines().next().unwrap_or(""), text.lines().find_map(|l| l.strip_prefix("why ")).unwrap_or(""));
        mind::println!("[UPDATER] A TRIAL REFUSED BEFORE ({}): {}; {}", REFUSED, what, why);
        self.failure = format!("{}: {}", what.strip_prefix("refused ").unwrap_or(what), why);
    }

    fn confirm(&mut self, why: &str) {
        let records = self.records();
        if plan::confirm(records, self.slot, 0).is_none() {
            return mind::println!("[UPDATER] SLOT {}: THE RECORD NEEDS NO CONFIRMATION", slot_name(self.slot));
        }
        let keys = self.keys_to_hold(records);
        let Some(write) = plan::confirm(records, self.slot, keys) else { return };
        match self.write_record(write) {
            Ok(()) => {
                mind::println!("[UPDATER] SLOT {} CONFIRMED ON THE DISK: {}", slot_name(self.slot), why);
                // A trial refused before is answered by this one (801-UPD-0037).
                if mind::fs::remove(REFUSED).is_ok() { mind::println!("[UPDATER] {} REMOVED: A TRIAL WAS CONFIRMED SINCE", REFUSED); }
                if keys > plan::keys(records) { mind::println!("[UPDATER] KEY FILE VERSION {} IN THE RECORDS: A SLOT WITH A LOWER ONE IS NOT BOOTED (MC-9.6)", keys); }
                self.raise_floor();
                self.keep_running();
                self.trim();
            }
            Err(error) => { let _ = self.fail::<()>("confirm", error); }
        }
    }

    /// The key file version the confirmation writes into the records (351-UPD-0029): the running slot's, never lower
    /// than theirs. When it rises, the other slot gets the key files the updater trusts first if a boot key they name
    /// signed its manifest, and stays a fallback; otherwise it is not booted from then on, which the log says.
    fn keys_to_hold(&self, records: [Option<Record>; 2]) -> u64 {
        let held = plan::keys(records);
        let parser = parser();
        let running = match slot_keys(slot_name(self.slot), 0, parser.endpoint()) {
            Ok(keys) => keys.version,
            Err(refusal) => { mind::println!("[UPDATER] SLOT {}: {}: THE RECORDS' KEY FILE VERSION STAYS", slot_name(self.slot), refusal); return held; }
        };
        if running <= held { return held; }
        let other = self.other();
        if slot_keys(other, running, parser.endpoint()).is_err() {
            let read = |name: &str, limit: usize| slot_file(other, name, limit);
            if let (Some(manifest), Some(signature)) = (read("MANIFEST", MANIFEST_MAX), read("MANIFEST.SIG", 64)) {
                if !self.refresh_keys(other, &manifest, &signature) {
                    mind::println!("[UPDATER] SLOT {}: SIGNED BY NO BOOT KEY OF KEY FILE VERSION {}: NOT A FALLBACK FROM NOW ON", other, running);
                }
            }
        }
        running
    }

    /// The version floor raised (351-UPD-0024), as `plan::raise` says: a record of its own, written like the others.
    fn raise_floor(&self) {
        if self.slot == 0 { return; }
        // The running slot's version as its manifest names it, the one the bootloader compares: never a floor above it.
        let version = File::open(&format!("MIND/{}/MANIFEST", slot_name(self.slot))).ok().filter(|f| f.size() <= MANIFEST_MAX).and_then(|f| {
            let mut text = alloc::vec![0u8; f.size()];
            (f.read_at(0, &mut text) == Ok(text.len()) && mind::sha256::digest(&text) == self.manifest).then(|| plan::manifest_version(&text))
        }).unwrap_or(0);
        let records = self.records();
        let Some(write) = plan::raise(records, self.slot, self.minimum, version) else { return };
        match self.write_record(write) {
            Ok(()) => mind::println!("[UPDATER] VERSION FLOOR {} RAISED TO {}: A SLOT BELOW IT IS NOT BOOTED (MC-9.4)", plan::floor(records), write.1.floor),
            Err(_) => mind::println!("[UPDATER] VERSION FLOOR: NOT WRITTEN"),
        }
    }

    /// The key files the updater trusts written into `slot` over its own (351-UPD-0029), when a boot key they name signed
    /// its manifest: the root changes before it, then the key file. Whether they were.
    fn refresh_keys(&self, slot: &str, manifest: &[u8], signature: &[u8]) -> bool {
        let Some((keys, file)) = &self.trusted else { return false };
        if !boot_signed(keys, manifest, signature) { return false; }
        let roots = kept_roots(keys.version);
        if roots.len() > keychain::ROTATIONS_MAX { return false; }
        let write = |name: &str, data: &[u8]| File::create(&format!("MIND/{}/{}", slot, name)).is_ok_and(|mut f| f.write_at(0, data) == Ok(data.len()) && f.flush().is_ok());
        let written = keychain::ROTATIONS.iter().zip(&roots).all(|(name, data)| write(name, data)) && write("KEYS", file);
        for name in &keychain::ROTATIONS[roots.len()..] { let _ = mind::fs::remove(&format!("MIND/{}/{}", slot, name)); }
        if written { mind::println!("[UPDATER] SLOT {}: KEY FILE VERSION {} WRITTEN THERE, A BOOT KEY OF WHICH SIGNED ITS MANIFEST", slot, keys.version); }
        written
    }

    // What update.txt asks it to do by itself; whether to try again soon, since the source did not answer.
    fn automatic(&mut self) -> bool {
        let what = self.config.automatic;
        if what == Automatic::None || self.config.source.is_none() { return false; }
        let unanswered = |result: Result<(), Error>| matches!(result, Err(Error::Network | Error::NoParser));
        let checked = self.check().map(drop);
        if checked.is_err() || what == Automatic::Check { return unanswered(checked); }
        let fetched = self.fetch().map(drop);
        if fetched.is_err() || what == Automatic::Fetch { return unanswered(fetched); }
        let tries = self.config.tries;
        let _ = self.apply(tries);
        false
    }
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let slot = match info.boot_slot.slot { BOOT_SLOT_A => b'A', BOOT_SLOT_B => b'B', _ => 0 };
    let mut updater = Updater { config: config(), slot, trial: info.boot_slot.trial != 0, manifest: info.boot_slot.manifest, running: 0, staged: 0, minimum: 0, held: None,
                                failure: String::new(), install: None, handles: 0, store: None, trusted: None, stamp_seen: 0,
                                order_seen: 0, order_refused: None };
    // The highest timestamp version taken before (351-UPD-0028): a lower one is refused.
    updater.stamp_seen = kept_stamp(&updater.config.channel);
    // The recovery orders applied before (351-UPD-0030): one not above them is refused.
    updater.order_seen = order_applied(&kept_orders(), &updater.config.channel);
    if updater.order_seen > 0 { mind::println!("[UPDATER] RECOVERY ORDER {} OF CHANNEL {} APPLIED BEFORE ({})", updater.order_seen, updater.config.channel, ORDERS); }
    if updater.stamp_seen > 0 { mind::println!("[UPDATER] TIMESTAMP VERSION {} OF CHANNEL {} FROM {}, TAKEN BEFORE", updater.stamp_seen, updater.config.channel, STAMP); }
    if keys::ROOT_TEST_KEYS {
        mind::println!("[UPDATER] KEYS: THE TEST ROOT KEYS");
    }
    // The trial is confirmed on the disk only once init says it confirmed the boot, asked every ASK_MS: while init holds
    // it for the Effector server's word, and when init cannot say, it stays a trial (801-UPD-0037).
    let (mut waiting, mut ask_at) = (false, None);
    if slot == 0 {
        mind::println!("[UPDATER] BOOTED FROM THE VOLUME'S ROOT: NO SLOT TO FILL");
        // The key file kept, read only when there is one: a boot from the root starts no parser for nothing.
        if [TRUSTED, TRUSTED_NEW].iter().any(|path| File::open(path).is_ok()) { updater.trust_kept(parser().endpoint()); }
    } else {
        // Both slots' channels through one parser: a session's helper may hold the updater's one endpoint for a moment
        // after it ends (109-KRN-0133).
        let parser = parser();
        updater.trust_kept(parser.endpoint());
        let kept = updater.kept(slot_name(slot), Some(updater.manifest), parser.endpoint());
        updater.slot_period(parser.endpoint());
        let (running, minimum) = kept.map_or((0, 0), |c| (c.version, c.minimum));
        updater.running = running; updater.minimum = minimum;
        let staged = updater.kept(updater.other(), None, parser.endpoint()).map_or(0, |c| c.version);
        drop(parser);
        updater.staged = if staged > running { staged } else { 0 };
        updater.say_other_formats();
        let recorded = plan::newest(updater.records());
        mind::println!("[UPDATER] SLOT {}{}, VERSION {}; THE NEWER RECORD: {}", slot_name(slot), if updater.trial { " ON TRIAL" } else { "" },
            if running == 0 { String::from("NOT KNOWN") } else { format!("{}", running) },
            recorded.map_or(String::from("NONE"), |(_, r)| format!("SLOT {} {}, FLOOR {}", r.slot as char, if r.confirmed { "CONFIRMED" } else { "NOT CONFIRMED" }, plan::floor(updater.records()))));
        if let Some(channel) = kept { updater.hold(channel, Held::Slot(slot)); }
        updater.raise_floor();
        // A transfer a restart cut is not resumed: the gateway begins it again.
        updater.drop_bundle("LEFT BY A RESTART");
        // 351-UPD-0033: releases kept in the block store, once the running slot is confirmed.
        match Store::open() {
            Ok(store) => { mind::println!("[UPDATER] STORE: RELEASES ARE KEPT IN THE BLOCK STORE"); updater.store = Some(store); }
            Err(why) => mind::println!("[UPDATER] STORE: {}: NO RELEASE IS KEPT THERE", why),
        }
        updater.keep_running();
        updater.trim();
        updater.refused_before();
        if updater.trial {
            mind::println!("[UPDATER] TRIAL BOOT: CONFIRMED ON THE DISK ONLY ONCE INIT CONFIRMS IT");
            (waiting, ask_at) = (true, Some(0));
        }
    }
    let (mut next_automatic, mut said) = (Some(0), false);
    let mut scratch: alloc::boxed::Box<[u8; api::REQUEST_MAX]> = alloc::vec![0u8; api::REQUEST_MAX].into_boxed_slice().try_into().unwrap();
    loop {
        let now = mind::time::uptime_ms();
        // init answers at once (init.wit 1.13): confirmed, refused, or deciding while it waits for the Effector server.
        if ask_at.is_some_and(|at| now >= at) {
            ask_at = None;
            match wire::with_timeout(ASK_MS as u32, || init::boot(Endpoint(SLOT_LIFECYCLE))) {
                Ok(Ok(boot)) if boot.trial && boot.confirmed => { waiting = false; updater.confirm("INIT CONFIRMED THE TRIAL BOOT"); }
                Ok(Ok(boot)) if boot.trial && boot.deciding => {
                    if !said { said = true; mind::println!("[UPDATER] INIT HOLDS THE TRIAL BOOT FOR THE EFFECTOR SERVER'S WORD: NOT CONFIRMED UNTIL THEN"); }
                    ask_at = Some(now + ASK_MS);
                }
                // Refused: the trial spent, its reason kept, and the machine restarted into the slot before.
                Ok(Ok(boot)) if boot.trial => { (waiting, next_automatic) = (false, None); updater.refused(&boot); }
                Ok(Ok(_)) => waiting = false,
                Err(mind::sys::Error::Other(ERR_TIMEOUT)) => ask_at = Some(now + ASK_MS),
                other => {
                    if !said { said = true; mind::println!("[UPDATER] INIT DOES NOT SAY HOW THE BOOT WENT ({:?}): NOT CONFIRMED, ASKED AGAIN", other.map(|r| r.map(|b| (b.trial, b.confirmed)))); }
                    ask_at = Some(now + ASK_MS * 10);
                }
            }
        }
        // Nothing automatic while the trial waits: the other slot is its fallback until then.
        if !waiting && next_automatic.is_some_and(|at| now >= at) {
            let again = updater.automatic().then_some(RETRY_MS);
            let every = (updater.config.every_ms > 0).then_some(updater.config.every_ms);
            next_automatic = [again, every].into_iter().flatten().min().map(|ms| mind::time::uptime_ms() + ms);
        }
        let wake = [ask_at, if waiting { None } else { next_automatic }].into_iter().flatten().min();
        let wait = wake.map_or(0, |at| at.saturating_sub(mind::time::uptime_ms()).clamp(1, u32::MAX as usize) as u32);
        // Requests are checked against idl/update.wit before they are served (MC-2.4).
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, wait) else { continue };
        // The Effector gateway's client (BADGE_UPDATE_GATEWAY, update.wit 1.2) stages or activates a release only while
        // init's remote-update counts, asked at each such request so that the owner's change holds at once (751-KRN-0185).
        let allowed = |what: &str| -> Result<(), Error> {
            if request.badge != mind::process::BADGE_UPDATE_GATEWAY { return Ok(()); }
            match wire::with_timeout(ASK_MS as u32, || init::mode(Endpoint(SLOT_LIFECYCLE))) {
                Ok(Ok(modes)) if modes.remote_update => Ok(()),
                other => {
                    let why = if matches!(other, Ok(Ok(_))) { "REMOTE-UPDATE IS OFF" } else { "INIT DID NOT SAY" };
                    mind::println!("[UPDATER] {} REFUSED TO THE EFFECTOR GATEWAY (PID {}): {} (MC-12.6)", what, request.sender, why);
                    Err(Error::Denied)
                }
            }
        };
        // Only the gateway's client hands over a bundle; its word is asked at begin and at commit.
        let gateway = |what: &str| -> Result<(), Error> {
            if request.badge == mind::process::BADGE_UPDATE_GATEWAY { return allowed(what); }
            mind::println!("[UPDATER] {} REFUSED (BADGE {}, PID {}): ONLY THE EFFECTOR GATEWAY HANDS OVER A BUNDLE (MC-12.6)", what, request.badge, request.sender);
            Err(Error::Denied)
        };
        let _ = match api::decode(&request, RECEIVED, &mut scratch) {
            Ok((api::Request::Check, call)) => { let offer = updater.check(); api::reply_check(call, offer.as_ref().map_err(|e| *e)) }
            Ok((api::Request::Fetch, call)) => api::reply_fetch(call, allowed("FETCH").and_then(|()| updater.fetch())),
            Ok((api::Request::Apply { tries }, call)) => api::reply_apply(call, allowed("APPLY").and_then(|()| updater.apply(tries))),
            Ok((api::Request::Rollback, call)) => api::reply_rollback(call, allowed("ROLLBACK").and_then(|()| updater.rollback())),
            Ok((api::Request::Status, call)) => api::reply_status(call, &updater.state()),
            Ok((api::Request::Metadata, call)) => api::reply_metadata(call, &updater.metadata()),
            Ok((api::Request::Kept, call)) => api::reply_kept(call, &updater.releases_kept()),
            Ok((api::Request::InstallBegin { size, sha256, command }, call)) =>
                api::reply_install_begin(call, gateway("INSTALL-BEGIN").and_then(|()| updater.install_begin(size, sha256.as_str(), command.as_str()))),
            Ok((api::Request::InstallWrite { handle, offset, bytes }, call)) => {
                let written = if request.badge == mind::process::BADGE_UPDATE_GATEWAY { updater.install_write(handle, offset, bytes) } else { Err(Error::Denied) };
                api::reply_install_write(call, written)
            }
            Ok((api::Request::InstallCommit { handle }, call)) =>
                api::reply_install_commit(call, gateway("INSTALL-COMMIT").and_then(|()| updater.install_commit(handle))),
            Err(reason) if request.is_call => wire::reject(reason),
            Err(_) => Ok(()),
        };
    }
}
