#![no_std]
#![no_main]
// Key service (issue 103, MC-11.9, Appendix B.6): the device key lives only here. It is an Ed25519 key made from
// RDRAND once (no RDRAND: no key, every request answers no-key) and kept across boots in this service's private
// directory of the boot disk, sealed by the TPM where there is one (351-NET-0006); else unencrypted (351-NET-0005)
// until the owner sets a passphrase, and from then on sealed under it (1.6, lock.rs, 308-NET-0055): the service then
// starts without the key (`key-state` waiting) and holds it, with all that opens under it, once `unlock` opens it.
// Never exported. Serves
// idl/keystore.wit: the certificate and the public key to anyone; signatures only to the signer's badge (the TLS
// service) for TLS, a client's and (1.7) a server's, to the SSH service's badge for SSH logins (1.2) and to the SSH
// server's for its key exchanges (1.8), only for data
// of the form the purpose names, and within each one's budget (per boot; a TLS server's, a minute); the Wi-Fi station's remembered networks (1.1, wifi.rs) and its country (1.3) only to the station's badge;
// each program's secrets (1.4, secrets.rs) only to the client the loader got for that program, and the block store's own
// (1.5) only to the client init badged for it; `key-state` to anyone, `unlock` and `passphrase-set` only to the
// passphrase's badge (1.6). Holds: an RTC client (slot 2) for the certificate's start
// date, a VFS client with its own badge (slot 3) for `system/keystore`, the TPM service's client with the seal badge
// (slot 4). On a boot on trial (801-NET-0079) it writes, moves and removes nothing the slot before reads: a key that
// does not open is not replaced, and what would convert one waits for a boot not on trial; the Wi-Fi networks remembered
// then go to a file of the trial's own (wifi.rs).
extern crate alloc;

mod certificate;
mod form;
mod lock;
mod secret_file;
mod secrets;
mod stored;
mod wifi;

use alloc::vec::Vec;
use ed25519_dalek::{Signer, SigningKey};
use mind::abi::BootInfo;
use mind::idl::keystore::{self, Error, KeyState, Purpose, Usage};
use mind::idl::wire;
use mind::fs::File;
use mind::ipc::Endpoint;
use mind::idl::codec::List;
use mind::network::{BADGE_KEY_SIGNER, BADGE_KEY_SSH, BADGE_KEY_SSH_HOST, BADGE_KEY_STATION};

const RECEIVED: usize = 9;
const BUDGET: u32 = 4096; // TLS signatures per boot
const SSH_BUDGET: u32 = 256; // SSH logins per boot (1.2)
const SERVER_BUDGET: u32 = 120; // TLS server signatures a minute (1.7)
const HOST_BUDGET: u32 = 60; // SSH host signatures a minute (1.8): one each key exchange
const MINUTE_MS: usize = 60_000;

struct Keys {
    key: Option<SigningKey>, certificate: Vec<u8>, usage: Usage, ssh_logins: u32, wifi: Option<wifi::Wifi>, secrets: secrets::Clients,
    // The TLS server and SSH host signatures of the minute that started at `minute` (1.7, 1.8).
    served: u32, hosted: u32, minute: usize,
    // The device key sealed under a passphrase (1.6); whether a TPM seals it instead; the certificate's serial.
    lock: lock::Lock, tpm: bool, serial: [u8; 16],
    // Whether the boot is on trial, and whether the key held is the one the disk keeps, not one for this boot (801-NET-0079).
    trial: bool, kept: bool, release: [u8; 32],
}

// A key held: the one the disk keeps, or one for this boot only, which nothing is written under (801-NET-0079).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Held { Kept, ThisBoot }

// How long a TPM that does not answer at the start is waited for while a key it sealed is there.
const TPM_WAIT_MS: usize = 5000;
const TPM_STEP_MS: usize = 250;

