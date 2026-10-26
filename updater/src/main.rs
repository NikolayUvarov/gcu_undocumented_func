#![no_std]
#![no_main]
// The updater (351-UPD-0007, idl/update.wit, docs/update/updater.md; MC-9.2–9.4, MC-3.11): turns a published release
// into a staged, verified slot and nothing more. It checks a channel's signature with the release key built into it, a
// manifest's with the boot key, and every file it stages by size and SHA-256; the parser service reads the
// metadata (MC-11.11). It writes only the slot that did not boot and the boot records (vfs_server's update zone), and
// confirms a trial boot on the disk once the kernel's deadline has passed without a restart. Holds: its endpoint, the
// clock, the boot disk's client badged for the update zone, the parser, init's client badged for a restart, the log,
// the flow grant the policy gives it and the TLS client.
extern crate alloc;

#[allow(dead_code)]
#[path = "../../bootloader/src/slots.rs"]
mod slots;
mod plan;
mod source;
mod keys { include!(concat!(env!("OUT_DIR"), "/keys.rs")); }

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use ed25519_dalek::{Signature, VerifyingKey};
use mind::abi::*;
use mind::fs::{Dir, File, MODE_CREATE, MODE_WRITE};
use mind::idl::update::{self as api, Error, Metadata, Offer, State};
use mind::idl::codec::Text;
use mind::idl::{init, wire};
use mind::ipc::Endpoint;
use mind::parse::{read_channel, read_manifest, Refused, Session};
use mind::release::{self, Channel, CHANNEL_MAX, MANIFEST_MAX};
use slots::{Record, RECORD};
use plan::Kind;
use source::{copy, sha256, Source};

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
const PARSE: Endpoint = Endpoint(SLOT_PARSE);
const MARGIN_MS: usize = 5000; // past the kernel's deadline: had the trial not been confirmed, the machine would have restarted
const ASK_MS: usize = 1000; // while a trial waits, init is asked this often how the boot went (init.wit 1.4, 351-UPD-0021)
const TRIES_MAX: u8 = 9;
const RETRY_MS: usize = 30_000; // an automatic run the source did not answer is tried again after this

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

fn refused(why: Refused) -> Error { match why { Refused::Service => Error::NoParser, Refused::Malformed | Refused::Lied => Error::Malformed } }

/// A parser of the check's own, which the loader starts as a helper holding nothing but its endpoint, and which ends
/// when the check does (109-NET-0010). Only with a loader client in SLOT_LOADER (`init`'s, 109-KRN-0117); the shared
/// service without one, or where the loader starts none (logged with the reason).
fn parser() -> Session {
    if mind::dev::cap_info(SLOT_LOADER).0 != CAP_KIND_ENDPOINT { return Session::shared(PARSE); }
    Session::own(Endpoint::LOADER).unwrap_or_else(|why| { mind::println!("[UPDATER] PARSER: THE SHARED SERVICE ({:?})", why); Session::shared(PARSE) })
}

/// A channel file whose fields `parser` read and whose signature is the release key's.
fn channel_of(file: &[u8], parser: Endpoint) -> Result<Channel, Error> {
    let (channel, signed) = read_channel(parser, file).map_err(refused)?;
    if !verified(&keys::RELEASE_KEY, &file[..signed.body], &signed.signature) { return Err(Error::Signature); }
    Ok(channel)
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
}

/// Where the channel the updater holds came from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Held { Source, Slot(u8) }

