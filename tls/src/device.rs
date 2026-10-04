// The device certificate as a rustls client certificate: the certificate comes from the key service and so does every
// signature (idl/keystore.wit). The private key never enters this process.
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use mind::idl::keystore::{self, Purpose};
use mind::ipc::Endpoint;
use pki_types::CertificateDer;
use rustls::client::ResolvesClientCert;
use rustls::sign::{CertifiedKey, Signer, SigningKey};
use rustls::{SignatureAlgorithm, SignatureScheme};

#[derive(Debug)]
struct DeviceKey(Endpoint);

impl SigningKey for DeviceKey {
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn Signer>> {
        offered.contains(&SignatureScheme::ED25519).then(|| Box::new(DeviceSigner(self.0)) as Box<dyn Signer>)
    }
    fn algorithm(&self) -> SignatureAlgorithm { SignatureAlgorithm::ED25519 }
}

#[derive(Debug)]
struct DeviceSigner(Endpoint);

impl Signer for DeviceSigner {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rustls::Error> {
        let mut signature = [0u8; 64];
        match keystore::sign(self.0, Purpose::TlsClient, message, &mut signature) {
            Ok(Ok(64)) => Ok(signature.to_vec()),
            Ok(Ok(_)) => Err(rustls::Error::General("short signature from the key service".into())),
            Ok(Err(error)) => Err(rustls::Error::General(alloc::format!("key service: {:?}", error))),
            Err(error) => Err(rustls::Error::General(alloc::format!("key service unreachable: {:?}", error))),
        }
    }
    fn scheme(&self) -> SignatureScheme { SignatureScheme::ED25519 }
}

/// Offers the device certificate when a server asks for one with Ed25519 among its schemes; remembers that it did.
#[derive(Debug)]
pub struct Device { key: Arc<CertifiedKey>, pub sent: AtomicBool }

impl Device {
    /// The device certificate from the key service at `keys`, or None without one.
    pub fn new(keys: Endpoint) -> Option<Self> {
        let mut der = [0u8; 512];
        let length = keystore::certificate(keys, &mut der).ok()?.ok()?;
        let certificate = CertificateDer::from(der[..length].to_vec());
        Some(Self { key: Arc::new(CertifiedKey::new(alloc::vec![certificate], Arc::new(DeviceKey(keys)))), sent: AtomicBool::new(false) })
    }
}

impl ResolvesClientCert for Device {
    fn resolve(&self, _: &[&[u8]], schemes: &[SignatureScheme]) -> Option<Arc<CertifiedKey>> {
        let usable = schemes.contains(&SignatureScheme::ED25519);
        self.sent.store(usable, Ordering::Relaxed);
        usable.then(|| self.key.clone())
    }
    fn has_certs(&self) -> bool { true }
}