impl Keys {
    fn new(trial: bool, release: &[u8; 32]) -> Self {
        if trial { mind::println!("[KEYSTORE] THE BOOT IS ON TRIAL: NOTHING IN {} THE SLOT BEFORE READS IS WRITTEN, MOVED OR REMOVED UNTIL A BOOT NOT ON TRIAL", stored::DIR); }
        let mut keys = Self { key: None, certificate: Vec::new(), usage: Usage { signatures: 0, budget: BUDGET, refused: 0 }, ssh_logins: 0, wifi: None, served: 0, hosted: 0, minute: mind::time::uptime_ms(),
                              secrets: secrets::Clients::new(), lock: lock::Lock::new(), tpm: tpm(), serial: [0; 16], trial, kept: false, release: *release };
        // Without the random source no key is made or used: the service fails closed, as the TLS service does.
        if !random(&mut keys.serial) { return keys; }
        let mut seed = [0u8; 32];
        if let Some(held) = Self::seed(&mut seed, keys.tpm, &mut keys.lock, trial) { keys.hold(&seed, held, release); }
        // The seed is the private key: wipe the stack copy (SigningKey wipes its own on drop).
        stored::wipe(&mut seed);
        keys
    }

    // Holds the key of `seed`: its certificate, its public key in the log, and the remembered Wi-Fi networks, which open
    // under it (so there are none without it).
    fn hold(&mut self, seed: &[u8; 32], held: Held, release: &[u8; 32]) {
        let key = SigningKey::from_bytes(seed);
        let start = mind::rtc::unix_time().map(|t| t.saturating_sub(86400)).unwrap_or(0);
        let der = certificate::build(&key, self.serial, start);
        let name = certificate::common_name(&key);
        let name = core::str::from_utf8(name.as_bytes()).unwrap_or("");
        mind::println!("[KEYSTORE] DEVICE KEY READY: {} (ED25519, CERTIFICATE {} BYTES)", name, der.len());
        mind::println!("[KEYSTORE] PUBLIC KEY {} {}", certificate::openssh(&key).as_str(), name.replace(' ', "-"));
        self.kept = held == Held::Kept;
        let writes = match (held, self.trial) { (Held::ThisBoot, _) => wifi::Writes::None, (Held::Kept, true) => wifi::Writes::Trial, (Held::Kept, false) => wifi::Writes::Kept };
        self.wifi = Some(wifi::Wifi::load(seed, writes, release));
        (self.key, self.certificate) = (Some(key), der);
    }

    // Whether the key is held, waits for its passphrase, or is none (1.6): answered at once.
    fn state(&self) -> KeyState {
        if self.key.is_some() { KeyState::Held } else if self.lock.file.is_some() { KeyState::Waiting } else { KeyState::None }
    }

    // Opens the key sealed under the passphrase (1.6), then removes the unencrypted copy a cut may have left beside it.
    fn unlock(&mut self, badge: u16, pid: u64, passphrase: &[u8]) -> Result<(), Error> {
        lock::client(badge, pid, "UNLOCK")?;
        if self.state() != KeyState::Waiting {
            let why = if self.tpm { "THE TPM SEALS THE DEVICE KEY" } else if self.key.is_some() { "THE DEVICE KEY IS HELD" } else { "NO DEVICE KEY WAITS FOR ONE" };
            mind::println!("[KEYSTORE] REFUSED UNLOCK FOR PID {}: {}", pid, why);
            return Err(Error::Invalid);
        }
        let mut seed = self.lock.unlock(pid, passphrase)?;
        let release = self.release;
        self.hold(&seed, Held::Kept, &release);
        stored::wipe(&mut seed);
        if self.trial { if stored::plain_present() { mind::println!("[KEYSTORE] THE UNENCRYPTED COPY {} STAYS UNTIL A BOOT NOT ON TRIAL", stored::FILE); } }
        else if stored::remove_plain().is_ok() { mind::println!("[KEYSTORE] THE UNENCRYPTED COPY {} REMOVED", stored::FILE); }
        Ok(())
    }

    // Seals the key held under a new passphrase (1.6), where no TPM seals it.
    fn passphrase_set(&mut self, badge: u16, pid: u64, old: &[u8], new: &[u8]) -> Result<(), Error> {
        lock::client(badge, pid, "PASSPHRASE-SET")?;
        let refuse = |why: &str, error: Error| { mind::println!("[KEYSTORE] REFUSED PASSPHRASE-SET FOR PID {}: {}", pid, why); Err(error) };
        if self.tpm { return refuse("THE TPM SEALS THE DEVICE KEY", Error::Invalid); }
        if self.trial { return refuse("THE BOOT IS ON TRIAL: SET IT AT A BOOT NOT ON TRIAL", Error::Invalid); }
        if self.key.is_some() && !self.kept { return refuse("THE DEVICE KEY IS ONE FOR THIS BOOT ONLY", Error::NoKey); }
        let mut seed = match (&self.key, self.state()) {
            (Some(key), _) => key.to_bytes(),
            (None, KeyState::Waiting) => return refuse("THE DEVICE KEY WAITS FOR ITS PASSPHRASE", Error::Invalid),
            (None, _) => return refuse("NO DEVICE KEY", Error::NoKey),
        };
        let result = self.lock.set(pid, &seed, old, new);
        stored::wipe(&mut seed);
        result
    }