/// Whether a channel's expiry has passed, or the clock cannot be read (MC-9.4).
fn expired(channel: &Channel) -> bool { plan::expired(release::expires_unix(&channel.expires), mind::rtc::unix_time()) }

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
        mind::println!("[UPDATER] RECORD {} SEQUENCE {}: SLOT {}, FALLBACK {}, {} TRIES, {}, FLOOR {}", RECORDS[file], record.sequence, record.slot as char,
            if record.fallback == 0 { '-' } else { record.fallback as char }, record.tries, if record.confirmed { "CONFIRMED" } else { "NOT CONFIRMED" }, record.floor);
        Ok(())
    }

    fn other(&self) -> &'static str { slot_name(plan::other(self.slot)) }

    /// The channel kept in a slot, if it is signed with the release key and names, for this architecture, the manifest
    /// `digest` (None: read the slot's MANIFEST); its version is the slot's. The caller's parser reads it, so that one
    /// session serves several reads (109-KRN-0133).
    fn kept(&self, slot: &str, digest: Option<[u8; 32]>, parser: Endpoint) -> Option<Channel> {
        let file = File::open(&format!("MIND/{}/CHANNEL", slot)).ok()?;
        let mut bytes = alloc::vec![0u8; file.size().min(CHANNEL_MAX)];
        if file.read_at(0, &mut bytes) != Ok(bytes.len()) { return None; }
        let channel = channel_of(&bytes, parser).ok()?;
        let digest = digest.or_else(|| {
            let manifest = File::open(&format!("MIND/{}/MANIFEST", slot)).ok()?;
            (manifest.size() <= MANIFEST_MAX).then(|| sha256(&manifest)).flatten()
        });
        (digest.is_some() && channel.manifest(ARCH) == digest).then_some(channel)
    }

    /// Holds `channel`, and says so in the log when it is another one or came from elsewhere.
    fn hold(&mut self, channel: Channel, from: Held) {
        if self.held == Some((channel, from)) { return; }
        self.held = Some((channel, from));
        let from = match from { Held::Source => String::from("FROM THE SOURCE"), Held::Slot(slot) => format!("KEPT IN SLOT {}", slot as char) };
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
                   checked: from == Held::Source }
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

    /// The channel, its signature, form and expiry, and its version against the running one; with the file as read.
    fn read_channel(&mut self, parser: &mut Session) -> Result<(Channel, Vec<u8>), Error> {
        let Some(source) = &self.config.source else { return Err(Error::NoSource) };
        let bytes = source.small(&format!("channels/{}", self.config.channel), CHANNEL_MAX, parser)?;
        let channel = channel_of(&bytes, parser.endpoint())?;
        if channel.name.as_str() != self.config.channel { return Err(Error::Malformed); }
        self.hold(channel, Held::Source);
        // A clock that cannot be read cannot show the channel current (MC-9.4: expiry handled by a set policy).
        if expired(&channel) { return Err(Error::Expired); }
        self.minimum = self.minimum.max(channel.minimum);
        self.raise_floor();
        Ok((channel, bytes))
    }

    fn check(&mut self) -> Result<Offer, Error> {
        let mut parser = parser();
        let (channel, _) = match self.read_channel(&mut parser) { Ok(read) => read, Err(error) => return self.fail("check", error) };
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
        match self.stage() { Ok(version) => { self.failure.clear(); Ok(version) } Err(error) => self.fail("fetch", error) }
    }

    fn stage(&mut self) -> Result<u64, Error> {
        self.may_stage()?;
        let mut parser = parser();
        let (channel, channel_file) = self.read_channel(&mut parser)?;
        if channel.version <= self.running { return Err(Error::Older); }
        let digest = channel.manifest(ARCH).ok_or(Error::Missing)?;
        let Some(source) = &self.config.source else { return Err(Error::NoSource) };
        let release = format!("releases/{}/{}", channel.version, ARCH);
        let manifest = source.small(&format!("{}/MANIFEST", release), MANIFEST_MAX, &mut parser)?;
        if mind::sha256::digest(&manifest) != digest { return Err(Error::Digest); }
        let signature = source.small(&format!("{}/MANIFEST.SIG", release), 64, &mut parser)?;
        if !verified(&keys::BOOT_KEY, &manifest, &signature) { return Err(Error::Signature); }
        let files = slot_files(&manifest, parser.endpoint())?;
        let slot = self.other();
        let applications = files.iter().filter(|f| kind(f.path.as_str()) == Kind::Application).count();
        mind::println!("[UPDATER] FETCH: VERSION {} INTO SLOT {}: {} BOOT FILES AND {} APPLICATIONS, MANIFEST {}", channel.version, slot,
                       files.len() - applications, applications, short(&digest));
        // The channel goes first and comes back last: a slot holds one only once all of it is there. A boot file the
        // manifest does not list goes too (the bootloader refuses a slot with one); a listed one is resumed.
        let dir = Dir::create(&format!("MIND/{}", slot)).map_err(|_| Error::Write)?;
        self.staged = 0;
        let mut stale = Vec::new();
        dir.list(|entry| if !entry.is_dir { stale.push(String::from(entry.name_str())); }).map_err(|_| Error::Write)?;
        let listed = |name: &str| files.iter().any(|f| plan::in_slot(f.path.as_str()).eq_ignore_ascii_case(name));
        for name in stale.iter().filter(|name| ["CHANNEL", "MANIFEST", "MANIFEST.SIG"].iter().any(|kept| kept.eq_ignore_ascii_case(name)) || !listed(name)) {
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
                        mind::println!("[UPDATER] FETCH: {} OF {}: {}, {} BYTES COPIED FROM {} IN {} MS", index + 1, files.len(), f.path.as_str(), f.size, from,
                                       mind::time::uptime_ms() - started);
                        continue;
                    }
                    file.truncate(0).map_err(|_| Error::Write)?;
                }
            }
            let blob = format!("blobs/{}", f.digest.iter().map(|b| format!("{:02x}", b)).collect::<String>());
            let resumed = file.size() > 0;
            fetched += source.file(&blob, &mut file, f.size, &mut parser)?;
            // What was there before may be another file's start: fetched again whole, once.
            if !whole(&file) && resumed {
                file.truncate(0).map_err(|_| Error::Write)?;
                fetched += source.file(&blob, &mut file, f.size, &mut parser)?;
            }
            if !whole(&file) {
                // Not the file: the next fetch starts it again.
                let _ = file.truncate(0);
                mind::println!("[UPDATER] FETCH: {} IS NOT AS THE MANIFEST LISTS IT", f.path.as_str());
                return Err(Error::Digest);
            }
            let _ = file.flush();
            // Each file in the log, so that a slow or stalled fetch shows how far it got (351-UPD-0018).
            mind::println!("[UPDATER] FETCH: {} OF {}: {}, {} BYTES IN {} MS", index + 1, files.len(), f.path.as_str(), f.size, mind::time::uptime_ms() - started);
        }
        for (name, data) in [("MANIFEST", &manifest), ("MANIFEST.SIG", &signature), ("CHANNEL", &channel_file)] {
            let mut file = File::create(&format!("MIND/{}/{}", slot, name)).map_err(|_| Error::Write)?;
            if file.write_at(0, data) != Ok(data.len()) || file.flush().is_err() { return Err(Error::Write); }
        }
        self.staged = channel.version;
        mind::println!("[UPDATER] FETCH: VERSION {} STAGED IN SLOT {}: {} FILES, {} BYTES FETCHED", channel.version, slot, files.len(), fetched);
        Ok(channel.version)
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
        if !verified(&keys::BOOT_KEY, &manifest, &signature) { return Err(Error::Signature); }
        // The metadata first: a session's own parser ends after 30 s without a request, less than hashing a slot takes
        // on a slow machine (parse's IDLE_MS).
        let parser = parser();
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
            let channel = self.verify_other()?;
            let version = channel.version;
            if version <= self.running { return Err(Error::Older); }
            // Past its channel's expiry a staged release waits for a current one (351-UPD-0025); a rollback does not.
            if expired(&channel) { return Err(Error::Expired); }
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
            let version = self.verify_other().map_err(|_| Error::NoFallback)?.version;
            if version < self.minimum { return Err(Error::NoFallback); }
            let write = plan::trial_of_other(self.records(), self.slot, 1).ok_or(Error::Write)?;
            mind::println!("[UPDATER] ROLLBACK: VERSION {} IN SLOT {} ON TRIAL, FALLING BACK TO {}", version, self.other(), slot_name(self.slot));
            self.write_record(write)?;
            self.restart()
        })();
        result.or_else(|error| self.fail("rollback", error))
    }

    /// On a trial boot the kernel restarts the machine at its deadline unless init confirmed the boot: once init says it
    /// did, or past that deadline, the record says so on the disk; `why` says which.
    fn confirm(&mut self, why: &str) {
        match plan::confirm(self.records(), self.slot) {
            Some(write) => match self.write_record(write) {
                Ok(()) => { mind::println!("[UPDATER] SLOT {} CONFIRMED ON THE DISK: {}", slot_name(self.slot), why); self.raise_floor(); }
                Err(error) => { let _ = self.fail::<()>("confirm", error); }
            },
            None => mind::println!("[UPDATER] SLOT {}: THE RECORD NEEDS NO CONFIRMATION", slot_name(self.slot)),
        }
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
    let mut updater = Updater { config: config(), slot, trial: info.boot_slot.trial != 0, manifest: info.boot_slot.manifest, running: 0, staged: 0, minimum: 0, held: None, failure: String::new() };
    if keys::RELEASE_TEST_KEY || keys::BOOT_TEST_KEY {
        mind::println!("[UPDATER] KEYS: {}{}", if keys::RELEASE_TEST_KEY { "THE TEST RELEASE KEY " } else { "" }, if keys::BOOT_TEST_KEY { "THE TEST BOOT KEY" } else { "" });
    }
    // The trial is confirmed once init says it confirmed the boot (asked every ASK_MS), or past the kernel's deadline
    // (351-KRN-0014), measured on the same clock, when init cannot say.
    let (mut confirm_at, mut ask_at) = (None, None);
    if slot == 0 {
        mind::println!("[UPDATER] BOOTED FROM THE VOLUME'S ROOT: NO SLOT TO FILL");
    } else {
        // Both slots' channels through one parser: a session's helper may hold the updater's one endpoint for a moment
        // after it ends (109-KRN-0133).
        let parser = parser();
        let kept = updater.kept(slot_name(slot), Some(updater.manifest), parser.endpoint());
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
        if updater.trial {
            let seconds = if info.boot_slot.deadline_s == 0 { TRIAL_DEADLINE_S } else { info.boot_slot.deadline_s } as usize;
            let at = seconds * 1000 + MARGIN_MS;
            mind::println!("[UPDATER] TRIAL BOOT: CONFIRMED ON THE DISK ONCE INIT CONFIRMS IT, OR AT {} S UNLESS THE KERNEL RESTARTS THE MACHINE FIRST", at / 1000);
            (confirm_at, ask_at) = (Some(at), Some(0));
        }
    }
    let mut next_automatic = Some(0);
    loop {
        let now = mind::time::uptime_ms();
        // init answers once it has confirmed or refused the boot (init.wit 1.4); until then the call times out.
        if ask_at.is_some_and(|at| now >= at) {
            ask_at = None;
            match wire::with_timeout(ASK_MS as u32, || init::boot(Endpoint(SLOT_LIFECYCLE))) {
                Ok(Ok(boot)) if boot.trial && boot.confirmed => { confirm_at = None; updater.confirm("INIT CONFIRMED THE TRIAL BOOT"); }
                // Refused: the kernel restarts the machine at its deadline, into the other slot, which stays untouched.
                Ok(Ok(boot)) if boot.trial => {
                    (confirm_at, next_automatic) = (None, None);
                    mind::println!("[UPDATER] INIT REFUSED THE TRIAL BOOT: NOT CONFIRMED; THE KERNEL RESTARTS THE MACHINE AT ITS DEADLINE");
                }
                Err(mind::sys::Error::Other(ERR_TIMEOUT)) => ask_at = Some(now + ASK_MS),
                other => mind::println!("[UPDATER] INIT DOES NOT SAY HOW THE BOOT WENT ({:?}): CONFIRMED PAST THE KERNEL'S DEADLINE", other.map(|r| r.map(|b| (b.trial, b.confirmed)))),
            }
        }
        if confirm_at.is_some_and(|at| now >= at) { confirm_at = None; updater.confirm("THE TRIAL OUTLIVED THE KERNEL'S DEADLINE"); }
        // Nothing automatic while the trial waits: the other slot is its fallback until then.
        if confirm_at.is_none() && next_automatic.is_some_and(|at| now >= at) {
            let again = updater.automatic().then_some(RETRY_MS);
            let every = (updater.config.every_ms > 0).then_some(updater.config.every_ms);
            next_automatic = [again, every].into_iter().flatten().min().map(|ms| mind::time::uptime_ms() + ms);
        }
        let wake = [ask_at, confirm_at, if confirm_at.is_none() { next_automatic } else { None }].into_iter().flatten().min();
        let wait = wake.map_or(0, |at| at.saturating_sub(mind::time::uptime_ms()).clamp(1, u32::MAX as usize) as u32);
        // Requests are checked against idl/update.wit before they are served (MC-2.4).
        let Ok(request) = Endpoint::SERVICE.recv_timeout(0, wait) else { continue };
        let _ = match api::decode(&request, 0) {
            Ok((api::Request::Check, call)) => { let offer = updater.check(); api::reply_check(call, offer.as_ref().map_err(|e| *e)) }
            Ok((api::Request::Fetch, call)) => api::reply_fetch(call, updater.fetch()),
            Ok((api::Request::Apply { tries }, call)) => api::reply_apply(call, updater.apply(tries)),
            Ok((api::Request::Rollback, call)) => api::reply_rollback(call, updater.rollback()),
            Ok((api::Request::Status, call)) => api::reply_status(call, &updater.state()),
            Ok((api::Request::Metadata, call)) => api::reply_metadata(call, &updater.metadata()),
            Err(reason) if request.is_call => wire::reject(reason),
            Err(_) => Ok(()),
        };
    }
}
