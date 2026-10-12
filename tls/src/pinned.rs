// A server known by a pin instead of the root store. connect-pinned (idl/tls.wit 1.1, 351-NET-0002): the SHA-256 of
// its certificate's SubjectPublicKeyInfo, nothing else checked but the handshake's signature. connect-pins (1.2,
// 750-NET-0020): one of two pins, of the leaf (its DER) or of its SPKI, and the certificate still checked as webpki
// checks one (name, validity, purpose, CA:false, issuer's signature).
use alloc::sync::Arc;
use alloc::vec::Vec;
use pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::{CertificateError, DigitallySignedStruct, Error, SignatureScheme};
use sha2::Digest;
use x509_cert::der::asn1::ObjectIdentifier;

// RFC 5280: basic constraints, extended key usage, and its server-authentication purpose.
const BASIC_CONSTRAINTS: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.19");
const EXTENDED_KEY_USAGE: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.37");
const SERVER_AUTH: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.3.6.1.5.5.7.3.1");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Leaf, Spki }

#[derive(Debug)]
pub struct Pinned { pub pins: [(Kind, [u8; 32]); 2], pub count: usize, pub checked: bool, pub provider: Arc<CryptoProvider> }

impl Pinned {
    /// connect-pinned's one SPKI pin, the certificate otherwise unchecked (1.1).
    pub fn key(pin: [u8; 32], provider: Arc<CryptoProvider>) -> Self { Pinned { pins: [(Kind::Spki, pin); 2], count: 1, checked: false, provider } }
}

/// The SHA-256 of a certificate's SubjectPublicKeyInfo (its DER, with the outer SEQUENCE).
pub fn spki_sha256(certificate: &CertificateDer<'_>) -> Option<[u8; 32]> {
    let certificate = webpki::EndEntityCert::try_from(certificate).ok()?;
    Some(sha2::Sha256::digest(certificate.subject_public_key_info().as_ref()).into())
}

impl Pinned {
    // The certificate's name (webpki), its validity period by the clock, the server-authentication purpose when it names
    // purposes, and CA:false. Its issuer is not looked for: the pin is what is trusted, and a server may send its leaf
    // alone.
    fn check(&self, end_entity: &CertificateDer<'_>, name: &ServerName<'_>, now: UnixTime) -> Result<(), Error> {
        use x509_cert::der::Decode;
        use x509_cert::ext::pkix::{BasicConstraints, ExtendedKeyUsage};
        let bad = |why: CertificateError| Error::InvalidCertificate(why);
        let certificate = x509_cert::Certificate::from_der(end_entity.as_ref()).map_err(|_| bad(CertificateError::BadEncoding))?;
        let validity = &certificate.tbs_certificate.validity;
        let now = now.as_secs();
        if now < validity.not_before.to_unix_duration().as_secs() { return Err(bad(CertificateError::NotValidYet)); }
        if now > validity.not_after.to_unix_duration().as_secs() { return Err(bad(CertificateError::Expired)); }
        for extension in certificate.tbs_certificate.extensions.iter().flatten() {
            let value = extension.extn_value.as_bytes();
            if extension.extn_id == BASIC_CONSTRAINTS {
                let constraints = BasicConstraints::from_der(value).map_err(|_| bad(CertificateError::BadEncoding))?;
                if constraints.ca { return Err(bad(CertificateError::InvalidPurpose)); }
            } else if extension.extn_id == EXTENDED_KEY_USAGE {
                let purposes = ExtendedKeyUsage::from_der(value).map_err(|_| bad(CertificateError::BadEncoding))?;
                if !purposes.0.contains(&SERVER_AUTH) { return Err(bad(CertificateError::InvalidPurpose)); }
            }
        }
        let leaf = webpki::EndEntityCert::try_from(end_entity).map_err(|_| bad(CertificateError::BadEncoding))?;
        leaf.verify_is_valid_for_subject_name(name).map_err(|_| bad(CertificateError::NotValidForName))
    }
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _intermediates: &[CertificateDer<'_>], name: &ServerName<'_>, _ocsp: &[u8], now: UnixTime) -> Result<ServerCertVerified, Error> {
        let spki = spki_sha256(end_entity).ok_or(Error::InvalidCertificate(CertificateError::BadEncoding))?;
        let leaf: [u8; 32] = sha2::Sha256::digest(end_entity.as_ref()).into();
        let matched = self.pins[..self.count].iter().any(|(kind, pin)| *pin == if *kind == Kind::Leaf { leaf } else { spki });
        if !matched { return Err(Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)); }
        if self.checked { self.check(end_entity, name, now)?; }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(&self, message: &[u8], certificate: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, certificate, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], certificate: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, certificate, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> { self.provider.signature_verification_algorithms.supported_schemes() }
}
