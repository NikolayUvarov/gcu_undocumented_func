#![no_std]
#![no_main]
// The updater (351-UPD-0007, idl/update.wit, docs/update/updater.md; MC-9.2–9.4, MC-3.11): turns a published release
// into a staged, verified slot and nothing more. It checks a channel's signature with the release key built into it, a
// manifest's with the boot key, and every boot file it stages by size and SHA-256; the parser service reads the
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
use mind::idl::update::{self as api, Error, Offer, State};
use mind::idl::codec::Text;
use mind::idl::{init, wire};
use mind::ipc::Endpoint;
use mind::parse::{read_channel, read_manifest, Refused};
use mind::release::{self, Channel, CHANNEL_MAX, MANIFEST_MAX};
use slots::{Record, RECORD};
use source::{sha256, Source};

#[cfg(target_arch = "x86_64")]
const ARCH: &str = "x86_64";
#[cfg(target_arch = "aarch64")]
const ARCH: &str = "aarch64";
const CONFIG: &str = "update.txt";
const RECORDS: [&str; 2] = ["MIND/BOOT0", "MIND/BOOT1"];
const PARSE: Endpoint = Endpoint(SLOT_PARSE);
const MARGIN_MS: usize = 5000; // past the kernel's deadline: had the trial not been confirmed, the machine would have restarted
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

/// A channel file whose fields the parser read and whose signature is the release key's.
fn channel_of(file: &[u8]) -> Result<Channel, Error> {
    let (channel, signed) = read_channel(PARSE, file).map_err(refused)?;
    if !verified(&keys::RELEASE_KEY, &file[..signed.body], &signed.signature) { return Err(Error::Signature); }
    Ok(channel)
}

/// The boot files of a manifest: the kernel and the services the bootloader loads, by name, size and SHA-256.
fn boot_files(text: &[u8]) -> Result<Vec<release::File>, Error> {
    let mut files = Vec::new();
    read_manifest(PARSE, text, |file| files.push(*file)).map_err(refused)?;
    files.retain(|f| f.path.as_str() == "kernel.elf" || BOOT_FILES.contains(&f.path.as_str()));
    // The bootloader loads neither a slot without these.
    if !["kernel.elf", "init.elf"].iter().all(|name| files.iter().any(|f| f.path.as_str() == *name)) { return Err(Error::Malformed); }
    Ok(files)
}

fn slot_name(slot: u8) -> &'static str { if slot == b'A' { "A" } else { "B" } }

struct Updater {
    config: Config,
    /// The running slot (`A`, `B`, or 0 booted from the root), whether it booted on trial, its manifest's digest.
    slot: u8, trial: bool, manifest: [u8; 32],
    /// The running version (0: not known), the version staged in the other slot (0: none), the minimum last seen.
    running: u64, staged: u64, minimum: u64,
    failure: String,
}

impl Updater {
    fn records(&self) -> [Option<Record>; 2] {
        RECORDS.map(|path| {
            let file = File::open(path).ok()?;
            let mut data = [0u8; RECORD];
            (file.size() == RECORD && file.read_at(0, &mut data) == Ok(RECORD)).then(|| Record::parse(&data)).flatten()
        })
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
        mind::println!("[UPDATER] RECORD {} SEQUENCE {}: SLOT {}, FALLBACK {}, {} TRIES, {}", RECORDS[file], record.sequence, record.slot as char,
            if record.fallback == 0 { '-' } else { record.fallback as char }, record.tries, if record.confirmed { "CONFIRMED" } else { "NOT CONFIRMED" });
        Ok(())
    }

    fn other(&self) -> &'static str { slot_name(plan::other(self.slot)) }

