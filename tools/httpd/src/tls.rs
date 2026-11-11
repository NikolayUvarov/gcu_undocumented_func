//! TLS 1.3 for httpd: rustls over the RustCrypto crates, as `tls/src/provider.rs` (THIRD_PARTY.md), with the server's key from a file.

use std::fs::File;
use std::io::Read;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use aead::{AeadInPlace, KeyInit};
use hmac::{Mac, SimpleHmac};
use pki_types::pem::PemObject;
use pki_types::{CertificateDer, PrivateKeyDer};
use rustls::crypto::cipher::{self, AeadKey, BorrowedPayload, InboundOpaqueMessage, InboundPlainMessage, Iv, MessageDecrypter, MessageEncrypter, OutboundOpaqueMessage, OutboundPlainMessage, PrefixedPayload, Tls13AeadAlgorithm, UnsupportedOperationError};
use rustls::crypto::tls13::HkdfUsingHmac;
use rustls::crypto::{hash, hmac as rhmac, ActiveKeyExchange, CipherSuiteCommon, CryptoProvider, GetRandomFailed, KeyProvider, SecureRandom, SharedSecret, SupportedKxGroup, WebPkiSupportedAlgorithms};
use rustls::sign::{Signer, SigningKey};
use rustls::{CipherSuite, ConnectionTrafficSecrets, ContentType, NamedGroup, PeerMisbehaved, ProtocolVersion, ServerConfig, SignatureAlgorithm, SignatureScheme, SupportedCipherSuite, Tls13CipherSuite};
use sha2::digest::core_api::BlockSizeUser;
use sha2::{Digest, Sha256, Sha384, Sha512};

/// TLS 1.3 alone: three AEAD suites, X25519 and P-256, keys ECDSA P-256 and Ed25519 (and RSA-PSS with `rsa`); no client certificates.
pub fn provider(rsa: bool) -> Arc<CryptoProvider> {
    Arc::new(CryptoProvider {
        cipher_suites: vec![SupportedCipherSuite::Tls13(&TLS13_CHACHA20_POLY1305_SHA256), SupportedCipherSuite::Tls13(&TLS13_AES_256_GCM_SHA384), SupportedCipherSuite::Tls13(&TLS13_AES_128_GCM_SHA256)],
        kx_groups: vec![&X25519 as &dyn SupportedKxGroup, &SECP256R1],
        signature_verification_algorithms: NO_VERIFICATION,
        secure_random: &Urandom,
        key_provider: if rsa { &Keys { rsa: true } } else { &Keys { rsa: false } },
    })
}

// No client certificate is asked for, so none is verified.
static NO_VERIFICATION: WebPkiSupportedAlgorithms = WebPkiSupportedAlgorithms { all: &[], mapping: &[] };

// Random bytes from the kernel.

#[derive(Debug)]
struct Urandom;
impl SecureRandom for Urandom {
    fn fill(&self, out: &mut [u8]) -> Result<(), GetRandomFailed> { fill(out).map_err(|_| GetRandomFailed) }
}

fn fill(out: &mut [u8]) -> std::io::Result<()> { File::open("/dev/urandom")?.read_exact(out) }

fn random<const N: usize>() -> Result<[u8; N], rustls::Error> {
    let mut out = [0u8; N];
    fill(&mut out).map_err(|_| rustls::Error::FailedToGetRandomBytes)?;
    Ok(out)
}

// The RSA-PSS salt's source.
struct Rng;
impl rand_core::RngCore for Rng {
    fn next_u32(&mut self) -> u32 { let mut b = [0; 4]; self.fill_bytes(&mut b); u32::from_le_bytes(b) }
    fn next_u64(&mut self) -> u64 { let mut b = [0; 8]; self.fill_bytes(&mut b); u64::from_le_bytes(b) }
    fn fill_bytes(&mut self, out: &mut [u8]) { fill(out).expect("/dev/urandom") }
    fn try_fill_bytes(&mut self, out: &mut [u8]) -> Result<(), rand_core::Error> { fill(out).map_err(rand_core::Error::new) }
}
impl rand_core::CryptoRng for Rng {}