    // A secrets client's call (1.4), or a boot service's (1.5): `f` gets the device key's seed (wiped after) and whose
    // secrets they are; `denied` to any other client, `no-key` without the device key.
    fn secret<T>(&mut self, badge: u16, pid: u64, f: impl FnOnce(&[u8; 32], &secrets::Owner) -> Result<T, Error>) -> Result<T, Error> {
        let owner = self.secrets.owner(badge, pid)?;
        // Under a key for this boot only, the owners' files would not open, and a new one would open at no later boot.
        if self.key.is_some() && !self.kept { mind::println!("[KEYSTORE] SECRETS REFUSED FOR PID {}: THE DEVICE KEY IS ONE FOR THIS BOOT ONLY", pid); return Err(Error::NoKey); }
        let mut seed = self.key.as_ref().map(SigningKey::to_bytes).ok_or(Error::NoKey)?;
        let result = f(&seed, &owner);
        stored::wipe(&mut seed);
        result
    }

    // The stored seed, or a new one made and stored (351-NET-0005); none without the random source, and where a
    // passphrase seals it (`lock` then holds the file it waits on) or a sealed file does not open (1.6). A key that is
    // there but does not open is never replaced on a boot on trial, nor one a TPM sealed while no TPM answers (801-NET-0079).
    fn seed(seed: &mut [u8; 32], tpm: bool, lock: &mut lock::Lock, trial: bool) -> Option<Held> {
        if tpm {
            if lock::present() { mind::println!("[KEYSTORE] {} LEFT AS IT IS: THE TPM SEALS THE DEVICE KEY, NOT A PASSPHRASE", passlock::FILE); }
            return Self::sealed_seed(seed, trial);
        }
        // A key sealed under a passphrase is never replaced: a file that does not open leaves no key, and stays.
        let kept = |why: core::fmt::Arguments| { mind::println!("[KEYSTORE] {} {}: NO DEVICE KEY, THE FILE LEFT AS IT IS AND NO NEW KEY MADE", passlock::FILE, why); None };
        match lock::find() {
            lock::Found::File(file) => {
                mind::println!("[KEYSTORE] NO TPM: THE DEVICE KEY IS SEALED UNDER THE OWNER'S PASSPHRASE IN {}: IT WAITS FOR IT", passlock::FILE);
                if File::open(stored::FILE).is_ok() { mind::println!("[KEYSTORE] THE UNENCRYPTED COPY {} IS REMOVED ONCE THE PASSPHRASE OPENS THE KEY", stored::FILE); }
                lock.file = Some(file);
                return None;
            }
            lock::Found::Damaged => return kept(format_args!("DAMAGED")),
            lock::Found::Version(version) => return kept(format_args!("OF VERSION {}, NOT {}", version, passlock::VERSION)),
            lock::Found::Unreadable(error) => return kept(format_args!("UNREADABLE ({:?})", error)),
            lock::Found::Missing => {}
        }
        // A key a TPM sealed, and no TPM answers this boot: it may answer at the next, so nothing is replaced (801-NET-0079).
        if stored::sealed_present() {
            mind::println!("[KEYSTORE] THE DEVICE KEY IS SEALED BY A TPM IN {}, BUT NO TPM ANSWERS: A KEY FOR THIS BOOT ONLY, NOTHING IN {} CHANGED", stored::SEALED, stored::DIR);
            return this_boot(seed);
        }
        mind::println!("[KEYSTORE] NO TPM: THE DEVICE KEY IS KEPT ON DISK, NOT SEALED");
        let made = |seed: &mut [u8; 32], why: &str| -> Option<Held> {
            if !random(seed) { return None; }
            match stored::store(seed) {
                Ok(()) => { mind::println!("[KEYSTORE] DEVICE KEY MADE{} AND STORED IN {} (ON DISK, NOT SEALED)", why, stored::FILE); Some(Held::Kept) }
                Err(error) => { mind::println!("[KEYSTORE] DEVICE KEY MADE{}, NOT STORED: {:?} (THIS BOOT ONLY)", why, error); Some(Held::ThisBoot) }
            }
        };
        match stored::load() {
            stored::Loaded::Seed(stored) => {
                seed.copy_from_slice(&stored);
                let mut stored = stored;
                stored::wipe(&mut stored);
                mind::println!("[KEYSTORE] DEVICE KEY FROM {} (ON DISK, NOT SEALED)", stored::FILE);
                Some(Held::Kept)
            }
            stored::Loaded::Missing if trial => untouched(seed, "NO DEVICE KEY IS STORED"),
            stored::Loaded::Missing => anew(made(seed, "")),
            stored::Loaded::Damaged if trial => untouched(seed, "THE STORED KEY IS DAMAGED"),
            // A damaged file is replaced: servers that knew the old key must be told the new one.
            stored::Loaded::Damaged => anew(made(seed, " ANEW: THE STORED ONE WAS DAMAGED")),
            stored::Loaded::Unreadable(error) => { mind::println!("[KEYSTORE] STORED KEY UNREADABLE: {:?}: A KEY FOR THIS BOOT ONLY, THE FILE LEFT AS IT IS", error); this_boot(seed) }
        }
    }

