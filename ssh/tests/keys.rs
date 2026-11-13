//! The owner's list of login keys (112-NET-0072): lines with their rights taken, every other line left out with why;
//! base64 as OpenSSH writes it, fingerprints as OpenSSH prints them (checked against ssh-keygen when it is there).
#![cfg(feature = "server")]
use ssh::keys::{self, Listed};
use ssh::server::Rights;

const KEY: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIGlvSK6PNn5BaQvv5q7lQ1xlrbtrf6SlB5qnQ4k9Q8G4";

fn blob_key() -> [u8; 32] { keys::unbase64(KEY.as_bytes()).unwrap()[19..].try_into().unwrap() }

#[test]
fn lines_taken_and_left_out() {
    let text = format!("# the owner's keys\n\nconsole,files ssh-ed25519 {KEY} owner@laptop\nfiles ssh-ed25519 {KEY}\n  console  ssh-ed25519  {KEY}  two words \n\
        ssh-ed25519 {KEY} no rights\nfiles,root ssh-ed25519 {KEY}\nfiles ssh-rsa {KEY}\nfiles ssh-ed25519 AAAA*\nfiles ssh-ed25519 {}\nfiles\n",
        &KEY[..KEY.len() - 4]);
    let (listed, left) = keys::parse(&text);
    let both = Rights { console: true, files: true };
    assert_eq!(listed, vec![
        Listed { key: blob_key(), rights: both, comment: "owner@laptop".into() },
        Listed { key: blob_key(), rights: Rights { console: false, files: true }, comment: String::new() },
        Listed { key: blob_key(), rights: Rights { console: true, files: false }, comment: "two words".into() },
    ]);
    let numbers: Vec<usize> = left.iter().map(|&(n, _)| n).collect();
    assert_eq!(numbers, vec![6, 7, 8, 9, 10, 11], "{left:?}");
    assert!(left[0].1.contains("no rights") && left[1].1.contains("a word other than") && left[2].1.contains("other than ssh-ed25519"), "{left:?}");
    assert!(left[3].1.contains("base64") && left[4].1.contains("not an ssh-ed25519 key") && left[5].1 == "no key", "{left:?}");
}

#[test]
fn bounds() {
    let one = format!("files ssh-ed25519 {KEY}\n");
    let (listed, left) = keys::parse(&one.repeat(70));
    assert_eq!((listed.len(), left.len()), (64, 6));
    assert!(left.iter().all(|&(_, why)| why == "more than 64 keys"));
    let long = format!("files ssh-ed25519 {KEY} {}", "x".repeat(1100));
    assert_eq!(keys::line(&long), Some(Err("a line longer than 1024 bytes")));
    // A blob with bytes after the key, and base64 whose last bits are not zero, are not keys.
    let mut blob = keys::unbase64(KEY.as_bytes()).unwrap();
    blob.push(0);
    let encoded = { let mut s = keys::base64_unpadded(&blob); while s.len() % 4 != 0 { s.push('='); } s };
    assert_eq!(keys::line(&format!("files ssh-ed25519 {encoded}")), Some(Err("a key blob that is not an ssh-ed25519 key")));
    assert_eq!(keys::unbase64(b"AB=="), None);
    assert_eq!(keys::unbase64(b"AA=="), Some(vec![0]));
}

#[test]
fn base64_and_fingerprints_as_openssh_writes_them() {
    for n in 0..40u8 {
        let data: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37).wrapping_add(n)).collect();
        let mut text = keys::base64_unpadded(&data);
        while text.len() % 4 != 0 { text.push('='); }
        assert_eq!(keys::unbase64(text.as_bytes()), Some(data));
    }
    let blob = keys::unbase64(KEY.as_bytes()).unwrap();
    let sha: [u8; 32] = sha2::Digest::finalize(sha2::Digest::chain_update(sha2::Sha256::default(), &blob)).into();
    let ours = keys::fingerprint(&sha);
    assert!(ours.starts_with("SHA256:") && ours.len() == 7 + 43, "{ours}");
    let dir = std::env::temp_dir().join(format!("mind-keys-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("key.pub");
    std::fs::write(&file, format!("ssh-ed25519 {KEY} test\n")).unwrap();
    if let Ok(out) = std::process::Command::new("ssh-keygen").args(["-l", "-E", "sha256", "-f"]).arg(&file).output() {
        let printed = String::from_utf8_lossy(&out.stdout);
        assert!(printed.contains(&ours), "ssh-keygen printed {printed:?}, ours {ours}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_keys_directory_as_fat_names_it() {
    for path in ["/ssh", "/ssh/authorized_keys", "/SSH/AUTHORIZED_KEYS", "/Ssh./x", "/ssh ./x", "ssh", "/ssh. . /x"] { assert!(keys::under_dir(path), "{path}"); }
    for path in ["/", "/sshd", "/x/ssh", "/ssh2/authorized_keys", "/.ssh", "/s sh"] { assert!(!keys::under_dir(path), "{path}"); }
}