// The server's key: PKCS#8 (RSA, P-256, Ed25519), PKCS#1 (RSA) or SEC1 (P-256).

// RSA's private-key operations in the `rsa` crate are not constant-time (RUSTSEC-2023-0071): an RSA key is loaded only
// when asked for, for the tests' certificates; a server's own key is ECDSA P-256 or Ed25519.
#[derive(Debug)]
struct Keys { rsa: bool }
impl KeyProvider for Keys {
    fn load_private_key(&self, der: PrivateKeyDer<'static>) -> Result<Arc<dyn SigningKey>, rustls::Error> {
        use p256::pkcs8::DecodePrivateKey;
        use rsa::pkcs1::DecodeRsaPrivateKey;
        let unsupported = || rustls::Error::General("the key is not ECDSA P-256, Ed25519 or RSA".into());
        let rsa = |key: rsa::RsaPrivateKey| -> Result<Arc<dyn SigningKey>, rustls::Error> {
            if !self.rsa { return Err(rustls::Error::General("an RSA key: its signing is not constant-time in this build (RUSTSEC-2023-0071); use ECDSA P-256 or Ed25519, or --allow-rsa for test certificates".into())); }
            Ok(Arc::new(RsaKey::new(key)?))
        };
        Ok(match der {
            PrivateKeyDer::Pkcs8(k) => {
                let k = k.secret_pkcs8_der();
                if let Ok(key) = ed25519_dalek::SigningKey::from_pkcs8_der(k) { Arc::new(Ed25519Key::new(key)?) }
                else if let Ok(key) = p256::ecdsa::SigningKey::from_pkcs8_der(k) { Arc::new(P256Key::new(key)?) }
                else if let Ok(key) = rsa::RsaPrivateKey::from_pkcs8_der(k) { rsa(key)? }
                else { return Err(unsupported()) }
            }
            PrivateKeyDer::Pkcs1(k) => rsa(rsa::RsaPrivateKey::from_pkcs1_der(k.secret_pkcs1_der()).map_err(|_| unsupported())?)?,
            PrivateKeyDer::Sec1(k) => {
                let secret = p256::SecretKey::from_sec1_der(k.secret_sec1_der()).map_err(|_| unsupported())?;
                Arc::new(P256Key::new(p256::ecdsa::SigningKey::from(secret))?)
            }
            _ => return Err(unsupported()),
        })
    }
}

// The public half as a certificate carries it (its SubjectPublicKeyInfo), so rustls checks the pair at loading.
fn spki_der(encoded: Result<spki::Document, spki::Error>) -> Result<Vec<u8>, rustls::Error> {
    encoded.map(|d| d.as_bytes().to_vec()).map_err(|_| rustls::Error::General("the key's public half cannot be encoded".into()))
}

#[derive(Debug)]
struct RsaKey { sha256: Arc<rsa::pss::BlindedSigningKey<Sha256>>, sha384: Arc<rsa::pss::BlindedSigningKey<Sha384>>, sha512: Arc<rsa::pss::BlindedSigningKey<Sha512>>, spki: Vec<u8> }
impl RsaKey {
    fn new(key: rsa::RsaPrivateKey) -> Result<RsaKey, rustls::Error> {
        use rsa::pkcs8::EncodePublicKey;
        let spki = spki_der(key.to_public_key().to_public_key_der())?;
        Ok(RsaKey { sha256: Arc::new(key.clone().into()), sha384: Arc::new(key.clone().into()), sha512: Arc::new(key.into()), spki })
    }
}

#[derive(Debug)]
enum RsaSigner { Sha256(Arc<rsa::pss::BlindedSigningKey<Sha256>>), Sha384(Arc<rsa::pss::BlindedSigningKey<Sha384>>), Sha512(Arc<rsa::pss::BlindedSigningKey<Sha512>>) }

