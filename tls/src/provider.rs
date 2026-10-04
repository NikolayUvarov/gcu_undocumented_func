// A rustls crypto provider over the RustCrypto crates, TLS 1.3 only: the cipher suites, key exchange groups and
// signature verification rustls asks for, random bytes from RDRAND and time from the RTC. Adapted from
// rustls-rustcrypto 0.0.2-alpha (MIT OR Apache-2.0, THIRD_PARTY.md). There is no private key here: the client's key
// lives in the key service (src/device.rs).
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use aead::{AeadInPlace, KeyInit};
use hmac::{Mac, SimpleHmac};
use pki_types::{AlgorithmIdentifier, InvalidSignature, SignatureVerificationAlgorithm, UnixTime};
use rustls::crypto::cipher::{self, AeadKey, BorrowedPayload, InboundOpaqueMessage, InboundPlainMessage, Iv, MessageDecrypter, MessageEncrypter, OutboundOpaqueMessage, OutboundPlainMessage, PrefixedPayload, Tls13AeadAlgorithm, UnsupportedOperationError};
use rustls::crypto::tls13::HkdfUsingHmac;
use rustls::crypto::{hash, hmac as rhmac, ActiveKeyExchange, CipherSuiteCommon, CryptoProvider, GetRandomFailed, KeyProvider, SecureRandom, SharedSecret, SupportedKxGroup, WebPkiSupportedAlgorithms};
use rustls::{CipherSuite, ConnectionTrafficSecrets, ContentType, NamedGroup, PeerMisbehaved, ProtocolVersion, SignatureScheme, SupportedCipherSuite, Tls13CipherSuite};
use sha2::digest::core_api::BlockSizeUser;
use sha2::{Digest, Sha256, Sha384, Sha512};
use pki_types::alg_id;

pub fn provider() -> Arc<CryptoProvider> {
    Arc::new(CryptoProvider {
        cipher_suites: alloc::vec![SupportedCipherSuite::Tls13(&TLS13_CHACHA20_POLY1305_SHA256), SupportedCipherSuite::Tls13(&TLS13_AES_256_GCM_SHA384), SupportedCipherSuite::Tls13(&TLS13_AES_128_GCM_SHA256)],
        kx_groups: alloc::vec![&X25519 as &dyn SupportedKxGroup, &SECP256R1],
        signature_verification_algorithms: ALGORITHMS,
        secure_random: &Rdrand,
        key_provider: &NoKeys,
    })
}

// Random bytes and time.

#[derive(Debug)]
pub struct Rdrand;
impl SecureRandom for Rdrand {
    fn fill(&self, out: &mut [u8]) -> Result<(), GetRandomFailed> { if mind::random::fill(out) { Ok(()) } else { Err(GetRandomFailed) } }
}

fn random<const N: usize>() -> Result<[u8; N], rustls::Error> {
    let mut out = [0u8; N];
    if mind::random::fill(&mut out) { Ok(out) } else { Err(rustls::Error::FailedToGetRandomBytes) }
}

#[derive(Debug)]
pub struct Rtc;
impl rustls::time_provider::TimeProvider for Rtc {
    fn current_time(&self) -> Option<UnixTime> { mind::rtc::unix_time().map(|s| UnixTime::since_unix_epoch(core::time::Duration::from_secs(s))) }
}

