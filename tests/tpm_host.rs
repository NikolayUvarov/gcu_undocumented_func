//! Host tests of the TPM 2.0 commands (libmind/src/tpm.rs, 351-DRV-0015): commands byte for byte against the TPM 2.0
//! Library's encodings, and the responses the service reads, built here as a TPM would send them.
#[path = "../libmind/src/tpm.rs"]
mod tpm;

use tpm::Error;

fn response(code: u32, body: &[u8]) -> Vec<u8> {
    let mut r = vec![0x80, 0x02];
    r.extend_from_slice(&((10 + body.len()) as u32).to_be_bytes());
    r.extend_from_slice(&code.to_be_bytes());
    r.extend_from_slice(body);
    r
}

#[test]
fn commands_as_the_tpm_reads_them() {
    assert_eq!(tpm::startup().unwrap().as_bytes(), [0x80, 0x01, 0, 0, 0, 12, 0, 0, 0x01, 0x44, 0, 0]);
    assert_eq!(tpm::flush(0x8000_0000).unwrap().as_bytes(), [0x80, 0x01, 0, 0, 0, 14, 0, 0, 0x01, 0x65, 0x80, 0, 0, 0]);
    // The password session: handle TPM_RS_PW, an empty nonce, no attributes, an empty password.
    assert_eq!(tpm::unseal(0x8000_0001).unwrap().as_bytes(),
               [0x80, 0x02, 0, 0, 0, 27, 0, 0, 0x01, 0x5E, 0x80, 0, 0, 1, 0, 0, 0, 9, 0x40, 0, 0, 9, 0, 0, 0, 0, 0]);
    let primary = tpm::create_primary().unwrap();
    let p = primary.as_bytes();
    assert_eq!(p.len(), 67);
    assert_eq!(&p[..14], [0x80, 0x02, 0, 0, 0, 67, 0, 0, 0x01, 0x31, 0x40, 0, 0, 1]);
    // inSensitive, then a 26-byte ECC template: P-256, AES-128-CFB, restricted decryption.
    assert_eq!(&p[27..33], [0, 4, 0, 0, 0, 0]);
    assert_eq!(&p[33..35], [0, 26]);
    assert_eq!(&p[35..39], [0, 0x23, 0, 0x0B]);
    assert_eq!(&p[39..43], 0x0003_0472u32.to_be_bytes());
    assert_eq!(&p[45..51], [0, 6, 0, 128, 0, 0x43]);
    assert_eq!(&p[51..57], [0, 0x10, 0, 3, 0, 0x10]);
    let sealed = tpm::create_sealed(0x8000_0000, b"secret").unwrap();
    let s = sealed.as_bytes();
    assert_eq!(&s[8..10], [0x01, 0x53]);
    // inSensitive holds the secret; the template is a keyed-hash object without sensitiveDataOrigin.
    assert_eq!(&s[27..39], [0, 10, 0, 0, 0, 6, b's', b'e', b'c', b'r', b'e', b't']);
    assert_eq!(&s[39..53], [0, 14, 0, 0x08, 0, 0x0B, 0, 0, 0x04, 0x52, 0, 0, 0, 0x10]);
    assert_eq!(s.len(), 10 + 4 + 13 + 12 + 16 + 2 + 4);
    // Load takes the blob's two parts as they are; GetCapability asks for one property, the manufacturer.
    let load = tpm::load(0x8000_0000, &[0, 1, 7], &[0, 2, 8, 9]).unwrap();
    assert_eq!(&load.as_bytes()[8..14], [0x01, 0x57, 0x80, 0, 0, 0]);
    assert_eq!(&load.as_bytes()[27..], [0, 1, 7, 0, 2, 8, 9]);
    assert_eq!(tpm::manufacturer().unwrap().as_bytes(), [0x80, 0x01, 0, 0, 0, 22, 0, 0, 0x01, 0x7A, 0, 0, 0, 6, 0, 0, 1, 5, 0, 0, 0, 1]);
    assert_eq!(tpm::RC_INITIALIZE, 0x100);
}

#[test]
fn secrets_are_bounded() {
    assert_eq!(tpm::create_sealed(1, &[]).err(), Some(Error::TooLarge));
    assert_eq!(tpm::create_sealed(1, &[7; tpm::SECRET_MAX + 1]).err(), Some(Error::TooLarge));
    assert!(tpm::create_sealed(1, &[7; tpm::SECRET_MAX]).is_ok());
}

#[test]
fn responses_read() {
    // CreatePrimary and Load: the handle first.
    assert_eq!(tpm::handle(&response(0, &[0x80, 0, 0, 2, 0, 0, 0, 0])), Ok(0x8000_0002));
    assert_eq!(tpm::handle(&response(0x101, &[])), Err(Error::Tpm(0x101)));
    // Create: parameterSize, TPM2B_PRIVATE, TPM2B_PUBLIC, then what the service does not keep.
    let mut body = vec![0, 0, 0, 20, 0, 3, 1, 2, 3, 0, 2, 9, 9, 0, 0];
    body.extend_from_slice(&[0; 5]);
    let mut blob = [0u8; 64];
    let n = tpm::parse_created(&response(0, &body), &mut blob).unwrap();
    assert_eq!(&blob[..n], [0, 3, 1, 2, 3, 0, 2, 9, 9]);
    assert_eq!(tpm::split_blob(&blob[..n]), Ok((&[0u8, 3, 1, 2, 3][..], &[0u8, 2, 9, 9][..])));
    assert_eq!(tpm::split_blob(&blob[..n - 1]), Err(Error::Malformed));
    assert_eq!(tpm::split_blob(&[0, 9, 1]), Err(Error::Malformed));
    // Unseal: parameterSize, TPM2B_SENSITIVE_DATA.
    let mut secret = [0u8; 16];
    let n = tpm::parse_unsealed(&response(0, &[0, 0, 0, 8, 0, 6, b's', b'e', b'c', b'r', b'e', b't']), &mut secret).unwrap();
    assert_eq!(&secret[..n], b"secret");
    assert_eq!(tpm::parse_unsealed(&response(0, &[0, 0, 0, 8, 0, 60, 1]), &mut secret), Err(Error::Malformed));
    // GetCapability: "IBM " for swtpm.
    let caps = [0, 0, 0, 0, 6, 0, 0, 0, 1, 0, 0, 1, 5, b'I', b'B', b'M', b' '];
    assert_eq!(tpm::parse_manufacturer(&response(0, &caps)), Ok(u32::from_be_bytes(*b"IBM ")));
}

#[test]
fn headers_are_checked() {
    assert_eq!(tpm::handle(&[0x80, 0x01, 0, 0]).err(), Some(Error::Malformed));
    // A size beyond the bytes given, or below a header's.
    assert_eq!(tpm::handle(&[0x80, 0x01, 0, 0, 0, 40, 0, 0, 0, 0]).err(), Some(Error::Malformed));
    assert_eq!(tpm::handle(&[0x80, 0x01, 0, 0, 0, 4, 0, 0, 0, 0]).err(), Some(Error::Malformed));
    assert_eq!(tpm::response_size(&[0x80, 0x01, 0, 0, 0, 12]), Some(12));
    assert_eq!(tpm::response_size(&[0x80, 0x01, 0, 0, 0xFF, 0]), None);
}