impl SigningKey for RsaKey {
    // TLS 1.3 signs with RSA only in PSS (RFC 8446 4.2.3).
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn Signer>> {
        let signer = if offered.contains(&SignatureScheme::RSA_PSS_SHA256) { RsaSigner::Sha256(self.sha256.clone()) }
            else if offered.contains(&SignatureScheme::RSA_PSS_SHA384) { RsaSigner::Sha384(self.sha384.clone()) }
            else if offered.contains(&SignatureScheme::RSA_PSS_SHA512) { RsaSigner::Sha512(self.sha512.clone()) }
            else { return None };
        Some(Box::new(signer))
    }
    fn public_key(&self) -> Option<pki_types::SubjectPublicKeyInfoDer<'_>> { Some(self.spki.as_slice().into()) }
    fn algorithm(&self) -> SignatureAlgorithm { SignatureAlgorithm::RSA }
}

impl Signer for RsaSigner {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rustls::Error> {
        use signature::{RandomizedSigner, SignatureEncoding};
        let failed = |_| rustls::Error::General("RSA-PSS signing failed".into());
        Ok(match self {
            RsaSigner::Sha256(k) => k.try_sign_with_rng(&mut Rng, message).map_err(failed)?.to_vec(),
            RsaSigner::Sha384(k) => k.try_sign_with_rng(&mut Rng, message).map_err(failed)?.to_vec(),
            RsaSigner::Sha512(k) => k.try_sign_with_rng(&mut Rng, message).map_err(failed)?.to_vec(),
        })
    }
    fn scheme(&self) -> SignatureScheme {
        match self { RsaSigner::Sha256(_) => SignatureScheme::RSA_PSS_SHA256, RsaSigner::Sha384(_) => SignatureScheme::RSA_PSS_SHA384, RsaSigner::Sha512(_) => SignatureScheme::RSA_PSS_SHA512 }
    }
}

#[derive(Clone, Debug)]
struct P256Key(p256::ecdsa::SigningKey, Vec<u8>);
impl P256Key {
    fn new(key: p256::ecdsa::SigningKey) -> Result<P256Key, rustls::Error> {
        use p256::pkcs8::EncodePublicKey;
        let spki = spki_der(p256::PublicKey::from(key.verifying_key()).to_public_key_der())?;
        Ok(P256Key(key, spki))
    }
}
impl SigningKey for P256Key {
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn Signer>> {
        offered.contains(&SignatureScheme::ECDSA_NISTP256_SHA256).then(|| Box::new(self.clone()) as Box<dyn Signer>)
    }
    fn public_key(&self) -> Option<pki_types::SubjectPublicKeyInfoDer<'_>> { Some(self.1.as_slice().into()) }
    fn algorithm(&self) -> SignatureAlgorithm { SignatureAlgorithm::ECDSA }
}
impl Signer for P256Key {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rustls::Error> {
        use signature::Signer as _;
        let signature: p256::ecdsa::Signature = self.0.try_sign(message).map_err(|_| rustls::Error::General("ECDSA signing failed".into()))?;
        Ok(signature.to_der().as_bytes().to_vec())
    }
    fn scheme(&self) -> SignatureScheme { SignatureScheme::ECDSA_NISTP256_SHA256 }
}

#[derive(Clone, Debug)]
struct Ed25519Key(ed25519_dalek::SigningKey, Vec<u8>);
impl Ed25519Key {
    fn new(key: ed25519_dalek::SigningKey) -> Result<Ed25519Key, rustls::Error> {
        use ed25519_dalek::pkcs8::EncodePublicKey;
        let spki = spki_der(key.verifying_key().to_public_key_der())?;
        Ok(Ed25519Key(key, spki))
    }
}
impl SigningKey for Ed25519Key {
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn Signer>> {
        offered.contains(&SignatureScheme::ED25519).then(|| Box::new(self.clone()) as Box<dyn Signer>)
    }
    fn public_key(&self) -> Option<pki_types::SubjectPublicKeyInfoDer<'_>> { Some(self.1.as_slice().into()) }
    fn algorithm(&self) -> SignatureAlgorithm { SignatureAlgorithm::ED25519 }
}
impl Signer for Ed25519Key {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rustls::Error> { use ed25519_dalek::Signer as _; Ok(self.0.sign(message).to_bytes().to_vec()) }
    fn scheme(&self) -> SignatureScheme { SignatureScheme::ED25519 }
}

