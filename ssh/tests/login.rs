//! What the key service signs for an SSH login (351-NET-0027): `ssh::login::fits` takes exactly the data a public-key
//! login with the device key signs, and nothing else.
use ssh::login::{fits, signed_data, user_fits, USER_MAX};
use ssh::wire::Writer;

const KEY: [u8; 32] = [7; 32];

fn data(session: &[u8], kind: u8, user: &[u8], service: &[u8], method: &[u8], signed: u8, algorithm: &[u8], key: &[u8; 32]) -> Vec<u8> {
    let mut blob = Writer::default();
    blob.string(algorithm).string(key);
    let mut w = Writer::default();
    w.string(session).u8(kind).string(user).string(service).string(method).u8(signed).string(algorithm).string(&blob.data);
    w.data
}

#[test]
fn a_login_with_the_key_fits() {
    let good = signed_data(&[1; 32], b"release", &KEY);
    assert!(fits(&good, &KEY));
    assert_eq!(good, data(&[1; 32], 50, b"release", b"ssh-connection", b"publickey", 1, b"ssh-ed25519", &KEY));
    assert!(fits(&signed_data(&[1; 32], &[b'u'; USER_MAX], &KEY), &KEY));
}

#[test]
fn anything_else_is_refused() {
    let id = [1u8; 32];
    for other in [
        data(&[1; 20], 50, b"u", b"ssh-connection", b"publickey", 1, b"ssh-ed25519", &KEY),   // a SHA-1 exchange's identifier
        data(&id, 51, b"u", b"ssh-connection", b"publickey", 1, b"ssh-ed25519", &KEY),        // another message
        data(&id, 50, b"", b"ssh-connection", b"publickey", 1, b"ssh-ed25519", &KEY),         // no user
        data(&id, 50, b"a user", b"ssh-connection", b"publickey", 1, b"ssh-ed25519", &KEY),   // a space
        data(&id, 50, &[b'u'; USER_MAX + 1], b"ssh-connection", b"publickey", 1, b"ssh-ed25519", &KEY),
        data(&id, 50, b"u", b"ssh-userauth", b"publickey", 1, b"ssh-ed25519", &KEY),          // another service
        data(&id, 50, b"u", b"ssh-connection", b"hostbased", 1, b"ssh-ed25519", &KEY),        // another method
        data(&id, 50, b"u", b"ssh-connection", b"publickey", 0, b"ssh-ed25519", &KEY),        // a query, not a login
        data(&id, 50, b"u", b"ssh-connection", b"publickey", 1, b"ssh-rsa", &KEY),            // another algorithm
        data(&id, 50, b"u", b"ssh-connection", b"publickey", 1, b"ssh-ed25519", &[8; 32]),    // another key
    ] {
        assert!(!fits(&other, &KEY));
    }
    let mut longer = signed_data(&id, b"u", &KEY);
    longer.push(0);
    assert!(!fits(&longer, &KEY));
    let good = signed_data(&id, b"u", &KEY);
    assert!(!fits(&good[..good.len() - 1], &KEY));
    // A TLS 1.3 CertificateVerify's form is not a login.
    let mut tls = vec![b' '; 64];
    tls.extend_from_slice(b"TLS 1.3, client CertificateVerify\0");
    tls.extend_from_slice(&[9; 32]);
    assert!(!fits(&tls, &KEY));
    assert!(!user_fits(b"tab\there"));
}
