//! The client's checks of a certificate chain and a TLS 1.3 handshake signature over the RustCrypto crates, for a hub
//! known by a CA bundle (`--ca`, 100003-APP-0160): ECDSA P-256 and P-384, Ed25519, RSA PKCS#1 v1.5 and PSS. Adapted
//! from rustls-rustcrypto 0.0.2-alpha (MIT OR Apache-2.0, THIRD_PARTY.md), as `tls/src/provider.rs` is; `httpd`'s
//! provider gives everything else and verifies nothing, being a server's.

use pki_types::{alg_id, AlgorithmIdentifier, InvalidSignature, SignatureVerificationAlgorithm};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::SignatureScheme;
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::sync::Arc;

/// `httpd`'s provider with these checks: a client's, for a chain checked against roots.
pub fn provider() -> Arc<CryptoProvider> {
    let mut p = (*httpd::tls::provider(false)).clone();
    p.signature_verification_algorithms = ALGORITHMS;
    Arc::new(p)
}

macro_rules! ecdsa {
    ($name:ident, $curve:expr, $signature:expr, $key:ty, $der:ty, $hash:ty) => {
        #[derive(Debug)]
        struct $name;
        impl SignatureVerificationAlgorithm for $name {
            fn public_key_alg_id(&self) -> AlgorithmIdentifier { $curve }
            fn signature_alg_id(&self) -> AlgorithmIdentifier { $signature }
            fn verify_signature(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<(), InvalidSignature> {
                use p256::ecdsa::signature::hazmat::PrehashVerifier;
                let signature = <$der>::from_bytes(signature).map_err(|_| InvalidSignature)?;
                let key = <$key>::from_sec1_bytes(public_key).map_err(|_| InvalidSignature)?;
                key.verify_prehash(&<$hash>::digest(message), &signature).map_err(|_| InvalidSignature)
            }
        }
    };
}

ecdsa!(EcdsaP256Sha256, alg_id::ECDSA_P256, alg_id::ECDSA_SHA256, p256::ecdsa::VerifyingKey, p256::ecdsa::DerSignature, Sha256);
ecdsa!(EcdsaP256Sha384, alg_id::ECDSA_P256, alg_id::ECDSA_SHA384, p256::ecdsa::VerifyingKey, p256::ecdsa::DerSignature, Sha384);
ecdsa!(EcdsaP384Sha256, alg_id::ECDSA_P384, alg_id::ECDSA_SHA256, p384::ecdsa::VerifyingKey, p384::ecdsa::DerSignature, Sha256);
ecdsa!(EcdsaP384Sha384, alg_id::ECDSA_P384, alg_id::ECDSA_SHA384, p384::ecdsa::VerifyingKey, p384::ecdsa::DerSignature, Sha384);

macro_rules! rsa {
    ($name:ident, $signature_id:expr, $verifier:ty, $signature:ty) => {
        #[derive(Debug)]
        struct $name;
        impl SignatureVerificationAlgorithm for $name {
            fn public_key_alg_id(&self) -> AlgorithmIdentifier { alg_id::RSA_ENCRYPTION }
            fn signature_alg_id(&self) -> AlgorithmIdentifier { $signature_id }
            fn verify_signature(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<(), InvalidSignature> {
                use rsa::pkcs1::DecodeRsaPublicKey;
                use rsa::signature::Verifier;
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
