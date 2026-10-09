// A server known by its key alone (idl/tls.wit 1.1, 351-NET-0002): the SHA-256 of its certificate's
// SubjectPublicKeyInfo must be the pin. No root, name or validity period is checked; the handshake's signature is.
use alloc::sync::Arc;
use alloc::vec::Vec;
use pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::{CertificateError, DigitallySignedStruct, Error, SignatureScheme};
use sha2::Digest;

#[derive(Debug)]
pub struct Pinned { pub pin: [u8; 32], pub provider: Arc<CryptoProvider> }

/// The SHA-256 of a certificate's SubjectPublicKeyInfo (its DER, with the outer SEQUENCE).
pub fn spki_sha256(certificate: &CertificateDer<'_>) -> Option<[u8; 32]> {
    let certificate = webpki::EndEntityCert::try_from(certificate).ok()?;
    Some(sha2::Sha256::digest(certificate.subject_public_key_info().as_ref()).into())
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _intermediates: &[CertificateDer<'_>], _name: &ServerName<'_>, _ocsp: &[u8], _now: UnixTime) -> Result<ServerCertVerified, Error> {
        match spki_sha256(end_entity) {
            Some(digest) if digest == self.pin => Ok(ServerCertVerified::assertion()),
            Some(_) => Err(Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)),
            None => Err(Error::InvalidCertificate(CertificateError::BadEncoding)),
        }
    }

    fn verify_tls12_signature(&self, message: &[u8], certificate: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, certificate, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], certificate: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, certificate, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> { self.provider.signature_verification_algorithms.supported_schemes() }
}
