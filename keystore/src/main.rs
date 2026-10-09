#![no_std]
#![no_main]
// Key service (issue 103, MC-11.9, Appendix B.6): the device key lives only here. It is an Ed25519 key made from
// RDRAND once (no RDRAND: no key, every request answers no-key) and kept across boots in this service's private
// directory of the boot disk (351-NET-0005; on disk, not sealed: a TPM is next), never exported. Serves
// idl/keystore.wit: the certificate to anyone; signatures only to the signer's badge (the TLS service), only for data of
// the form the purpose names, and at most BUDGET per boot. Holds: an RTC client (slot 2) for the certificate's start
// date, a VFS client with its own badge (slot 3) for `system/keystore`.
extern crate alloc;

mod certificate;
mod stored;

use alloc::vec::Vec;
use ed25519_dalek::{Signer, SigningKey};
use mind::abi::BootInfo;
use mind::idl::keystore::{self, Error, Purpose, Usage};
use mind::idl::wire;
use mind::ipc::Endpoint;
use mind::network::BADGE_KEY_SIGNER;

const RECEIVED: usize = 9;
const BUDGET: u32 = 4096; // signatures per boot
const TLS13_CLIENT: &[u8] = b"TLS 1.3, client CertificateVerify\0";

struct Keys { key: Option<SigningKey>, certificate: Vec<u8>, usage: Usage }

impl Keys {
    fn new() -> Self {
        let (mut seed, mut serial) = ([0u8; 32], [0u8; 16]);
        // Without the random source no key is made or used: the service fails closed, as the TLS service does.
        let key = (mind::random::fill(&mut serial) && Self::seed(&mut seed)).then(|| SigningKey::from_bytes(&seed));
        // The seed is the private key: wipe the stack copy (SigningKey wipes its own on drop).
        stored::wipe(&mut seed);
        let certificate = match &key {
            Some(key) => {
                let start = mind::rtc::unix_time().map(|t| t.saturating_sub(86400)).unwrap_or(0);
                let der = certificate::build(key, serial, start);
                let name = certificate::common_name(key);
                let name = core::str::from_utf8(name.as_bytes()).unwrap_or("");
                mind::println!("[KEYSTORE] DEVICE KEY READY: {} (ED25519, CERTIFICATE {} BYTES)", name, der.len());
                mind::println!("[KEYSTORE] PUBLIC KEY {} {}", certificate::openssh(key).as_str(), name.replace(' ', "-"));
                der
            }
            None => { mind::println!("[KEYSTORE] NO {}: NO DEVICE KEY", mind::random::SOURCE); Vec::new() }
        };
        Self { key, certificate, usage: Usage { signatures: 0, budget: BUDGET, refused: 0 } }
    }

    // The stored seed, or a new one made and stored (351-NET-0005); false without the random source.
    fn seed(seed: &mut [u8; 32]) -> bool {
        let made = |seed: &mut [u8; 32], why: &str| -> bool {
            if !mind::random::fill(seed) { return false; }
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
            stored::Loaded::Missing => made(seed, ""),
            // A damaged file is replaced: servers that knew the old key must be told the new one.
            stored::Loaded::Damaged => made(seed, " ANEW: THE STORED ONE WAS DAMAGED"),
            stored::Loaded::Unreadable(error) => { mind::println!("[KEYSTORE] STORED KEY UNREADABLE: {:?}", error); made(seed, " FOR THIS BOOT") }
        }
    }

    // Whether `data` has the form `purpose` names (so the key cannot sign anything else).
    fn fits(purpose: Purpose, data: &[u8]) -> bool {
        match purpose {
            Purpose::TlsClient => {
                let hash = data.len().wrapping_sub(64 + TLS13_CLIENT.len());
                (hash == 32 || hash == 48) && data[..64].iter().all(|&b| b == b' ') && &data[64..64 + TLS13_CLIENT.len()] == TLS13_CLIENT
            }
        }
    }

    fn sign(&mut self, badge: u16, pid: u64, purpose: Purpose, data: &[u8]) -> Result<[u8; 64], Error> {
        let result = if badge != BADGE_KEY_SIGNER { Err(("NOT THE SIGNER", Error::Denied)) }
            else if self.key.is_none() { Err(("NO KEY", Error::NoKey)) }
            else if self.usage.signatures >= self.usage.budget { Err(("BUDGET USED", Error::Budget)) }
            else if !Self::fits(purpose, data) { Err(("DATA DOES NOT FIT THE PURPOSE", Error::Invalid)) }
            else { Ok(()) };
        match (result, &self.key) {
            (Ok(()), Some(key)) => {
                self.usage.signatures += 1;
                mind::println!("[KEYSTORE] SIGNED {:?} FOR PID {} ({} OF {})", purpose, pid, self.usage.signatures, self.usage.budget);
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

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut keys = Keys::new();
    let mut scratch = [0u8; keystore::REQUEST_MAX];
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        if !request.is_call { continue; }
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
        };
    }
}
