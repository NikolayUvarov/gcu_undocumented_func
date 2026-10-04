#![no_std]
#![no_main]
// Key service (issue 103, MC-11.9, Appendix B.6): the device key lives only here. It is an Ed25519 key made from
// RDRAND at boot (no RDRAND: no key, every request answers no-key), kept in this process's memory and never stored or
// exported. Serves idl/keystore.wit: the certificate to anyone; signatures only to the signer's badge (the TLS service),
// only for data of the form the purpose names, and at most BUDGET per boot. Holds: an RTC client (slot 2) for the
// certificate's start date.
extern crate alloc;

mod certificate;

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
        let key = (mind::random::fill(&mut seed) && mind::random::fill(&mut serial)).then(|| SigningKey::from_bytes(&seed));
        // The seed is the private key: wipe the stack copy (SigningKey wipes its own on drop).
        for byte in seed.iter_mut() { unsafe { core::ptr::write_volatile(byte, 0) }; }
        let certificate = match &key {
            Some(key) => {
                let start = mind::rtc::unix_time().map(|t| t.saturating_sub(86400)).unwrap_or(0);
                let der = certificate::build(key, serial, start);
                mind::println!("[KEYSTORE] DEVICE KEY READY: {} (ED25519, CERTIFICATE {} BYTES)", core::str::from_utf8(certificate::common_name(key).as_bytes()).unwrap_or(""), der.len());
                der
            }
            None => { mind::println!("[KEYSTORE] NO RDRAND: NO DEVICE KEY"); Vec::new() }
        };
        Self { key, certificate, usage: Usage { signatures: 0, budget: BUDGET, refused: 0 } }
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