#[derive(Debug)]
struct NoKeys;
impl KeyProvider for NoKeys {
    fn load_private_key(&self, _: pki_types::PrivateKeyDer<'static>) -> Result<Arc<dyn rustls::sign::SigningKey>, rustls::Error> {
        Err(rustls::Error::General("private keys live in the key service".into()))
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

// Signature verification of certificates and handshakes: ECDSA P-256/P-384, Ed25519, RSA PKCS#1 v1.5 and PSS.

macro_rules! ecdsa {
    ($name:ident, $curve:expr, $signature:expr, $key:ty, $hash:ty) => {
        #[derive(Debug)]
        struct $name;
        impl SignatureVerificationAlgorithm for $name {
            fn public_key_alg_id(&self) -> AlgorithmIdentifier { $curve }
            fn signature_alg_id(&self) -> AlgorithmIdentifier { $signature }
            fn verify_signature(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<(), InvalidSignature> {
                use signature::hazmat::PrehashVerifier;
                let signature = ecdsa::der::Signature::from_bytes(signature).map_err(|_| InvalidSignature)?;
                let key = <$key>::from_sec1_bytes(public_key).map_err(|_| InvalidSignature)?;
                key.verify_prehash(&<$hash>::digest(message), &signature).map_err(|_| InvalidSignature)
            }
        }
    };
}

ecdsa!(EcdsaP256Sha256, alg_id::ECDSA_P256, alg_id::ECDSA_SHA256, p256::ecdsa::VerifyingKey, Sha256);
ecdsa!(EcdsaP256Sha384, alg_id::ECDSA_P256, alg_id::ECDSA_SHA384, p256::ecdsa::VerifyingKey, Sha384);
ecdsa!(EcdsaP384Sha256, alg_id::ECDSA_P384, alg_id::ECDSA_SHA256, p384::ecdsa::VerifyingKey, Sha256);
ecdsa!(EcdsaP384Sha384, alg_id::ECDSA_P384, alg_id::ECDSA_SHA384, p384::ecdsa::VerifyingKey, Sha384);

macro_rules! rsa {
    ($name:ident, $signature_id:expr, $verifier:ty, $signature:ty) => {
        #[derive(Debug)]
        struct $name;
        impl SignatureVerificationAlgorithm for $name {
            fn public_key_alg_id(&self) -> AlgorithmIdentifier { alg_id::RSA_ENCRYPTION }
            fn signature_alg_id(&self) -> AlgorithmIdentifier { $signature_id }
            fn verify_signature(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<(), InvalidSignature> {
                use rsa::pkcs1::DecodeRsaPublicKey;
                use signature::Verifier;
                let key = rsa::RsaPublicKey::from_pkcs1_der(public_key).map_err(|_| InvalidSignature)?;
                let signature = <$signature>::try_from(signature).map_err(|_| InvalidSignature)?;
                <$verifier>::new(key).verify(message, &signature).map_err(|_| InvalidSignature)
            }
        }
    };
}

rsa!(RsaPkcs1Sha256, alg_id::RSA_PKCS1_SHA256, rsa::pkcs1v15::VerifyingKey<Sha256>, rsa::pkcs1v15::Signature);
rsa!(RsaPkcs1Sha384, alg_id::RSA_PKCS1_SHA384, rsa::pkcs1v15::VerifyingKey<Sha384>, rsa::pkcs1v15::Signature);
rsa!(RsaPkcs1Sha512, alg_id::RSA_PKCS1_SHA512, rsa::pkcs1v15::VerifyingKey<Sha512>, rsa::pkcs1v15::Signature);
rsa!(RsaPssSha256, alg_id::RSA_PSS_SHA256, rsa::pss::VerifyingKey<Sha256>, rsa::pss::Signature);
rsa!(RsaPssSha384, alg_id::RSA_PSS_SHA384, rsa::pss::VerifyingKey<Sha384>, rsa::pss::Signature);
rsa!(RsaPssSha512, alg_id::RSA_PSS_SHA512, rsa::pss::VerifyingKey<Sha512>, rsa::pss::Signature);

#[derive(Debug)]
struct Ed25519;
impl SignatureVerificationAlgorithm for Ed25519 {
    fn public_key_alg_id(&self) -> AlgorithmIdentifier { alg_id::ED25519 }
    fn signature_alg_id(&self) -> AlgorithmIdentifier { alg_id::ED25519 }
    fn verify_signature(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<(), InvalidSignature> {
        let key: &[u8; 32] = public_key.try_into().map_err(|_| InvalidSignature)?;
        let signature = ed25519_dalek::Signature::from_slice(signature).map_err(|_| InvalidSignature)?;
        ed25519_dalek::VerifyingKey::from_bytes(key).map_err(|_| InvalidSignature)?.verify_strict(message, &signature).map_err(|_| InvalidSignature)
    }
}

static ALGORITHMS: WebPkiSupportedAlgorithms = WebPkiSupportedAlgorithms {
    all: &[&EcdsaP256Sha256, &EcdsaP256Sha384, &EcdsaP384Sha256, &EcdsaP384Sha384, &Ed25519, &RsaPkcs1Sha256, &RsaPkcs1Sha384, &RsaPkcs1Sha512, &RsaPssSha256, &RsaPssSha384, &RsaPssSha512],
    mapping: &[
        (SignatureScheme::ECDSA_NISTP384_SHA384, &[&EcdsaP384Sha384, &EcdsaP256Sha384]),
        (SignatureScheme::ECDSA_NISTP256_SHA256, &[&EcdsaP256Sha256, &EcdsaP384Sha256]),
        (SignatureScheme::ED25519, &[&Ed25519]),
        (SignatureScheme::RSA_PSS_SHA512, &[&RsaPssSha512]),
        (SignatureScheme::RSA_PSS_SHA384, &[&RsaPssSha384]),
        (SignatureScheme::RSA_PSS_SHA256, &[&RsaPssSha256]),
        (SignatureScheme::RSA_PKCS1_SHA512, &[&RsaPkcs1Sha512]),
        (SignatureScheme::RSA_PKCS1_SHA384, &[&RsaPkcs1Sha384]),
        (SignatureScheme::RSA_PKCS1_SHA256, &[&RsaPkcs1Sha256]),
    ],
};