    // With a TPM (351-NET-0006): the sealed key; a key kept unencrypted before is sealed, and its file removed; a blob
    // this TPM does not open (another machine's, or a TPM cleared since) is replaced by a new key. On a boot on trial the
    // sealing and the removal wait, and nothing is replaced (801-NET-0079).
    fn sealed_seed(seed: &mut [u8; 32], trial: bool) -> Option<Held> {
        let made = |seed: &mut [u8; 32], why: &str| -> Option<Held> {
            if !random(seed) { return None; }
            match stored::seal(seed) {
                Ok(()) => { mind::println!("[KEYSTORE] DEVICE KEY MADE{} AND SEALED BY THE TPM IN {}", why, stored::SEALED); Some(Held::Kept) }
                Err(why_not) => { mind::println!("[KEYSTORE] DEVICE KEY MADE{}, NOT STORED: {} (THIS BOOT ONLY)", why, why_not); Some(Held::ThisBoot) }
            }
        };
        match stored::unseal() {
            stored::Unsealed::Seed(mut unsealed) => {
                seed.copy_from_slice(&unsealed);
                stored::wipe(&mut unsealed);
                mind::println!("[KEYSTORE] DEVICE KEY FROM {} (SEALED BY THE TPM)", stored::SEALED);
                if trial { if stored::plain_present() { mind::println!("[KEYSTORE] THE UNENCRYPTED COPY {} STAYS UNTIL A BOOT NOT ON TRIAL", stored::FILE); } }
                else if stored::remove_plain().is_ok() { mind::println!("[KEYSTORE] THE UNENCRYPTED COPY {} REMOVED", stored::FILE); }
                Some(Held::Kept)
            }
            stored::Unsealed::Missing => match stored::load() {
                // The transition from the interim: the same key, sealed; the unencrypted file goes once the sealed one is written.
                stored::Loaded::Seed(mut plain) => {
                    seed.copy_from_slice(&plain);
                    stored::wipe(&mut plain);
                    if trial {
                        mind::println!("[KEYSTORE] DEVICE KEY FROM {} (ON DISK, NOT SEALED): THE TPM SEALS IT AT A BOOT NOT ON TRIAL", stored::FILE);
                        return Some(Held::Kept);
                    }
                    match stored::seal(seed) {
                        Ok(()) => {
                            let removed = stored::remove_plain().is_ok();
                            mind::println!("[KEYSTORE] DEVICE KEY FROM {} SEALED BY THE TPM IN {}{}", stored::FILE, stored::SEALED, if removed { "; THE UNENCRYPTED COPY REMOVED" } else { "; THE UNENCRYPTED COPY NOT REMOVED" });
                        }
                        Err(why) => mind::println!("[KEYSTORE] DEVICE KEY FROM {} (ON DISK, NOT SEALED: {})", stored::FILE, why),
                    }
                    Some(Held::Kept)
                }
                stored::Loaded::Unreadable(error) => { mind::println!("[KEYSTORE] STORED KEY UNREADABLE: {:?}: A KEY FOR THIS BOOT ONLY, THE FILE LEFT AS IT IS", error); this_boot(seed) }
                stored::Loaded::Missing if trial => untouched(seed, "NO DEVICE KEY IS STORED"),
                stored::Loaded::Damaged if trial => untouched(seed, "THE STORED KEY IS DAMAGED"),
                _ => anew(made(seed, "")),
            },
            stored::Unsealed::Refused if trial => untouched(seed, "THE SEALED KEY DOES NOT OPEN ON THIS TPM"),
            stored::Unsealed::Refused => anew(made(seed, " ANEW: THE SEALED ONE DOES NOT OPEN ON THIS TPM")),
            stored::Unsealed::Damaged if trial => untouched(seed, "THE SEALED KEY IS DAMAGED"),
            stored::Unsealed::Damaged => anew(made(seed, " ANEW: THE SEALED ONE WAS DAMAGED")),
            stored::Unsealed::Unreadable(error) => { mind::println!("[KEYSTORE] SEALED KEY UNREADABLE: {:?}: A KEY FOR THIS BOOT ONLY, THE FILE LEFT AS IT IS", error); this_boot(seed) }
            // The sealed key is kept for the next boot: a key for this boot only.
            stored::Unsealed::Failed => { mind::println!("[KEYSTORE] THE TPM SERVICE DID NOT UNSEAL THE KEY: A KEY FOR THIS BOOT ONLY"); this_boot(seed) }
        }
    }