/// The SHA-256 of the certificate's SubjectPublicKeyInfo with its outer SEQUENCE, as the updater's `pin` names it.
pub fn spki_sha256(certificate: &CertificateDer<'_>) -> Option<[u8; 32]> {
    let certificate = webpki::EndEntityCert::try_from(certificate).ok()?;
    Some(Sha256::digest(certificate.subject_public_key_info().as_ref()).into())
}

pub fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }

/// A server configuration from a PEM chain (the server's certificate first) and a PEM key, with its SPKI's hash.
pub fn load(cert: &Path, key: &Path, rsa: bool) -> Result<(Arc<ServerConfig>, [u8; 32]), String> {
    let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert).map_err(|e| format!("{}: {e}", cert.display()))?
        .collect::<Result<_, _>>().map_err(|e| format!("{}: {e}", cert.display()))?;
    let leaf = chain.first().ok_or_else(|| format!("{}: no certificate", cert.display()))?;
    let spki = spki_sha256(leaf).ok_or_else(|| format!("{}: the certificate cannot be read", cert.display()))?;
    let key = PrivateKeyDer::from_pem_file(key).map_err(|e| format!("{}: {e}", key.display()))?;
    let mut config = ServerConfig::builder_with_provider(provider(rsa)).with_protocol_versions(&[&rustls::version::TLS13]).map_err(|e| e.to_string())?
        .with_no_client_auth().with_single_cert(chain, key).map_err(|e| format!("the certificate and key: {e}"))?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok((Arc::new(config), spki))
}

/// The configuration new connections take, loaded again when its files change; one that fails to load leaves the last.
#[derive(Debug)]
pub struct Reloading { pub current: RwLock<Arc<ServerConfig>>, cert: PathBuf, key: PathBuf, rsa: bool }

impl Reloading {
    pub fn start(cert: PathBuf, key: PathBuf, rsa: bool, every: Duration) -> Result<Arc<Reloading>, String> {
        let (config, spki) = load(&cert, &key, rsa)?;
        eprintln!("HTTPD: TLS 1.3, THE KEY'S SPKI SHA-256 {}", hex(&spki));
        let reloading = Arc::new(Reloading { current: RwLock::new(config), cert, key, rsa });
        let watched = reloading.clone();
        std::thread::spawn(move || watched.watch(spki, every));
        Ok(reloading)
    }
    pub fn config(&self) -> Arc<ServerConfig> { self.current.read().unwrap().clone() }

    fn stamp(&self) -> Option<(SystemTime, SystemTime)> {
        Some((std::fs::metadata(&self.cert).ok()?.modified().ok()?, std::fs::metadata(&self.key).ok()?.modified().ok()?))
    }

    // A renewal keeps the key: a new one is published as a second pin first (351-UPD-0031), so a change is said aloud.
    fn watch(&self, mut spki: [u8; 32], every: Duration) {
        let mut seen = self.stamp();
        loop {
            std::thread::sleep(every);
            let now = self.stamp();
            if now == seen { continue; }
            seen = now;
            match load(&self.cert, &self.key, self.rsa) {
                Ok((config, new)) => {
                    *self.current.write().unwrap() = config;
                    if new == spki { eprintln!("HTTPD: THE CERTIFICATE RELOADED, THE SAME KEY"); }
                    else { eprintln!("HTTPD: THE CERTIFICATE RELOADED WITH ANOTHER KEY, SPKI SHA-256 {}: A DEVICE PINNING ONLY THE OLD ONE REFUSES THIS SERVER", hex(&new)); spki = new; }
                }
                Err(why) => eprintln!("HTTPD: THE CERTIFICATE OR KEY NOT LOADED ({why}); THE LAST ONES STAY"),
            }
        }
    }
}

