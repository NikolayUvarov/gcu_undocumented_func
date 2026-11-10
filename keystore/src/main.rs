#![no_std]
#![no_main]
// Key service (issue 103, MC-11.9, Appendix B.6): the device key lives only here. It is an Ed25519 key made from
// RDRAND once (no RDRAND: no key, every request answers no-key) and kept across boots in this service's private
// directory of the boot disk, sealed by the TPM where there is one (351-NET-0006); else unencrypted (351-NET-0005)
// until the owner sets a passphrase, and from then on sealed under it (1.6, lock.rs, 308-NET-0055): the service then
// starts without the key (`key-state` waiting) and holds it, with all that opens under it, once `unlock` opens it.
// Never exported. Serves
// idl/keystore.wit: the certificate and the public key to anyone; signatures only to the signer's badge (the TLS
// service) for TLS and to the SSH service's badge for SSH logins (1.2), only for data of the form the purpose names, and
// within each one's budget per boot; the Wi-Fi station's remembered networks (1.1, wifi.rs) and its country (1.3) only to the station's badge;
// each program's secrets (1.4, secrets.rs) only to the client the loader got for that program, and the block store's own
// (1.5) only to the client init badged for it; `key-state` to anyone, `unlock` and `passphrase-set` only to the
// passphrase's badge (1.6). Holds: an RTC client (slot 2) for the certificate's start
// date, a VFS client with its own badge (slot 3) for `system/keystore`, the TPM service's client with the seal badge
// (slot 4).
extern crate alloc;

mod certificate;
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
use mind::network::{BADGE_KEY_SIGNER, BADGE_KEY_SSH, BADGE_KEY_STATION};

const RECEIVED: usize = 9;
const BUDGET: u32 = 4096; // TLS signatures per boot
const SSH_BUDGET: u32 = 256; // SSH logins per boot (1.2)
const TLS13_CLIENT: &[u8] = b"TLS 1.3, client CertificateVerify\0";

struct Keys {
    key: Option<SigningKey>, certificate: Vec<u8>, usage: Usage, ssh_logins: u32, wifi: Option<wifi::Wifi>, secrets: secrets::Clients,
    // The device key sealed under a passphrase (1.6); whether a TPM seals it instead; the certificate's serial.
    lock: lock::Lock, tpm: bool, serial: [u8; 16],
}

impl Keys {
    fn new() -> Self {
        let mut keys = Self { key: None, certificate: Vec::new(), usage: Usage { signatures: 0, budget: BUDGET, refused: 0 }, ssh_logins: 0, wifi: None,
                              secrets: secrets::Clients::new(), lock: lock::Lock::new(), tpm: stored::tpm(), serial: [0; 16] };
        // Without the random source no key is made or used: the service fails closed, as the TLS service does.
        if !random(&mut keys.serial) { return keys; }
        let mut seed = [0u8; 32];
        if Self::seed(&mut seed, keys.tpm, &mut keys.lock) { keys.hold(&seed); }
        // The seed is the private key: wipe the stack copy (SigningKey wipes its own on drop).
        stored::wipe(&mut seed);
        keys
    }

    // Holds the key of `seed`: its certificate, its public key in the log, and the remembered Wi-Fi networks, which open
    // under it (so there are none without it).
    fn hold(&mut self, seed: &[u8; 32]) {
        let key = SigningKey::from_bytes(seed);
        let start = mind::rtc::unix_time().map(|t| t.saturating_sub(86400)).unwrap_or(0);
        let der = certificate::build(&key, self.serial, start);
        let name = certificate::common_name(&key);
        let name = core::str::from_utf8(name.as_bytes()).unwrap_or("");
        mind::println!("[KEYSTORE] DEVICE KEY READY: {} (ED25519, CERTIFICATE {} BYTES)", name, der.len());
        mind::println!("[KEYSTORE] PUBLIC KEY {} {}", certificate::openssh(&key).as_str(), name.replace(' ', "-"));
        self.wifi = Some(wifi::Wifi::load(seed));
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
        self.hold(&seed);
        stored::wipe(&mut seed);
        if stored::remove_plain().is_ok() { mind::println!("[KEYSTORE] THE UNENCRYPTED COPY {} REMOVED", stored::FILE); }
        Ok(())
    }

