// The content of a TLS 1.3 CertificateVerify the device key signs (RFC 8446 4.4.3): 64 spaces, the side's context
// string, a zero byte and a SHA-256 or SHA-384 transcript hash, nothing else.

/// The client's context string and its zero byte (`tls-client`).
pub const CLIENT: &[u8] = b"TLS 1.3, client CertificateVerify\0";
/// The server's (`tls-server`, keystore.wit 1.7).
pub const SERVER: &[u8] = b"TLS 1.3, server CertificateVerify\0";

/// Whether `data` is a CertificateVerify's signed content with `context`.
pub fn certificate_verify(data: &[u8], context: &[u8]) -> bool {
    let hash = data.len().wrapping_sub(64 + context.len());
    (hash == 32 || hash == 48) && data[..64].iter().all(|&b| b == b' ') && &data[64..64 + context.len()] == context
}