// Hashes, HMAC and HKDF.

struct Hash<D>(hash::HashAlgorithm, PhantomData<fn() -> D>);
struct HashContext<D>(D);

impl<D: Digest + Clone + Send + Sync + 'static> hash::Hash for Hash<D> {
    fn start(&self) -> Box<dyn hash::Context> { Box::new(HashContext(D::new())) }
    fn hash(&self, data: &[u8]) -> hash::Output { hash::Output::new(&D::digest(data)) }
    fn output_len(&self) -> usize { <D as Digest>::output_size() }
    fn algorithm(&self) -> hash::HashAlgorithm { self.0 }
}

impl<D: Digest + Clone + Send + Sync + 'static> hash::Context for HashContext<D> {
    fn fork_finish(&self) -> hash::Output { hash::Output::new(&self.0.clone().finalize()) }
    fn fork(&self) -> Box<dyn hash::Context> { Box::new(HashContext(self.0.clone())) }
    fn finish(self: Box<Self>) -> hash::Output { hash::Output::new(&self.0.finalize()) }
    fn update(&mut self, data: &[u8]) { self.0.update(data); }
}

struct Hmac<D>(PhantomData<fn() -> D>);
struct HmacKey<D: Digest + BlockSizeUser>(SimpleHmac<D>);

impl<D: Digest + BlockSizeUser + Clone + Send + Sync + 'static> rhmac::Hmac for Hmac<D> {
    fn with_key(&self, key: &[u8]) -> Box<dyn rhmac::Key> { Box::new(HmacKey(<SimpleHmac<D> as KeyInit>::new_from_slice(key).expect("HMAC takes keys of any length"))) }
    fn hash_output_len(&self) -> usize { <D as Digest>::output_size() }
}

impl<D: Digest + BlockSizeUser + Clone + Send + Sync + 'static> rhmac::Key for HmacKey<D> {
    fn sign_concat(&self, first: &[u8], middle: &[&[u8]], last: &[u8]) -> rhmac::Tag {
        let mut mac = self.0.clone();
        Mac::update(&mut mac, first);
        for part in middle { Mac::update(&mut mac, part); }
        Mac::update(&mut mac, last);
        rhmac::Tag::new(&mac.finalize().into_bytes())
    }
    fn tag_len(&self) -> usize { <D as Digest>::output_size() }
}

static SHA256: Hash<Sha256> = Hash(hash::HashAlgorithm::SHA256, PhantomData);
static SHA384: Hash<Sha384> = Hash(hash::HashAlgorithm::SHA384, PhantomData);
static HMAC_SHA256: Hmac<Sha256> = Hmac(PhantomData);
static HMAC_SHA384: Hmac<Sha384> = Hmac(PhantomData);
static HKDF_SHA256: HkdfUsingHmac<'static> = HkdfUsingHmac(&HMAC_SHA256);
static HKDF_SHA384: HkdfUsingHmac<'static> = HkdfUsingHmac(&HMAC_SHA384);

// AEAD record protection (TLS 1.3, RFC 8446 5.2). All three algorithms have 12-byte nonces and 16-byte tags.

const TAG: usize = 16;
struct Aead<A>(PhantomData<fn() -> A>);
struct Cipher<A>(A, Iv);

impl<A: AeadInPlace + KeyInit + Send + Sync + 'static> Tls13AeadAlgorithm for Aead<A> {
    fn encrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageEncrypter> { Box::new(Cipher(A::new_from_slice(key.as_ref()).expect("key length from key_len"), iv)) }
    fn decrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageDecrypter> { Box::new(Cipher(A::new_from_slice(key.as_ref()).expect("key length from key_len"), iv)) }
    fn key_len(&self) -> usize { A::key_size() }
    fn extract_keys(&self, _: AeadKey, _: Iv) -> Result<ConnectionTrafficSecrets, UnsupportedOperationError> { Err(UnsupportedOperationError) }
}

