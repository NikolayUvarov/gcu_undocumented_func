// The device certificate as a rustls client certificate, and (1.3) as a server's: the certificate comes from the key
// service and so does every signature (idl/keystore.wit). The private key never enters this process.
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use mind::idl::keystore::{self, Purpose};
use mind::ipc::Endpoint;
use pki_types::CertificateDer;
use rustls::client::ResolvesClientCert;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::{CertifiedKey, Signer, SigningKey};
use rustls::{SignatureAlgorithm, SignatureScheme};

#[derive(Debug)]
struct DeviceKey(Endpoint, Purpose);

impl SigningKey for DeviceKey {
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn Signer>> {
        offered.contains(&SignatureScheme::ED25519).then(|| Box::new(DeviceSigner(self.0, self.1)) as Box<dyn Signer>)
    }
    fn algorithm(&self) -> SignatureAlgorithm { SignatureAlgorithm::ED25519 }
}

#[derive(Debug)]
struct DeviceSigner(Endpoint, Purpose);

impl Signer for DeviceSigner {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rustls::Error> {
        let mut signature = [0u8; 64];
        match keystore::sign(self.0, self.1, message, &mut signature) {
            Ok(Ok(64)) => Ok(signature.to_vec()),
            Ok(Ok(_)) => Err(rustls::Error::General("short signature from the key service".into())),
            Ok(Err(error)) => Err(rustls::Error::General(alloc::format!("key service: {:?}", error))),
            Err(error) => Err(rustls::Error::General(alloc::format!("key service unreachable: {:?}", error))),
        }
    }
    fn scheme(&self) -> SignatureScheme { SignatureScheme::ED25519 }
}

/// The device certificate (DER) from the key service at `keys`, or None without one.
pub fn certificate(keys: Endpoint) -> Option<CertificateDer<'static>> {
    let mut der = [0u8; 512];
    let length = keystore::certificate(keys, &mut der).ok()?.ok()?;
    Some(CertificateDer::from(der[..length].to_vec()))
}

/// Offers the device certificate when a server asks for one with Ed25519 among its schemes; remembers that it did.
#[derive(Debug)]
pub struct Device { key: Arc<CertifiedKey>, pub sent: AtomicBool }

impl Device {
    /// The device certificate from the key service at `keys`, or None without one.
    pub fn new(keys: Endpoint) -> Option<Self> {
        let key = CertifiedKey::new(alloc::vec![certificate(keys)?], Arc::new(DeviceKey(keys, Purpose::TlsClient)));
        Some(Self { key: Arc::new(key), sent: AtomicBool::new(false) })
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

/// The server's chain (1.3): its first certificate holds the device key, which the key service signs with for
/// `tls-server` alone. Given to a peer that offers Ed25519; to any other none, and its handshake fails.
#[derive(Debug)]
pub struct Served { key: Arc<CertifiedKey> }

impl Served {
    pub fn new(keys: Endpoint, chain: Vec<CertificateDer<'static>>) -> Self {
        Self { key: Arc::new(CertifiedKey::new(chain, Arc::new(DeviceKey(keys, Purpose::TlsServer)))) }
    }
}

impl ResolvesServerCert for Served {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        hello.signature_schemes().contains(&SignatureScheme::ED25519).then(|| self.key.clone())
    }
}