    // A call only the station's badge may make: `denied` to any other, in the log.
    fn station(badge: u16, pid: u64, what: &str) -> Result<(), Error> {
        if badge != BADGE_KEY_STATION { mind::println!("[KEYSTORE] REFUSED WI-FI {} FOR PID {}: NOT THE STATION", what, pid); return Err(Error::Denied); }
        Ok(())
    }

    // The remembered networks for a station's call: `denied` first to any other badge, then `no-key` without the key.
    fn wifi(&mut self, badge: u16, pid: u64, what: &str) -> Result<&mut wifi::Wifi, Error> {
        Self::station(badge, pid, what)?;
        self.wifi.as_mut().ok_or(Error::NoKey)
    }

    // Whether `data` has the form `purpose` names (so the key cannot sign anything else): an SSH login only with this key.
    fn fits(purpose: Purpose, data: &[u8], key: &SigningKey) -> bool {
        match purpose {
            Purpose::TlsClient => form::certificate_verify(data, form::CLIENT),
            Purpose::TlsServer => form::certificate_verify(data, form::SERVER),
            Purpose::SshLogin => ssh::login::fits(data, &key.verifying_key().to_bytes()),
            // An exchange hash alone: no other form the device key signs is 32 bytes long.
            Purpose::SshHost => data.len() == 32,
        }
    }

    // The badge a purpose is for, and its signatures and budget this boot (`tls-server`, `ssh-host`: this minute).
    fn allowance(&mut self, purpose: Purpose) -> (u16, &'static str, u32, u32) {
        let now = mind::time::uptime_ms();
        if now - self.minute >= MINUTE_MS { (self.minute, self.served, self.hosted) = (now, 0, 0); }
        match purpose {
            Purpose::TlsClient => (BADGE_KEY_SIGNER, "NOT THE SIGNER", self.usage.signatures, self.usage.budget),
            Purpose::SshLogin => (BADGE_KEY_SSH, "NOT THE SSH SERVICE", self.ssh_logins, SSH_BUDGET),
            Purpose::TlsServer => (BADGE_KEY_SIGNER, "NOT THE SIGNER", self.served, SERVER_BUDGET),
            Purpose::SshHost => (BADGE_KEY_SSH_HOST, "NOT THE SSH SERVER", self.hosted, HOST_BUDGET),
        }
    }

    fn sign(&mut self, badge: u16, pid: u64, purpose: Purpose, data: &[u8]) -> Result<[u8; 64], Error> {
        let (allowed, stranger, made, budget) = self.allowance(purpose);
        let result = if badge != allowed { Err((stranger, Error::Denied)) }
            else if self.key.is_none() { Err(("NO KEY", Error::NoKey)) }
            else if made >= budget { Err(("BUDGET USED", Error::Budget)) }
            else if !self.key.as_ref().is_some_and(|key| Self::fits(purpose, data, key)) { Err(("DATA DOES NOT FIT THE PURPOSE", Error::Invalid)) }
            else { Ok(()) };
        match (result, &self.key) {
            (Ok(()), Some(key)) => {
                match purpose { Purpose::TlsClient => self.usage.signatures += 1, Purpose::SshLogin => self.ssh_logins += 1, Purpose::TlsServer => self.served += 1, Purpose::SshHost => self.hosted += 1 }
                let period = if matches!(purpose, Purpose::TlsServer | Purpose::SshHost) { " THIS MINUTE" } else { "" };
                mind::println!("[KEYSTORE] SIGNED {:?} FOR PID {} ({} OF {}{})", purpose, pid, made + 1, budget, period);
                Ok(key.sign(data).to_bytes())
            }
            (Err((why, error)), _) => {
                self.usage.refused += 1;
                mind::println!("[KEYSTORE] REFUSED {:?} FOR PID {}: {}", purpose, pid, why);
                Err(error)
            }
            (Ok(()), None) => Err(Error::NoKey),
        }
    }
}

