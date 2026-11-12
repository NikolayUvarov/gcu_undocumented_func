//! What the key service signs for TLS (keystore.wit 1.7, 110-NET-0069): a CertificateVerify's content of its own side
//! alone, with a SHA-256 or SHA-384 hash, nothing more or less.
#[path = "../keystore/src/form.rs"]
mod form;
use form::{certificate_verify, CLIENT, SERVER};

fn content(context: &[u8], hash: usize) -> Vec<u8> { [vec![b' '; 64], context.to_vec(), vec![0xA5; hash]].concat() }

#[test]
fn each_side_signs_its_own_content() {
    for hash in [32, 48] {
        assert!(certificate_verify(&content(CLIENT, hash), CLIENT));
        assert!(certificate_verify(&content(SERVER, hash), SERVER));
        // A client's content is no server's, and the other way round.
        assert!(!certificate_verify(&content(CLIENT, hash), SERVER));
        assert!(!certificate_verify(&content(SERVER, hash), CLIENT));
    }
}

#[test]
fn anything_else_is_refused() {
    for hash in [0, 20, 31, 33, 47, 49, 64] { assert!(!certificate_verify(&content(SERVER, hash), SERVER), "hash of {hash}"); }
    let mut prefix = content(SERVER, 32);
    prefix[10] = b'x';
    assert!(!certificate_verify(&prefix, SERVER));
    // Without the zero byte after the context string the hash would start one byte early.
    let unended = [vec![b' '; 64], SERVER[..SERVER.len() - 1].to_vec(), vec![0xA5; 33]].concat();
    assert!(!certificate_verify(&unended, SERVER));
    for short in [&b""[..], &[b' '; 64][..], &content(SERVER, 0)[..10]] { assert!(!certificate_verify(short, SERVER)); }
}
