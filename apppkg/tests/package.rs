//! Host tests of the signed application package check (750-UPD-0019, contract draft 0.1): a package that
//! scripts/app_package.py signed with the test key (tests/fixture) passes; every refusal of the draft is met.
use apppkg::{key_id, parse, verify, Arch, Error, Installed, Trusted, FILES_MAX, MANIFEST_MAX, PREFIX};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

const FIXTURE: &[u8] = include_bytes!("fixture/MANIFEST");
const FIXTURE_SIG: &[u8; 64] = include_bytes!("fixture/MANIFEST.SIG");
const HELLO: &[u8] = include_bytes!("fixture/bin/hello");
const GREETING: &[u8] = include_bytes!("fixture/data/greeting.txt");

// The test key of scripts/app_package.py: public text, never trusted on a device.
fn test_key() -> SigningKey { SigningKey::from_bytes(&Sha256::digest(b"MIND Core test application key: public, never for a real package").into()) }
fn other_key() -> SigningKey { SigningKey::from_bytes(&[7; 32]) }
fn trust(key: &SigningKey, name: &'static str) -> Trusted<'static> { Trusted { key: key.verifying_key().to_bytes(), name } }

fn manifest(key: &SigningKey, name: &str, version: u64, arch: &str, system: u64, files: &[&str]) -> Vec<u8> {
    let id: String = key_id(&key.verifying_key().to_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    let mut text = format!("MIND-APP-MANIFEST 1\nkey {id}\nname {name}\nversion {version}\narch {arch}\nsystem {system}\n");
    for file in files { text.push_str(file); text.push('\n'); }
    text.into_bytes()
}

fn line(path: &str, data: &[u8]) -> String {
    let hash: String = Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect();
    format!("file {path} {} {hash} 00000000 0", data.len())
}

fn signed(key: &SigningKey, manifest: &[u8]) -> [u8; 64] { key.sign(&[PREFIX, manifest].concat()).to_bytes() }

fn package(version: u64) -> (Vec<u8>, [u8; 64]) {
    let key = test_key();
    let text = manifest(&key, "hello", version, "x86_64", 7, &[&line("bin/hello", HELLO), &line("data/greeting.txt", GREETING)]);
    let signature = signed(&key, &text);
    (text, signature)
}

#[test]
fn the_tools_package_passes_and_its_files_match() {
    let checked = verify(FIXTURE, FIXTURE_SIG, &[trust(&test_key(), "hello")], Arch::X86_64, 7, None, false).expect("the tool's package");
    assert_eq!((checked.name, checked.version, checked.arch, checked.system, checked.count()), ("hello", 3, Arch::X86_64, 7, 2));
    let files: Vec<_> = checked.files().collect();
    assert_eq!(files.iter().map(|f| f.path).collect::<Vec<_>>(), ["bin/hello", "data/greeting.txt"]);
    assert!(files[0].matches(HELLO) && files[1].matches(GREETING));
    // The same bytes as the tool's: one encoding.
    assert_eq!(package(3).0, FIXTURE);
    // A file that changed, or is cut short, does not match.
    let mut changed = GREETING.to_vec();
    changed[0] ^= 1;
    assert!(!files[1].matches(&changed) && !files[1].matches(&GREETING[..GREETING.len() - 1]));
}

#[test]
fn a_changed_manifest_or_a_signature_without_the_prefix_is_refused() {
    let key = test_key();
    let (text, signature) = package(3);
    let mut changed = text.clone();
    let at = changed.windows(9).position(|w| w == b"version 3").unwrap() + 8;
    changed[at] = b'4';
    assert_eq!(verify(&changed, &signature, &[trust(&key, "hello")], Arch::X86_64, 7, None, false).unwrap_err(), Error::Signature);
    // A signature over the manifest alone, as a boot manifest's is, does not pass as an application's.
    let bare = key.sign(&text).to_bytes();
    assert_eq!(verify(&text, &bare, &[trust(&key, "hello")], Arch::X86_64, 7, None, false).unwrap_err(), Error::Signature);
}

#[test]
fn a_key_is_trusted_only_for_its_names() {
    let (text, signature) = package(3);
    let trusted = |list: &[Trusted]| verify(&text, &signature, list, Arch::X86_64, 7, None, false).map(|m| m.version);
    assert_eq!(trusted(&[trust(&other_key(), "hello")]), Err(Error::Untrusted));
    assert_eq!(trusted(&[trust(&test_key(), "other")]), Err(Error::Untrusted));
    assert_eq!(trusted(&[]), Err(Error::Untrusted));
    assert_eq!(trusted(&[trust(&test_key(), "*")]), Ok(3));
    // A key with the right identity in another key's manifest: the other key signed it.
    let forged = manifest(&test_key(), "hello", 3, "x86_64", 7, &[&line("a", b"x")]);
    let by_other = signed(&other_key(), &forged);
    assert_eq!(verify(&forged, &by_other, &[trust(&test_key(), "hello")], Arch::X86_64, 7, None, false).unwrap_err(), Error::Signature);
}

#[test]
fn architecture_release_and_version_are_checked() {
    let (text, signature) = package(3);
    let trusted = [trust(&test_key(), "hello")];
    let check = |arch, system, installed, downgrade| verify(&text, &signature, &trusted, arch, system, installed, downgrade).map(|m| m.version);
    assert_eq!(check(Arch::Aarch64, 7, None, false), Err(Error::Arch));
    assert_eq!(check(Arch::X86_64, 6, None, false), Err(Error::System));
    assert_eq!(check(Arch::X86_64, 8, None, false), Ok(3));
    let installed = |version, manifest: &[u8]| Some(Installed { version, manifest: Sha256::digest(manifest).into() });
    let (newer, _) = package(4);
    assert_eq!(check(Arch::X86_64, 7, installed(4, &newer), false), Err(Error::Older), "a lower version");
    assert_eq!(check(Arch::X86_64, 7, installed(4, &newer), true), Ok(3), "the owner allows a downgrade of this name");
    assert_eq!(check(Arch::X86_64, 7, installed(3, &text), false), Ok(3), "the same package again");
    assert_eq!(check(Arch::X86_64, 7, installed(3, b"other bytes"), false), Err(Error::Older), "the same version, other bytes");
    assert_eq!(check(Arch::X86_64, 7, installed(2, b"older"), false), Ok(3));
}

#[test]
fn only_the_one_encoding_parses() {
    let key = test_key();
    let ok = line("a", b"x");
    let malformed = |text: &[u8]| parse(text).map(|m| m.version).unwrap_err();
    let base = |name: &str, version: &str, arch: &str, system: &str, files: &[&str]| {
        let id: String = key_id(&key.verifying_key().to_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        let mut t = format!("MIND-APP-MANIFEST 1\nkey {id}\nname {name}\nversion {version}\narch {arch}\nsystem {system}\n");
        for f in files { t.push_str(f); t.push('\n'); }
        t.into_bytes()
    };
    assert!(parse(&base("hello", "3", "x86_64", "0", &[&ok])).is_ok());
    // The header: names, numbers and architectures of one form only.
    for (name, version, arch, system, line) in [("Hello", "3", "x86_64", "0", 3), ("-hello", "3", "x86_64", "0", 3), ("", "3", "x86_64", "0", 3),
                                                ("hello", "0", "x86_64", "0", 4), ("hello", "03", "x86_64", "0", 4), ("hello", "18446744073709551616", "x86_64", "0", 4),
                                                ("hello", "3", "x86", "0", 5), ("hello", "3", "x86_64", "-1", 6)] {
        assert_eq!(malformed(&base(name, version, arch, system, &[&ok])), Error::Malformed(line), "{name} {version} {arch} {system}");
    }
    assert_eq!(malformed(&base(&"a".repeat(33), "3", "x86_64", "0", &[&ok])), Error::Malformed(3));
    // Files: at least one, sorted, each once, a path of the allowed form, every field of one form.
    assert_eq!(malformed(&base("hello", "3", "x86_64", "0", &[])), Error::Malformed(7));
    assert_eq!(malformed(&base("hello", "3", "x86_64", "0", &[&line("b", b"x"), &line("a", b"x")])), Error::Malformed(8));
    assert_eq!(malformed(&base("hello", "3", "x86_64", "0", &[&ok, &ok])), Error::Malformed(8));
    for path in ["../a", "a/../b", "/a", "a/", "a//b", "./a", "A", "a b", "MANIFEST", &"a".repeat(129)] {
        assert_eq!(malformed(&base("hello", "3", "x86_64", "0", &[&line(path, b"x")])), Error::Malformed(7), "{path:?}");
    }
    let hash = "0".repeat(64);
    for file in [format!("file a 01 {hash} 00000000 0"), format!("file a 1 {} 00000000 0", "A".repeat(64)), format!("file a 1 {hash} 0000000 0"),
                 format!("file a 1 {hash} 00000000 0 extra"), format!("file a 1 {hash} 00000000"), format!("dir a 1 {hash} 00000000 0")] {
        assert_eq!(malformed(&base("hello", "3", "x86_64", "0", &[&file])), Error::Malformed(7), "{file}");
    }
    // Lines: printable ASCII ended by LF, the last one too; nothing unknown.
    let good = base("hello", "3", "x86_64", "0", &[&ok]);
    assert!(matches!(parse(&good[..good.len() - 1]), Err(Error::Malformed(_))));
    let mut crlf = good.clone();
    crlf.splice(19..19, *b"\r");
    assert!(matches!(parse(&crlf), Err(Error::Malformed(_))));
    let mut extra = good.clone();
    extra.splice(20..20, b"comment here\n".iter().copied());
    assert_eq!(malformed(&extra), Error::Malformed(2));
    assert_eq!(malformed(&good.iter().copied().skip(1).collect::<Vec<_>>()), Error::Malformed(1));
}

#[test]
fn bounds_hold() {
    let key = test_key();
    let files: Vec<String> = (0..=FILES_MAX).map(|i| line(&format!("f{i:04}"), b"x")).collect();
    let refs: Vec<&str> = files.iter().map(String::as_str).collect();
    assert_eq!(parse(&manifest(&key, "hello", 3, "x86_64", 0, &refs)).unwrap_err(), Error::TooLarge);
    assert_eq!(parse(&manifest(&key, "hello", 3, "x86_64", 0, &refs[..FILES_MAX])).unwrap().count(), FILES_MAX);
    let mut big = manifest(&key, "hello", 3, "x86_64", 0, &refs[..1]);
    big.resize(MANIFEST_MAX + 1, b'\n');
    assert_eq!(parse(&big).unwrap_err(), Error::TooLarge);
}