    // Seals the key held under a new passphrase (1.6), where no TPM seals it.
    fn passphrase_set(&mut self, badge: u16, pid: u64, old: &[u8], new: &[u8]) -> Result<(), Error> {
        lock::client(badge, pid, "PASSPHRASE-SET")?;
        let refuse = |why: &str, error: Error| { mind::println!("[KEYSTORE] REFUSED PASSPHRASE-SET FOR PID {}: {}", pid, why); Err(error) };
        if self.tpm { return refuse("THE TPM SEALS THE DEVICE KEY", Error::Invalid); }
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
        let mut seed = self.key.as_ref().map(SigningKey::to_bytes).ok_or(Error::NoKey)?;
        let result = f(&seed, &owner);
        stored::wipe(&mut seed);
        result
    }

    // The stored seed, or a new one made and stored (351-NET-0005); false without the random source, and where a
    // passphrase seals it (`lock` then holds the file it waits on) or a sealed file does not open (1.6).
    fn seed(seed: &mut [u8; 32], tpm: bool, lock: &mut lock::Lock) -> bool {
        if tpm {
            if lock::present() { mind::println!("[KEYSTORE] {} LEFT AS IT IS: THE TPM SEALS THE DEVICE KEY, NOT A PASSPHRASE", passlock::FILE); }
            return Self::sealed_seed(seed);
        }
        // A key sealed under a passphrase is never replaced: a file that does not open leaves no key, and stays.
        let kept = |why: core::fmt::Arguments| { mind::println!("[KEYSTORE] {} {}: NO DEVICE KEY, THE FILE LEFT AS IT IS AND NO NEW KEY MADE", passlock::FILE, why); false };
        match lock::find() {
            lock::Found::File(file) => {
                mind::println!("[KEYSTORE] NO TPM: THE DEVICE KEY IS SEALED UNDER THE OWNER'S PASSPHRASE IN {}: IT WAITS FOR IT", passlock::FILE);
                if File::open(stored::FILE).is_ok() { mind::println!("[KEYSTORE] THE UNENCRYPTED COPY {} IS REMOVED ONCE THE PASSPHRASE OPENS THE KEY", stored::FILE); }
                lock.file = Some(file);
                return false;
            }
            lock::Found::Damaged => return kept(format_args!("DAMAGED")),
            lock::Found::Version(version) => return kept(format_args!("OF VERSION {}, NOT {}", version, passlock::VERSION)),
            lock::Found::Unreadable(error) => return kept(format_args!("UNREADABLE ({:?})", error)),
            lock::Found::Missing => {}
        }
        mind::println!("[KEYSTORE] NO TPM: THE DEVICE KEY IS KEPT ON DISK, NOT SEALED");
        let made = |seed: &mut [u8; 32], why: &str| -> bool {
            if !random(seed) { return false; }
            match stored::store(seed) {
                Ok(()) => mind::println!("[KEYSTORE] DEVICE KEY MADE{} AND STORED IN {} (ON DISK, NOT SEALED)", why, stored::FILE),
                Err(error) => mind::println!("[KEYSTORE] DEVICE KEY MADE{}, NOT STORED: {:?} (THIS BOOT ONLY)", why, error),
            }
            true
        };
        match stored::load() {
            stored::Loaded::Seed(stored) => {
                seed.copy_from_slice(&stored);
                let mut stored = stored;
                stored::wipe(&mut stored);
                mind::println!("[KEYSTORE] DEVICE KEY FROM {} (ON DISK, NOT SEALED)", stored::FILE);
                true
            }
            stored::Loaded::Missing => anew(made(seed, "")),
            // A damaged file is replaced: servers that knew the old key must be told the new one.
            stored::Loaded::Damaged => anew(made(seed, " ANEW: THE STORED ONE WAS DAMAGED")),
            stored::Loaded::Unreadable(error) => { mind::println!("[KEYSTORE] STORED KEY UNREADABLE: {:?}", error); made(seed, " FOR THIS BOOT") }
        }
    }