    /// The version a slot holds as the channel kept there shows it: signed with the release key and naming, for this
    /// architecture, the manifest `digest` (None: read the slot's MANIFEST). 0 when it shows none.
    fn kept_version(&self, slot: &str, digest: Option<[u8; 32]>) -> (u64, u64) {
        let Ok(file) = File::open(&format!("MIND/{}/CHANNEL", slot)) else { return (0, 0) };
        let mut bytes = alloc::vec![0u8; file.size().min(CHANNEL_MAX)];
        if file.read_at(0, &mut bytes) != Ok(bytes.len()) { return (0, 0); }
        let Ok(channel) = channel_of(&bytes) else { return (0, 0) };
        let digest = digest.or_else(|| {
            let manifest = File::open(&format!("MIND/{}/MANIFEST", slot)).ok()?;
            (manifest.size() <= MANIFEST_MAX).then(|| sha256(&manifest)).flatten()
        });
        if digest.is_some() && channel.manifest(ARCH) == digest { (channel.version, channel.minimum) } else { (0, 0) }
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
    fn read_channel(&mut self) -> Result<(Channel, Vec<u8>), Error> {
        let Some(source) = &self.config.source else { return Err(Error::NoSource) };
        let bytes = source.small(&format!("channels/{}", self.config.channel), CHANNEL_MAX)?;
        let channel = channel_of(&bytes)?;
        if channel.name.as_str() != self.config.channel { return Err(Error::Malformed); }
        // A clock that cannot be read cannot show the channel current (MC-9.4: expiry handled by a set policy).
        let now = mind::rtc::unix_time().ok_or(Error::Expired)?;
        if release::expires_unix(&channel.expires) <= now { return Err(Error::Expired); }
        self.minimum = self.minimum.max(channel.minimum);
        Ok((channel, bytes))
    }

    fn check(&mut self) -> Result<Offer, Error> {
        let (channel, _) = match self.read_channel() { Ok(read) => read, Err(error) => return self.fail("check", error) };
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
        let (channel, channel_file) = self.read_channel()?;
        if channel.version <= self.running { return Err(Error::Older); }
        let digest = channel.manifest(ARCH).ok_or(Error::Missing)?;
        let Some(source) = &self.config.source else { return Err(Error::NoSource) };
        let release = format!("releases/{}/{}", channel.version, ARCH);
        let manifest = source.small(&format!("{}/MANIFEST", release), MANIFEST_MAX)?;
        if mind::sha256::digest(&manifest) != digest { return Err(Error::Digest); }
        let signature = source.small(&format!("{}/MANIFEST.SIG", release), 64)?;
        if !verified(&keys::BOOT_KEY, &manifest, &signature) { return Err(Error::Signature); }
        let files = boot_files(&manifest)?;
        let slot = self.other();
        mind::println!("[UPDATER] FETCH: VERSION {} INTO SLOT {}: {} BOOT FILES, MANIFEST {}", channel.version, slot, files.len(), short(&digest));
        // The channel goes first and comes back last: a slot holds one only once all of it is there. A boot file the
        // manifest does not list goes too (the bootloader refuses a slot with one); a listed one is resumed.
        let dir = Dir::create(&format!("MIND/{}", slot)).map_err(|_| Error::Write)?;
        self.staged = 0;
        let mut stale = Vec::new();
        dir.list(|entry| if !entry.is_dir { stale.push(String::from(entry.name_str())); }).map_err(|_| Error::Write)?;
        let listed = |name: &str| files.iter().any(|f| f.path.as_str().eq_ignore_ascii_case(name));
        for name in stale.iter().filter(|name| ["CHANNEL", "MANIFEST", "MANIFEST.SIG"].iter().any(|kept| kept.eq_ignore_ascii_case(name)) || !listed(name)) {
            dir.remove(name).map_err(|_| Error::Write)?;
        }
        let mut fetched = 0;
        for f in &files {
            let path = format!("MIND/{}/{}", slot, f.path.as_str());
            let mut file = File::open_mode(&path, MODE_WRITE | MODE_CREATE).map_err(|_| Error::Write)?;
            let blob = format!("blobs/{}", f.digest.iter().map(|b| format!("{:02x}", b)).collect::<String>());
            let resumed = file.size() > 0;
            fetched += source.file(&blob, &mut file, f.size)?;
            let whole = |file: &File| file.size() as u64 == f.size && sha256(file) == Some(f.digest);
            // What was there before may be another file's start: fetched again whole, once.
            if !whole(&file) && resumed {
                file.truncate(0).map_err(|_| Error::Write)?;
                fetched += source.file(&blob, &mut file, f.size)?;
            }
            if !whole(&file) {
                // Not the file: the next fetch starts it again.
                let _ = file.truncate(0);
                mind::println!("[UPDATER] FETCH: {} IS NOT AS THE MANIFEST LISTS IT", f.path.as_str());
                return Err(Error::Digest);
            }
            let _ = file.flush();
        }
        for (name, data) in [("MANIFEST", &manifest), ("MANIFEST.SIG", &signature), ("CHANNEL", &channel_file)] {
            let mut file = File::create(&format!("MIND/{}/{}", slot, name)).map_err(|_| Error::Write)?;
            if file.write_at(0, data) != Ok(data.len()) || file.flush().is_err() { return Err(Error::Write); }
        }
        self.staged = channel.version;
        mind::println!("[UPDATER] FETCH: VERSION {} STAGED IN SLOT {}: {} FILES, {} BYTES FETCHED", channel.version, slot, files.len(), fetched);
        Ok(channel.version)
    }

    /// The other slot checked as the bootloader will check it, and the version its kept channel shows.
    fn verify_other(&self) -> Result<(u64, u64), Error> {
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
        let files = boot_files(&manifest)?;
        for name in BOOT_FILES.iter().chain(["kernel.elf"].iter()) {
            let path = format!("MIND/{}/{}", slot, name);
            match (files.iter().find(|f| f.path.as_str() == *name), File::open(&path)) {
                (Some(f), Ok(file)) if file.size() as u64 == f.size && sha256(&file) == Some(f.digest) => {}
                // A boot file the manifest does not list must not be there: the bootloader refuses the slot.
                (None, Err(_)) => {}
                _ => { mind::println!("[UPDATER] SLOT {}: {} IS NOT AS THE MANIFEST LISTS IT", slot, name); return Err(Error::Digest); }
            }
        }
        match self.kept_version(slot, Some(mind::sha256::digest(&manifest))) { (0, _) => Err(Error::NotStaged), shown => Ok(shown) }
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
            let (version, _) = self.verify_other()?;
            if version <= self.running { return Err(Error::Older); }
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
            let (version, _) = self.verify_other().map_err(|_| Error::NoFallback)?;
            if version < self.minimum { return Err(Error::NoFallback); }
            let write = plan::trial_of_other(self.records(), self.slot, 1).ok_or(Error::Write)?;
            mind::println!("[UPDATER] ROLLBACK: VERSION {} IN SLOT {} ON TRIAL, FALLING BACK TO {}", version, self.other(), slot_name(self.slot));
            self.write_record(write)?;
            self.restart()
        })();
        result.or_else(|error| self.fail("rollback", error))
    }

    /// On a trial boot the kernel restarts the machine at its deadline unless init confirmed the boot: past it, the
    /// boot was confirmed, and the record says so on the disk.
    fn confirm(&mut self) {
        match plan::confirm(self.records(), self.slot) {
            Some(write) => match self.write_record(write) {
                Ok(()) => mind::println!("[UPDATER] SLOT {} CONFIRMED ON THE DISK: THE TRIAL OUTLIVED THE KERNEL'S DEADLINE", slot_name(self.slot)),
                Err(error) => { let _ = self.fail::<()>("confirm", error); }
            },
            None => mind::println!("[UPDATER] SLOT {}: THE RECORD NEEDS NO CONFIRMATION", slot_name(self.slot)),
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
    let mut updater = Updater { config: config(), slot, trial: info.boot_slot.trial != 0, manifest: info.boot_slot.manifest, running: 0, staged: 0, minimum: 0, failure: String::new() };
    if keys::RELEASE_TEST_KEY || keys::BOOT_TEST_KEY {
        mind::println!("[UPDATER] KEYS: {}{}", if keys::RELEASE_TEST_KEY { "THE TEST RELEASE KEY " } else { "" }, if keys::BOOT_TEST_KEY { "THE TEST BOOT KEY" } else { "" });
    }
    // The trial is confirmed past the kernel's deadline (351-KRN-0014), measured on the same clock.
    let mut confirm_at = None;
    if slot == 0 {
        mind::println!("[UPDATER] BOOTED FROM THE VOLUME'S ROOT: NO SLOT TO FILL");
    } else {
        let (running, minimum) = updater.kept_version(slot_name(slot), Some(updater.manifest));
        updater.running = running; updater.minimum = minimum;
        let (staged, _) = updater.kept_version(updater.other(), None);
        updater.staged = if staged > running { staged } else { 0 };
        let recorded = plan::newest(updater.records());
        mind::println!("[UPDATER] SLOT {}{}, VERSION {}; THE NEWER RECORD: {}", slot_name(slot), if updater.trial { " ON TRIAL" } else { "" },
            if running == 0 { String::from("NOT KNOWN") } else { format!("{}", running) },
            recorded.map_or(String::from("NONE"), |(_, r)| format!("SLOT {} {}", r.slot as char, if r.confirmed { "CONFIRMED" } else { "NOT CONFIRMED" })));
        if updater.trial {
            let seconds = if info.boot_slot.deadline_s == 0 { TRIAL_DEADLINE_S } else { info.boot_slot.deadline_s } as usize;
            let at = seconds * 1000 + MARGIN_MS;
            mind::println!("[UPDATER] TRIAL BOOT: CONFIRMED ON THE DISK AT {} S UNLESS THE KERNEL RESTARTS THE MACHINE FIRST", at / 1000);
            confirm_at = Some(at);
        }
    }
    let mut next_automatic = Some(0);
    loop {
        let now = mind::time::uptime_ms();
        if confirm_at.is_some_and(|at| now >= at) { confirm_at = None; updater.confirm(); }
        // Nothing automatic while the trial waits: the other slot is its fallback until then.
        if confirm_at.is_none() && next_automatic.is_some_and(|at| now >= at) {
            let again = updater.automatic().then_some(RETRY_MS);
            let every = (updater.config.every_ms > 0).then_some(updater.config.every_ms);
            next_automatic = [again, every].into_iter().flatten().min().map(|ms| mind::time::uptime_ms() + ms);
        }
        let wake = [confirm_at, if confirm_at.is_none() { next_automatic } else { None }].into_iter().flatten().min();
        let wait = wake.map_or(0, |at| at.saturating_sub(mind::time::uptime_ms()).clamp(1, u32::MAX as usize) as u32);
        // Requests are checked against idl/update.wit before they are served (MC-2.4).
        let Ok(request) = Endpoint::SERVICE.recv_timeout(0, wait) else { continue };
        let _ = match api::decode(&request, 0) {
            Ok((api::Request::Check, call)) => { let offer = updater.check(); api::reply_check(call, offer.as_ref().map_err(|e| *e)) }
            Ok((api::Request::Fetch, call)) => api::reply_fetch(call, updater.fetch()),
            Ok((api::Request::Apply { tries }, call)) => api::reply_apply(call, updater.apply(tries)),
            Ok((api::Request::Rollback, call)) => api::reply_rollback(call, updater.rollback()),
            Ok((api::Request::Status, call)) => api::reply_status(call, &updater.state()),
            Err(reason) if request.is_call => wire::reject(reason),
            Err(_) => Ok(()),
        };
    }
}