impl<A: AeadInPlace + Send + Sync> MessageEncrypter for Cipher<A> {
    fn encrypt(&mut self, m: OutboundPlainMessage<'_>, seq: u64) -> Result<OutboundOpaqueMessage, rustls::Error> {
        let total = self.encrypted_payload_len(m.payload.len());
        let mut payload = PrefixedPayload::with_capacity(total);
        payload.extend_from_chunks(&m.payload);
        payload.extend_from_slice(&m.typ.to_array());
        let nonce = cipher::Nonce::new(&self.1, seq).0;
        self.0.encrypt_in_place(aead::Nonce::<A>::from_slice(&nonce), &cipher::make_tls13_aad(total), &mut Encrypting(&mut payload)).map_err(|_| rustls::Error::EncryptError)?;
        Ok(OutboundOpaqueMessage::new(ContentType::ApplicationData, ProtocolVersion::TLSv1_2, payload))
    }
    fn encrypted_payload_len(&self, length: usize) -> usize { length + 1 + TAG }
}

impl<A: AeadInPlace + Send + Sync> MessageDecrypter for Cipher<A> {
    fn decrypt<'a>(&mut self, mut m: InboundOpaqueMessage<'a>, seq: u64) -> Result<InboundPlainMessage<'a>, rustls::Error> {
        let payload = &mut m.payload;
        let nonce = cipher::Nonce::new(&self.1, seq).0;
        let aad = cipher::make_tls13_aad(payload.len());
        self.0.decrypt_in_place(aead::Nonce::<A>::from_slice(&nonce), &aad, &mut Decrypting(payload)).map_err(|_| rustls::Error::DecryptError)?;
        m.into_tls13_unpadded_message()
    }
}

struct Encrypting<'a>(&'a mut PrefixedPayload);
impl AsRef<[u8]> for Encrypting<'_> { fn as_ref(&self) -> &[u8] { self.0.as_ref() } }
impl AsMut<[u8]> for Encrypting<'_> { fn as_mut(&mut self) -> &mut [u8] { self.0.as_mut() } }
impl aead::Buffer for Encrypting<'_> {
    fn extend_from_slice(&mut self, other: &[u8]) -> aead::Result<()> { self.0.extend_from_slice(other); Ok(()) }
    fn truncate(&mut self, length: usize) { self.0.truncate(length) }
}

struct Decrypting<'a, 'p>(&'a mut BorrowedPayload<'p>);
impl AsRef<[u8]> for Decrypting<'_, '_> { fn as_ref(&self) -> &[u8] { self.0 } }
impl AsMut<[u8]> for Decrypting<'_, '_> { fn as_mut(&mut self) -> &mut [u8] { self.0 } }
impl aead::Buffer for Decrypting<'_, '_> {
    fn extend_from_slice(&mut self, _: &[u8]) -> aead::Result<()> { Err(aead::Error) } // decryption only shrinks
    fn truncate(&mut self, length: usize) { self.0.truncate(length) }
}

static CHACHA20_POLY1305: Aead<chacha20poly1305::ChaCha20Poly1305> = Aead(PhantomData);
static AES_128_GCM: Aead<aes_gcm::Aes128Gcm> = Aead(PhantomData);
static AES_256_GCM: Aead<aes_gcm::Aes256Gcm> = Aead(PhantomData);