    // With a TPM (351-NET-0006): the sealed key; a key kept unencrypted before is sealed, and its file removed; a blob
    // this TPM does not open (another machine's, or a TPM cleared since) is replaced by a new key.
    fn sealed_seed(seed: &mut [u8; 32]) -> bool {
        let made = |seed: &mut [u8; 32], why: &str| -> bool {
            if !random(seed) { return false; }
            match stored::seal(seed) {
                Ok(()) => mind::println!("[KEYSTORE] DEVICE KEY MADE{} AND SEALED BY THE TPM IN {}", why, stored::SEALED),
                Err(why_not) => mind::println!("[KEYSTORE] DEVICE KEY MADE{}, NOT STORED: {} (THIS BOOT ONLY)", why, why_not),
            }
            true
        };
        match stored::unseal() {
            stored::Unsealed::Seed(mut unsealed) => {
                seed.copy_from_slice(&unsealed);
                stored::wipe(&mut unsealed);
                mind::println!("[KEYSTORE] DEVICE KEY FROM {} (SEALED BY THE TPM)", stored::SEALED);
                if stored::remove_plain().is_ok() { mind::println!("[KEYSTORE] THE UNENCRYPTED COPY {} REMOVED", stored::FILE); }
                true
            }
            stored::Unsealed::Missing => match stored::load() {
                // The transition from the interim: the same key, sealed; the unencrypted file goes once the sealed one is written.
                stored::Loaded::Seed(mut plain) => {
                    seed.copy_from_slice(&plain);
                    stored::wipe(&mut plain);
                    match stored::seal(seed) {
                        Ok(()) => {
                            let removed = stored::remove_plain().is_ok();
                            mind::println!("[KEYSTORE] DEVICE KEY FROM {} SEALED BY THE TPM IN {}{}", stored::FILE, stored::SEALED, if removed { "; THE UNENCRYPTED COPY REMOVED" } else { "; THE UNENCRYPTED COPY NOT REMOVED" });
                        }
                        Err(why) => mind::println!("[KEYSTORE] DEVICE KEY FROM {} (ON DISK, NOT SEALED: {})", stored::FILE, why),
                    }
                    true
                }
                _ => anew(made(seed, "")),
            },
            stored::Unsealed::Refused => anew(made(seed, " ANEW: THE SEALED ONE DOES NOT OPEN ON THIS TPM")),
            stored::Unsealed::Damaged => anew(made(seed, " ANEW: THE SEALED ONE WAS DAMAGED")),
            stored::Unsealed::Unreadable(error) => { mind::println!("[KEYSTORE] SEALED KEY UNREADABLE: {:?}", error); made(seed, " FOR THIS BOOT") }
            // The sealed key is kept for the next boot: a key for this boot only.
            stored::Unsealed::Failed => { mind::println!("[KEYSTORE] THE TPM SERVICE DID NOT UNSEAL THE KEY: A KEY FOR THIS BOOT ONLY"); random(seed) }
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
            Purpose::TlsClient => {
                let hash = data.len().wrapping_sub(64 + TLS13_CLIENT.len());
                (hash == 32 || hash == 48) && data[..64].iter().all(|&b| b == b' ') && &data[64..64 + TLS13_CLIENT.len()] == TLS13_CLIENT
            }
            Purpose::SshLogin => ssh::login::fits(data, &key.verifying_key().to_bytes()),
        }
    }

    // The badge a purpose is for, and its signatures and budget this boot.
    fn allowance(&self, purpose: Purpose) -> (u16, &'static str, u32, u32) {
        match purpose {
            Purpose::TlsClient => (BADGE_KEY_SIGNER, "NOT THE SIGNER", self.usage.signatures, self.usage.budget),
            Purpose::SshLogin => (BADGE_KEY_SSH, "NOT THE SSH SERVICE", self.ssh_logins, SSH_BUDGET),
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
                match purpose { Purpose::TlsClient => self.usage.signatures += 1, Purpose::SshLogin => self.ssh_logins += 1 }
                mind::println!("[KEYSTORE] SIGNED {:?} FOR PID {} ({} OF {})", purpose, pid, made + 1, budget);
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

// Random bytes, or a line in the log saying the service has no key without them.
// A device key made anew, not for this boot only: the owners' secrets under the earlier one set aside (308-NET-0064).
fn anew(made: bool) -> bool {
    if made { secrets::set_aside(); }
    made
}

fn random(bytes: &mut [u8]) -> bool {
    let filled = mind::random::fill(bytes);
    if !filled { mind::println!("[KEYSTORE] NO {}: NO DEVICE KEY", mind::random::SOURCE); }
    filled
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut keys = Keys::new();
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