// A device key made anew and stored: the owners' secrets under the earlier one set aside (308-NET-0064).
fn anew(made: Option<Held>) -> Option<Held> {
    if made == Some(Held::Kept) { secrets::set_aside(); }
    made
}

// A key for this boot only: nothing is written under it.
fn this_boot(seed: &mut [u8; 32]) -> Option<Held> { random(seed).then_some(Held::ThisBoot) }

// On a boot on trial, a key that is not there or does not open: one for this boot only, and the files as they are.
fn untouched(seed: &mut [u8; 32], why: &str) -> Option<Held> {
    mind::println!("[KEYSTORE] THE BOOT IS ON TRIAL AND {}: A KEY FOR THIS BOOT ONLY, THE KEY FILES LEFT AS THEY ARE FOR THE SLOT BEFORE", why);
    this_boot(seed)
}

// Whether the TPM answers; while a key it sealed is there, it is waited for a few seconds, as its service may be starting.
fn tpm() -> bool {
    let start = mind::time::uptime_ms();
    loop {
        if stored::tpm() { return true; }
        if !stored::sealed_present() || mind::time::uptime_ms() - start >= TPM_WAIT_MS { return false; }
        mind::time::sleep(TPM_STEP_MS);
    }
}

// Random bytes, or a line in the log saying the service has no key without them.