static TLS13_CHACHA20_POLY1305_SHA256: Tls13CipherSuite = Tls13CipherSuite {
    common: CipherSuiteCommon { suite: CipherSuite::TLS13_CHACHA20_POLY1305_SHA256, hash_provider: &SHA256, confidentiality_limit: u64::MAX },
    hkdf_provider: &HKDF_SHA256, aead_alg: &CHACHA20_POLY1305, quic: None,
};
// AES-GCM: at most 2^24.5 records per key (RFC 9147 4.5.3); rustls updates the keys before.
static TLS13_AES_128_GCM_SHA256: Tls13CipherSuite = Tls13CipherSuite {
    common: CipherSuiteCommon { suite: CipherSuite::TLS13_AES_128_GCM_SHA256, hash_provider: &SHA256, confidentiality_limit: 1 << 24 },
    hkdf_provider: &HKDF_SHA256, aead_alg: &AES_128_GCM, quic: None,
};
static TLS13_AES_256_GCM_SHA384: Tls13CipherSuite = Tls13CipherSuite {
    common: CipherSuiteCommon { suite: CipherSuite::TLS13_AES_256_GCM_SHA384, hash_provider: &SHA384, confidentiality_limit: 1 << 24 },
    hkdf_provider: &HKDF_SHA384, aead_alg: &AES_256_GCM, quic: None,
};

// Key exchange: X25519 and secp256r1, ephemeral keys from RDRAND.

#[derive(Debug)]
struct X25519Group;
static X25519: X25519Group = X25519Group;
struct X25519Exchange { secret: x25519_dalek::StaticSecret, public: x25519_dalek::PublicKey }

impl SupportedKxGroup for X25519Group {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, rustls::Error> {
        let secret = x25519_dalek::StaticSecret::from(random::<32>()?);
        let public = x25519_dalek::PublicKey::from(&secret);
        Ok(Box::new(X25519Exchange { secret, public }))
    }
    fn name(&self) -> NamedGroup { NamedGroup::X25519 }
}

impl ActiveKeyExchange for X25519Exchange {
    fn complete(self: Box<Self>, peer: &[u8]) -> Result<SharedSecret, rustls::Error> {
        let peer: [u8; 32] = peer.try_into().map_err(|_| rustls::Error::from(PeerMisbehaved::InvalidKeyShare))?;
        let shared = self.secret.diffie_hellman(&peer.into());
        // An all-zero result means a small-order peer key (RFC 7748 6.1).
        if !shared.was_contributory() { return Err(PeerMisbehaved::InvalidKeyShare.into()); }
        Ok(SharedSecret::from(&shared.as_bytes()[..]))
    }
    fn pub_key(&self) -> &[u8] { self.public.as_bytes() }
    fn group(&self) -> NamedGroup { NamedGroup::X25519 }
}

#[derive(Debug)]
struct Secp256r1Group;
static SECP256R1: Secp256r1Group = Secp256r1Group;
struct Secp256r1Exchange { secret: p256::SecretKey, public: Vec<u8> }

impl SupportedKxGroup for Secp256r1Group {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, rustls::Error> {
        use p256::elliptic_curve::sec1::ToEncodedPoint;
        // A random scalar outside 1..n is drawn again (probability about 2^-32).
        let secret = loop { if let Ok(secret) = p256::SecretKey::from_slice(&random::<32>()?) { break secret; } };
        let public = secret.public_key().to_encoded_point(false).as_bytes().to_vec();
        Ok(Box::new(Secp256r1Exchange { secret, public }))
    }
    fn name(&self) -> NamedGroup { NamedGroup::secp256r1 }
}

impl ActiveKeyExchange for Secp256r1Exchange {
    fn complete(self: Box<Self>, peer: &[u8]) -> Result<SharedSecret, rustls::Error> {
        let peer = p256::PublicKey::from_sec1_bytes(peer).map_err(|_| rustls::Error::from(PeerMisbehaved::InvalidKeyShare))?;
        let shared = p256::ecdh::diffie_hellman(self.secret.to_nonzero_scalar(), peer.as_affine());
        Ok(SharedSecret::from(shared.raw_secret_bytes().as_slice()))
    }
    fn pub_key(&self) -> &[u8] { &self.public }
    fn group(&self) -> NamedGroup { NamedGroup::secp256r1 }
}