fn random(bytes: &mut [u8]) -> bool {
    let filled = mind::random::fill(bytes);
    if !filled { mind::println!("[KEYSTORE] NO {}: NO DEVICE KEY", mind::random::SOURCE); }
    filled
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let mut keys = Keys::new(info.boot_slot.trial != 0, &info.boot_slot.manifest);
    // A secret's value passes through here: the copy is wiped after each request that carried one.
    let mut scratch = alloc::boxed::Box::new([0u8; keystore::REQUEST_MAX]);
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        if !request.is_call { continue; }
        let mut carried = false;
        let _ = match keystore::decode(&request, RECEIVED, &mut scratch) {
            Err(reason) => wire::reject(reason),
            Ok((keystore::Request::Certificate, call)) => {
                let result = if keys.certificate.is_empty() { Err(Error::NoKey) } else { Ok(keys.certificate.as_slice()) };
                keystore::reply_certificate(call, result)
            }
            Ok((keystore::Request::Sign { purpose, data }, call)) => {
                let signature = keys.sign(request.badge, request.sender, purpose, data);
                keystore::reply_sign(call, signature.as_ref().map(|s| &s[..]).map_err(|e| *e))
            }
            Ok((keystore::Request::Usage, call)) => keystore::reply_usage(call, Ok(&keys.usage)),
            Ok((keystore::Request::PublicKey, call)) => {
                let public = keys.key.as_ref().map(|key| key.verifying_key().to_bytes());
                keystore::reply_public_key(call, public.as_ref().map(|p| &p[..]).ok_or(Error::NoKey))
            }
            // The station's calls (1.1): `denied` to any other badge, `no-key` without the device key.
            Ok((keystore::Request::WifiRemember { ssid, pmk }, call)) => {
                let result = keys.wifi(request.badge, request.sender, "REMEMBER").and_then(|w| w.remember(request.sender, ssid.as_slice(), pmk.as_slice()));
                keystore::reply_wifi_remember(call, result)
            }
            Ok((keystore::Request::WifiNetworks, call)) => {
                let mut list = List::default();
                let result = keys.wifi(request.badge, request.sender, "LIST").and_then(|w| w.networks(&mut list));
                keystore::reply_wifi_networks(call, result.map(|_| list.as_slice()))
            }
            Ok((keystore::Request::WifiKeys { ssid, authenticator, supplicant, anonce, snonce }, call)) => {
                let result = keys.wifi(request.badge, request.sender, "KEYS").and_then(|w| w.keys(request.sender, ssid.as_slice(), authenticator, supplicant, anonce.as_slice(), snonce.as_slice()));
                let reply = keystore::reply_wifi_keys(call, result.as_ref().map(|k| &k[..]).map_err(|e| *e));
                if let Ok(mut k) = result { stored::wipe(&mut k); }
                reply
            }
            Ok((keystore::Request::WifiForget { ssid }, call)) => {
                let result = keys.wifi(request.badge, request.sender, "FORGET").and_then(|w| w.forget(request.sender, ssid.as_slice()));
                keystore::reply_wifi_forget(call, result)
            }
            // The station's country (1.3): its badge only; it needs no key, as nothing in it is secret.
            Ok((keystore::Request::WifiCountry, call)) => {
                let kept = Keys::station(request.badge, request.sender, "COUNTRY").map(|()| wifi::country());
                keystore::reply_wifi_country(call, kept.as_ref().map(|code| code.as_ref().map_or("", |c| core::str::from_utf8(c).unwrap_or(""))).map_err(|e| *e))
            }
            Ok((keystore::Request::WifiKeepCountry { code }, call)) => {
                let result = Keys::station(request.badge, request.sender, "COUNTRY").and_then(|()| wifi::keep_country(request.sender, code.as_str()));
                keystore::reply_wifi_keep_country(call, result)
            }
            // The loader's calls for a program's secrets client (1.4): its badge alone.
            Ok((keystore::Request::SecretsClient, call)) => keystore::reply_secrets_client(call, keys.secrets.client(request.badge, request.sender)),
            Ok((keystore::Request::SecretsName { client, program }, call)) => {
                keystore::reply_secrets_name(call, keys.secrets.name(request.badge, request.sender, client, program.as_str()))
            }
            Ok((keystore::Request::SecretsAdopt { client, task }, call)) => keystore::reply_secrets_adopt(call, keys.secrets.adopt(request.badge, request.sender, client, task)),
            // A program's calls on its secrets client, or the block store's on its own: about the caller's own secrets alone.
            Ok((keystore::Request::SecretPut { name, value }, call)) => {
                carried = true;
                let result = keys.secret(request.badge, request.sender, |seed, owner| secrets::put(seed, owner, request.sender, name.as_str(), value));
                keystore::reply_secret_put(call, result)
            }
            Ok((keystore::Request::SecretGet { name }, call)) => {
                let mut result = keys.secret(request.badge, request.sender, |seed, owner| secrets::get(seed, owner, request.sender, name.as_str()));
                let reply = keystore::reply_secret_get(call, result.as_deref().map_err(|e| *e));
                if let Ok(value) = result.as_mut() { stored::wipe(value); }
                reply
            }
            Ok((keystore::Request::SecretDelete { name }, call)) => {
                let result = keys.secret(request.badge, request.sender, |seed, owner| secrets::delete(seed, owner, request.sender, name.as_str()));
                keystore::reply_secret_delete(call, result)
            }
            Ok((keystore::Request::SecretList, call)) => {
                let mut list = List::default();
                let result = keys.secret(request.badge, request.sender, |seed, owner| secrets::list(seed, owner, &mut list));
                keystore::reply_secret_list(call, result.map(|_| list.as_slice()))
            }
            // The device key's state, to anyone, at once (1.6).
            Ok((keystore::Request::KeyState, call)) => keystore::reply_key_state(call, Ok(keys.state())),
            // The passphrase's calls (1.6): its badge alone; the decoded copies are wiped in place, then the request's.
            Ok((keystore::Request::Unlock { ref mut passphrase }, call)) => {
                carried = true;
                let result = keys.unlock(request.badge, request.sender, passphrase.as_str().as_bytes());
                stored::wipe_text(passphrase);
                keystore::reply_unlock(call, result)
            }
            Ok((keystore::Request::PassphraseSet { ref mut old, ref mut new }, call)) => {
                carried = true;
                let result = keys.passphrase_set(request.badge, request.sender, old.as_str().as_bytes(), new.as_str().as_bytes());
                stored::wipe_text(old);
                stored::wipe_text(new);
                keystore::reply_passphrase_set(call, result)
            }
        };
        if carried { stored::wipe(&mut scratch[..]); }
    }
}
