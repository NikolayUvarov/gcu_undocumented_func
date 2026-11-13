//! Host tests of the release metadata (libmind/src/release.rs, 351-UPD-0013): channel files, key files (351-UPD-0026)
//! and boot manifests made by scripts/release.py and scripts/sign_manifest.py are read field by field and encode again
//! to exactly their bytes; every other form is refused, and any byte changed in a valid file is refused or reads as what
//! it then encodes. A key file's root signatures are counted as the updater counts them (`signed_by`), and a device takes
//! the next version only when the root before it and its own both signed it (`step`, 351-UPD-0027). A channel's timestamp
//! file (351-UPD-0028) is read and refused as a channel is.
#![allow(dead_code)]
#[path = "../libmind/src/release.rs"]
mod release;

use release::{channel_is, encode_channel, encode_file, encode_keys, encode_timestamp, expires_unix, keys_is, parse_channel, parse_keys, parse_timestamp, signed_by, step,
               timestamp_is, Malformed, Manifest, Rebuild, Step, CHANNEL_MAX, FILE_LINE_MAX, KEYS_MAX, ROLES, TIMESTAMP_MAX};
use std::process::Command;

// Output of a Python snippet run with scripts/ on its path.
fn python(code: &str) -> Vec<u8> {
    let out = Command::new("python3").args(["-c", &format!("import sys; sys.path.insert(0, 'scripts')\n{}", code)]).output().expect("python3");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

// A channel file as `release.py publish` writes it, signed with the test release key.
fn channel_file(name: &str, version: u64, minimum: u64, expires: &str, manifests: &str) -> Vec<u8> {
    python(&format!(
        "import release\nbody = release.channel_bytes({name:?}, {version}, {minimum}, {expires:?}, {manifests})\n\
         sys.stdout.buffer.write(body + release.signature_lines(body, [release.TEST_RELEASE_SEED]))"))
}

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{:02x}", b)).collect() }

fn stable() -> Vec<u8> { channel_file("stable", 7, 5, "2026-11-08T00:00:00Z", "{'x86_64': 'ab' * 32, 'aarch64': '0c' * 32}") }

#[test]
fn a_published_channel_reads_and_encodes_to_its_signed_line() {
    let file = stable();
    let (channel, signed) = parse_channel(&file).unwrap();
    assert_eq!((channel.name.as_str(), channel.version, channel.minimum, &channel.expires), ("stable", 7, 5, b"2026-11-08T00:00:00Z"));
    assert_eq!(channel.count, 2);
    assert_eq!(channel.manifest("aarch64"), Some([0x0c; 32]));
    assert_eq!(channel.manifest("x86_64"), Some([0xab; 32]));
    assert_eq!(channel.manifest("riscv64"), None);
    let mut body = [0u8; CHANNEL_MAX];
    let n = encode_channel(&channel, &mut body).unwrap();
    assert_eq!(&body[..n], &file[..signed.body]);
    let line = std::str::from_utf8(&file[signed.body..]).unwrap();
    assert_eq!(signed.count, 1);
    assert_eq!(line, format!("ed25519 {} {}\n", hex(&signed.signatures[0].key), hex(&signed.signatures[0].value)));
    assert_eq!(hex(&signed.signatures[0].key), python("import release, sign_manifest as sm\nprint(sm.public_key(release.TEST_RELEASE_SEED).hex(), end='')").iter().map(|&b| b as char).collect::<String>());
    // One architecture, and a large version.
    let one = channel_file("test-2", 18446744073709551615, 1, "2028-02-29T23:59:59Z", "{'x86_64': '00' * 32}");
    let (channel, signed) = parse_channel(&one).unwrap();
    assert_eq!((channel.version, channel.count), (u64::MAX, 1));
    let n = encode_channel(&channel, &mut body).unwrap();
    assert_eq!(&body[..n], &one[..signed.body]);
}

#[test]
fn other_forms_of_a_channel_are_refused() {
    let good = String::from_utf8(stable()).unwrap();
    let (body, signature) = good.split_once('\n').unwrap();
    let signature = signature.trim_end();
    let changed = |from: &str, to: &str| -> Vec<u8> { assert!(good.contains(from), "{}", from); good.replacen(from, to, 1).into_bytes() };
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("a space", changed("\"version\":7", "\"version\": 7")),
        ("keys out of order", format!("{}\n{}", body.replace("\"minimum\":5,\"version\":7", "\"version\":7,\"minimum\":5"), signature).into_bytes()),
        ("architectures out of order", changed("{\"aarch64\":", "{\"zz\":")),
        ("upper-case hex", changed(&"0c".repeat(32), &"0C".repeat(32))),
        ("a short digest", changed(&"0c".repeat(32), &"0c".repeat(31))),
        ("a leading zero", changed("\"version\":7", "\"version\":07")),
        ("minimum above the version", changed("\"minimum\":5", "\"minimum\":8")),
        ("minimum 0", changed("\"minimum\":5", "\"minimum\":0")),
        ("a negative version", changed("\"version\":7", "\"version\":-7")),
        ("a version past u64", changed("\"version\":7", "\"version\":18446744073709551616")),
        ("31 February", changed("2026-11-08", "2026-02-31")),
        ("29 February of 2026", changed("2026-11-08", "2026-02-29")),
        ("hour 24", changed("T00:00:00Z", "T24:00:00Z")),
        ("no Z", changed("00:00:00Z", "00:00:00+")),
        ("an escaped name", changed("\"stable\"", "\"st\\u0061ble\"")),
        ("an empty name", changed("\"stable\"", "\"\"")),
        ("another key", changed("\"channel\":", "\"chan\":")),
        ("bytes after the signature", format!("{}x", good).into_bytes()),
        ("no final newline", good.trim_end().as_bytes().to_vec()),
        ("CRLF", good.replace('\n', "\r\n").into_bytes()),
        ("a short signature", changed(&signature[..90], &signature[..88])),
        ("upper-case signature", format!("{}\ned25519 {}\n", body, signature[8..].to_uppercase()).into_bytes()),
        ("no signature line", format!("{}\n", body).into_bytes()),
        ("a signature of before 351-UPD-0026, naming no key", format!("{}\ned25519 {}\n", body, &signature[8 + 65..]).into_bytes()),
        ("two spaces in a signature line", changed(&signature[..72], &format!("{} ", &signature[..72]))),
        ("nine signature lines", format!("{}\n{}", body, format!("{}\n", signature).repeat(9)).into_bytes()),
    ];
    for (what, file) in cases {
        assert_eq!(parse_channel(&file).err(), Some(Malformed), "{}: {}", what, String::from_utf8_lossy(&file));
    }
    let five = channel_file("stable", 7, 5, "2026-11-08T00:00:00Z", "{'a': '00' * 32, 'b': '00' * 32, 'c': '00' * 32, 'd': '00' * 32, 'e': '00' * 32}");
    assert_eq!(parse_channel(&five).err(), Some(Malformed), "five architectures");
}

#[test]
fn a_changed_byte_is_refused_or_reads_as_what_it_encodes() {
    // One encoding: whatever the parser accepts encodes back to exactly the line it read.
    let file = stable();
    let mut body = [0u8; CHANNEL_MAX];
    let mut accepted = 0;
    for at in 0..file.len() {
        for value in [b'0', b'9', b'a', b'z', b'"', b',', b'\n', b' ', 0x80] {
            let mut changed = file.clone();
            if changed[at] == value { continue; }
            changed[at] = value;
            if let Ok((channel, signed)) = parse_channel(&changed) {
                accepted += 1;
                let n = encode_channel(&channel, &mut body).unwrap();
                assert_eq!(&body[..n], &changed[..signed.body], "at {}", at);
            }
        }
    }
    assert!(accepted > 0);
}

#[test]
fn expiry_in_seconds_since_1970() {
    for stamp in ["1970-01-01T00:00:00Z", "2000-02-29T12:34:56Z", "2026-11-08T00:00:00Z", "2100-03-01T23:59:59Z"] {
        let expected: u64 = String::from_utf8(python(&format!("import calendar, time\nprint(calendar.timegm(time.strptime({:?}, '%Y-%m-%dT%H:%M:%SZ')))", stamp))).unwrap().trim().parse().unwrap();
        assert_eq!(expires_unix(stamp.as_bytes().try_into().unwrap()), expected, "{}", stamp);
    }
}

// A signed manifest as the build writes it, of a volume with these files.
fn manifest_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let list: Vec<String> = files.iter().map(|(p, d)| format!("({:?}, bytes({:?}))", p, d)).collect();
    python(&format!(
        "import pathlib, tempfile, sign_manifest as sm\nv = pathlib.Path(tempfile.mkdtemp())\n\
         for p, d in [{}]:\n    (v / p).parent.mkdir(parents=True, exist_ok=True); (v / p).write_bytes(d)\n\
         sys.stdout.buffer.write(sm.sign_volume(v, sm.TEST_SEED, ('c' * 40, 'nightly-x', 'd' * 64)))", list.join(", ")))
}

// The manifest's text again, from what `Manifest` read: its header lines as they are, its files encoded.
fn encode_again(manifest: &Manifest) -> Vec<u8> {
    let mut out = Vec::new();
    for i in 0..manifest.headers() { out.extend_from_slice(manifest.header(i).unwrap()); out.push(b'\n'); }
    let mut line = [0u8; FILE_LINE_MAX];
    for i in 0..manifest.files() { let n = encode_file(&manifest.file(i).unwrap(), &mut line).unwrap(); out.extend_from_slice(&line[..n]); out.push(b'\n'); }
    out
}

#[test]
fn a_built_manifest_reads_and_encodes_to_its_text() {
    let elf = b"\x7fELF and more";
    let text = manifest_of(&[("kernel.elf", elf), ("EFI/BOOT/BOOTX64.EFI", b"MZ"), ("LICENSES/LICENSE-MIT", b""), ("rtc.elf", b"\x7fELF")]);
    let manifest = Manifest::parse(&text).unwrap();
    assert_eq!((manifest.headers(), manifest.files()), (5, 4));
    assert_eq!(manifest.header(0), Some(&b"MIND-MANIFEST 1"[..]));
    assert_eq!(manifest.header(2), Some(&b"commit cccccccccccccccccccccccccccccccccccccccc"[..]));
    let kernel = (0..manifest.files()).map(|i| manifest.file(i).unwrap()).find(|f| f.path.as_str() == "kernel.elf").unwrap();
    assert_eq!(kernel.size, elf.len() as u64);
    assert_eq!(manifest.file(4), None);
    assert_eq!(encode_again(&manifest), text);
    // The build's own manifests, where they are built.
    for volume in ["usb_root", "aarch64_root"] {
        let Ok(text) = std::fs::read(format!("{}/MANIFEST", volume)) else { continue };
        let manifest = Manifest::parse(&text).unwrap();
        assert!(manifest.files() > 20, "{}", volume);
        assert_eq!(encode_again(&manifest), text, "{}", volume);
    }
}

#[test]
fn other_forms_of_a_manifest_are_refused() {
    let good = String::from_utf8(manifest_of(&[("a.elf", b"1"), ("b.elf", b"2")])).unwrap();
    let line_a = good.lines().find(|l| l.starts_with("file a.elf")).unwrap().to_string();
    let line_b = good.lines().find(|l| l.starts_with("file b.elf")).unwrap().to_string();
    let changed = |from: &str, to: &str| -> Vec<u8> { assert!(good.contains(from), "{}", from); good.replacen(from, to, 1).into_bytes() };
    let digest = line_a.split(' ').nth(3).unwrap();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("another format", changed("MIND-MANIFEST 1", "MIND-MANIFEST 2")),
        ("files out of order", changed(&format!("{}\n{}", line_a, line_b), &format!("{}\n{}", line_b, line_a))),
        ("a file twice", changed(&line_b, &line_a)),
        ("a path with ..", changed("file a.elf", "file ../a.elf")),
        ("an absolute path", changed("file a.elf", "file /a.elf")),
        ("an empty part", changed("file a.elf", "file x//a.elf")),
        ("upper-case hex", changed(digest, &digest.to_uppercase())),
        ("short flags", changed(" 00000000 0", " 0000000 0")),
        ("a leading zero", changed("file a.elf 1 ", "file a.elf 01 ")),
        ("two spaces", changed("file a.elf 1 ", "file a.elf  1 ")),
        ("a field more", changed(&line_a, &format!("{} 0", line_a))),
        ("a header after a file", format!("{}note x\n", good).into_bytes()),
        ("a header without a value", changed("toolchain nightly-x", "toolchain")),
        ("no final newline", good.trim_end().as_bytes().to_vec()),
        ("CRLF", good.replace('\n', "\r\n").into_bytes()),
        ("a tab", changed("toolchain nightly-x", "toolchain\tnightly-x")),
    ];
    for (what, text) in cases {
        assert!(Manifest::parse(&text).is_err(), "{}: {}", what, String::from_utf8_lossy(&text));
    }
    // The build's order is by path components: `voice/m.bin` before `voice.elf`, and not the other way round.
    let voice = String::from_utf8(manifest_of(&[("voice/m.bin", b"1"), ("voice.elf", b"2")])).unwrap();
    let (dir, elf) = (voice.lines().find(|l| l.starts_with("file voice/")).unwrap(), voice.lines().find(|l| l.starts_with("file voice.elf")).unwrap());
    assert!(voice.find(dir) < voice.find(elf));
    assert!(Manifest::parse(voice.as_bytes()).is_ok());
    assert!(Manifest::parse(voice.replacen(&format!("{}\n{}", dir, elf), &format!("{}\n{}", elf, dir), 1).as_bytes()).is_err());
    // A header line the format may gain later is kept as text; nine are too many.
    let more = changed("toolchain nightly-x\n", "toolchain nightly-x\nbranch claude/x\n");
    let manifest = Manifest::parse(&more).unwrap();
    assert_eq!((manifest.headers(), encode_again(&manifest)), (6, more.clone()));
    let nine = changed("toolchain nightly-x\n", "toolchain nightly-x\na 1\nb 2\nc 3\nd 4\n");
    assert!(Manifest::parse(&nine).is_err());
}

#[test]
fn a_parser_that_lies_is_caught() {
    // The updater's check of the parser's answer: the fields it gives must make exactly the file.
    let file = stable();
    let (channel, signed) = parse_channel(&file).unwrap();
    assert!(channel_is(&file, &channel, &signed));
    let mut lies = vec![channel; 4];
    lies[0].version += 1;
    lies[1].minimum = 1;
    lies[2].manifests[0].1[0] ^= 1;
    lies[3].count = 1;
    for lie in &lies { assert!(!channel_is(&file, lie, &signed), "{:?}", lie); }
    let mut other = signed;
    other.signatures[0].value[63] ^= 1;
    assert!(!channel_is(&file, &channel, &other));
    other = signed;
    other.signatures[0].key[0] ^= 1;
    assert!(!channel_is(&file, &channel, &other), "another key named");
    other = signed;
    other.count = 2;
    assert!(!channel_is(&file, &channel, &other), "a signature added");
    other = signed;
    other.body -= 1;
    assert!(!channel_is(&file, &channel, &other));

    let text = manifest_of(&[("a.elf", b"1"), ("b.elf", b"2"), ("c.elf", b"3")]);
    let manifest = Manifest::parse(&text).unwrap();
    let files: Vec<_> = (0..manifest.files()).map(|i| manifest.file(i).unwrap()).collect();
    let rebuild = |headers: usize, files: &[release::File]| {
        let mut r = Rebuild::new(&text);
        for i in 0..headers { r.line(manifest.header(i).unwrap()); }
        for f in files { r.file(f); }
        r.done()
    };
    assert!(rebuild(5, &files));
    assert!(!rebuild(4, &files), "a header line left out");
    assert!(!rebuild(5, &files[..2]), "a file left out");
    let mut bigger = files.clone();
    bigger[1].size += 1;
    assert!(!rebuild(5, &bigger), "a size changed");
    let mut swapped = files.clone();
    swapped.swap(0, 1);
    assert!(!rebuild(5, &swapped), "files reordered");
}

// A key file as `release.py keys` makes it: the test roles, signed by the test root keys `roots` (indices of
// release.TEST_ROOT_SEEDS, repeats and other seeds as Python expressions allowed).
fn keys_file(roots: &str) -> Vec<u8> {
    python(&format!("import release, datetime\nnow = datetime.datetime(2026, 11, 13, tzinfo=datetime.timezone.utc)\n\
                     whole = release.test_keys_file(now, roots=[{roots}])\nsys.stdout.buffer.write(whole)"))
}

// The test root set's public keys and threshold, as the updater's build reads them.
fn root_set() -> (Vec<[u8; 32]>, u8) {
    let text = String::from_utf8(python("import release\nk, t = release.root_set()\nprint(t, *[x.hex() for x in k])")).unwrap();
    let mut words = text.split_whitespace();
    let threshold = words.next().unwrap().parse().unwrap();
    (words.map(|w| (0..32).map(|i| u8::from_str_radix(&w[2 * i..2 * i + 2], 16).unwrap()).collect::<Vec<_>>().try_into().unwrap()).collect(), threshold)
}

fn verify(key: &[u8; 32], body: &[u8], signature: &[u8; 64]) -> bool {
    let code = format!("import sign_manifest as sm\nprint(sm.verify(bytes.fromhex({:?}), bytes.fromhex({:?}), bytes.fromhex({:?})))", hex(key), hex(body), hex(signature));
    python(&code) == b"True\n"
}

const TWO: &str = "release.TEST_ROOT_SEEDS[0], release.TEST_ROOT_SEEDS[1]";

#[test]
fn a_key_file_reads_and_encodes_to_its_signed_line() {
    let file = keys_file(TWO);
    let (keys, signed) = parse_keys(&file).unwrap();
    assert_eq!((keys.version, &keys.expires), (1, b"2036-11-10T00:00:00Z"));
    assert_eq!(signed.count, 2);
    for name in ROLES {
        let role = keys.role(name).unwrap();
        assert_eq!((role.count, role.threshold), if name == "root" { (3, 2) } else { (1, 1) }, "{}", name);
    }
    assert_eq!(keys.root(), keys.role("root").unwrap());
    let mut body = [0u8; KEYS_MAX];
    let n = encode_keys(&keys, &mut body).unwrap();
    assert_eq!(&body[..n], &file[..signed.body]);
    assert!(keys_is(&file, &keys, &signed));
    let release = keys.role("release").unwrap().keys()[0];
    let test_release = python("import release, sign_manifest as sm\nsys.stdout.buffer.write(sm.public_key(release.TEST_RELEASE_SEED))");
    assert_eq!(&release[..], &test_release[..]);
}

#[test]
fn the_root_threshold_counts_each_known_key_once() {
    let (roots, threshold) = root_set();
    assert_eq!((roots.len(), threshold), (3, 2));
    let counts = |given: &str| {
        let file = keys_file(given);
        let (_, signed) = parse_keys(&file).unwrap();
        signed_by(&roots, threshold, &file[..signed.body], &signed, &verify)
    };
    assert!(counts(TWO), "two of three");
    assert!(counts("*release.TEST_ROOT_SEEDS"), "all three");
    assert!(!counts("release.TEST_ROOT_SEEDS[2]"), "one of three");
    assert!(!counts("release.TEST_ROOT_SEEDS[1], release.TEST_ROOT_SEEDS[1]"), "two signatures by one root key count once");
    assert!(counts("release.TEST_ROOT_SEEDS[0], release.TEST_ROOT_SEEDS[0], release.TEST_ROOT_SEEDS[2]"), "a repeat passed over");
    assert!(!counts("release.TEST_ROOT_SEEDS[0], release.TEST_RELEASE_SEED"), "a signature by an unknown key is ignored");
    assert!(counts("release.TEST_RELEASE_SEED, release.TEST_ROOT_SEEDS[0], release.TEST_ROOT_SEEDS[2]"), "an unknown key among them");
    // A root key's line whose signature is not over this body does not count.
    let file = keys_file(TWO);
    let (_, mut signed) = parse_keys(&file).unwrap();
    signed.signatures[1].value[0] ^= 1;
    assert!(!signed_by(&roots, threshold, &file[..signed.body], &signed, &verify), "a bad signature");
    assert!(!signed_by(&roots, 0, &file[..signed.body], &signed, &verify), "threshold 0");
}

#[test]
fn other_forms_of_a_key_file_are_refused() {
    let good = String::from_utf8(keys_file(TWO)).unwrap();
    let (body, signatures) = good.split_once('\n').unwrap();
    let changed = |from: &str, to: &str| -> Vec<u8> { assert!(good.contains(from), "{}", from); good.replacen(from, to, 1).into_bytes() };
    let release_key = &body[body.find("\"release\":{\"keys\":[\"").unwrap() + 20..][..64];
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("a space", changed("\"version\":1", "\"version\": 1")),
        ("version 0", changed("\"version\":1}", "\"version\":0}")),
        ("threshold 0", changed("\"threshold\":1}", "\"threshold\":0}")),
        ("a threshold above the keys", changed("\"threshold\":1}", "\"threshold\":2}")),
        ("a role left out", changed("\"recovery\":", "\"recovered\":")),
        ("roles out of order", changed("\"boot\":", "\"zzzz\":")),
        ("no keys", changed(&format!("[\"{}\"]", release_key), "[]")),
        ("a key twice", changed(&format!("[\"{}\"]", release_key), &format!("[\"{0}\",\"{0}\"]", release_key))),
        ("five keys", changed(&format!("[\"{}\"]", release_key), &format!("[{}]", (1..=5).map(|i| format!("\"{:064x}\"", i)).collect::<Vec<_>>().join(",")))),
        ("keys out of order", changed(&format!("[\"{}\"]", release_key), &format!("[\"{:064x}\",\"{:064x}\"]", 2, 1))),
        ("an upper-case key", changed(release_key, &release_key.to_uppercase())),
        ("a short key", changed(release_key, &release_key[..62])),
        ("29 February of 2026", changed("2036-11-10", "2026-02-29")),
        ("no signature line", format!("{}\n", body).into_bytes()),
        ("a signature of before 351-UPD-0026, naming no key", format!("{}\ned25519 {}\n", body, &signatures.lines().next().unwrap()[8 + 65..]).into_bytes()),
        ("bytes after the signatures", format!("{}x", good).into_bytes()),
        ("CRLF", good.replace('\n', "\r\n").into_bytes()),
    ];
    for (what, file) in cases {
        assert_eq!(parse_keys(&file).err(), Some(Malformed), "{}: {}", what, String::from_utf8_lossy(&file));
    }
    let mut big = good.clone().into_bytes();
    big.resize(KEYS_MAX + 1, b'\n');
    assert_eq!(parse_keys(&big).err(), Some(Malformed), "larger than KEYS_MAX");
}

#[test]
fn a_changed_byte_of_a_key_file_is_refused_or_reads_as_what_it_encodes() {
    let file = keys_file(TWO);
    let mut body = [0u8; KEYS_MAX];
    let mut accepted = 0;
    for at in 0..file.len() {
        for value in [b'0', b'9', b'a', b'f', b'"', b',', b'\n', b' ', 0x80] {
            let mut changed = file.clone();
            if changed[at] == value { continue; }
            changed[at] = value;
            if let Ok((keys, signed)) = parse_keys(&changed) {
                accepted += 1;
                let n = encode_keys(&keys, &mut body).unwrap();
                assert_eq!(&body[..n], &changed[..signed.body], "at {}", at);
                assert!(keys_is(&changed, &keys, &signed), "at {}", at);
            }
        }
    }
    assert!(accepted > 0);
}

#[test]
fn a_parser_that_lies_about_a_key_file_is_caught() {
    let file = keys_file(TWO);
    let (keys, signed) = parse_keys(&file).unwrap();
    let mut lies = vec![keys; 4];
    lies[0].version += 1;
    lies[1].roles[2].keys[0][0] ^= 1;
    lies[2].roles[2].threshold = 0;
    lies[3].expires[3] = b'7';
    for lie in &lies { assert!(!keys_is(&file, lie, &signed), "{:?}", lie); }
    let mut other = signed;
    other.count = 1;
    assert!(!keys_is(&file, &keys, &other), "a signature left out");
}

// Key file `version` as `release.py keys` makes one: its root role the seeds `root` with threshold `threshold`, signed by
// the seeds `signers` (Python expressions; NEW is a second root set of two keys).
fn version_file(version: u64, root: &str, threshold: u8, signers: &str) -> Vec<u8> {
    python(&format!("import release, datetime, hashlib\nnow = datetime.datetime(2026, 11, 13, tzinfo=datetime.timezone.utc)\n\
                     NEW = [hashlib.sha256(f'another root {{i}}'.encode()).digest() for i in (1, 2)]\n\
                     whole = release.test_keys_file(now, version={version}, roots=[{signers}], root=([{root}], {threshold}))\n\
                     sys.stdout.buffer.write(whole)"))
}

#[test]
fn a_device_takes_the_next_key_file_signed_by_both_roots() {
    let (roots, threshold) = root_set();
    let old = "*release.TEST_ROOT_SEEDS";
    let first = version_file(1, old, 2, TWO);
    let (v1, s1) = parse_keys(&first).unwrap();
    let take = |held: u64, keys: &[[u8; 32]], threshold: u8, file: &[u8]| {
        let (next, signed) = parse_keys(file).unwrap();
        step(held, keys, threshold, &next, &file[..signed.body], &signed, verify)
    };
    // Version 1 from none: the built-in root signed it, and its own root is that root.
    assert_eq!(step(0, &roots, threshold, &v1, &first[..s1.body], &s1, verify), Step::Next);
    assert_eq!(take(0, &roots, threshold, &version_file(1, old, 2, "release.TEST_ROOT_SEEDS[0]")), Step::Unsigned, "one root key of two");
    // The root rotated to a set of two, one of which signs: the old root's threshold and the new one's must both sign.
    let root = v1.root();
    assert_eq!(take(1, root.keys(), root.threshold, &version_file(2, "*NEW", 1, &format!("{}, NEW[0]", TWO))), Step::Next);
    assert_eq!(take(1, root.keys(), root.threshold, &version_file(2, "*NEW", 1, TWO)), Step::Unsigned, "the new root did not sign");
    assert_eq!(take(1, root.keys(), root.threshold, &version_file(2, "*NEW", 1, "NEW[0], NEW[1]")), Step::Unsigned, "the old root did not sign");
    assert_eq!(take(1, root.keys(), root.threshold, &version_file(2, "*NEW", 1, "release.TEST_ROOT_SEEDS[0], NEW[0]")), Step::Unsigned,
               "one of the old root's two");
    // Once version 2 is held, its root alone signs version 3; the old one is not trusted.
    let (v2, _) = parse_keys(&version_file(2, "*NEW", 1, &format!("{}, NEW[0]", TWO))).unwrap();
    let root2 = v2.root();
    assert_eq!(take(2, root2.keys(), root2.threshold, &version_file(3, "*NEW", 1, "NEW[1]")), Step::Next);
    assert_eq!(take(2, root2.keys(), root2.threshold, &version_file(3, "*NEW", 1, TWO)), Step::Unsigned, "the root before");
    // A lower version is refused, the same one is compared by the caller, and one past the next waits for those between.
    assert_eq!(take(2, root2.keys(), root2.threshold, &first), Step::Lower);
    assert_eq!(take(2, root2.keys(), root2.threshold, &version_file(2, "*NEW", 1, "NEW[0]")), Step::Same);
    assert_eq!(take(1, root.keys(), root.threshold, &version_file(3, old, 2, TWO)), Step::Skips);
}

// A timestamp as `release.py` writes it: channel stable, the digest of `b"a channel"`, signed by the test timestamp key.
fn timestamp_file() -> Vec<u8> {
    python("import release, hashlib\nbody = release.timestamp_bytes('stable', hashlib.sha256(b'a channel').hexdigest(), '2026-11-14T06:00:00Z', 7)\n\
            sys.stdout.buffer.write(body + release.signature_lines(body, [release.TEST_TIMESTAMP_SEED]))")
}

#[test]
fn a_timestamp_reads_and_encodes_to_its_signed_line() {
    let file = timestamp_file();
    let (stamp, signed) = parse_timestamp(&file).unwrap();
    assert_eq!((stamp.channel.as_str(), stamp.version, &stamp.expires), ("stable", 7, b"2026-11-14T06:00:00Z"));
    let mut body = [0u8; TIMESTAMP_MAX];
    let n = encode_timestamp(&stamp, &mut body).unwrap();
    assert_eq!(&body[..n], &file[..signed.body]);
    assert!(timestamp_is(&file, &stamp, &signed));
    let digest = python("import hashlib\nsys.stdout.buffer.write(hashlib.sha256(b'a channel').digest())");
    assert_eq!(&stamp.digest[..], &digest[..]);
    // A parser that lies about any field is caught.
    let mut lies = vec![stamp; 3];
    lies[0].version += 1;
    lies[1].digest[0] ^= 1;
    lies[2].expires[3] = b'7';
    for lie in &lies { assert!(!timestamp_is(&file, lie, &signed), "{:?}", lie); }
}

#[test]
fn other_forms_of_a_timestamp_are_refused() {
    let good = String::from_utf8(timestamp_file()).unwrap();
    let changed = |from: &str, to: &str| -> Vec<u8> { assert!(good.contains(from), "{}", from); good.replacen(from, to, 1).into_bytes() };
    let digest = &good[good.find("\"digest\":\"").unwrap() + 10..][..64];
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("a space", changed("\"version\":7", "\"version\": 7")),
        ("version 0", changed("\"version\":7}", "\"version\":0}")),
        ("an upper-case digest", changed(digest, &digest.to_uppercase())),
        ("a short digest", changed(digest, &digest[..62])),
        ("no expiry", changed("\"expires\":\"2026-11-14T06:00:00Z\",", "")),
        ("an expiry not a time", changed("2026-11-14T06", "2026-11-31T06")),
        ("fields out of order", changed("{\"channel\":\"stable\",", "{\"zchannel\":\"stable\",")),
        ("no signature", good.split_inclusive('\n').next().unwrap().as_bytes().to_vec()),
    ];
    for (what, bytes) in &cases { assert_eq!(parse_timestamp(bytes).err(), Some(Malformed), "{}", what); }
    let mut big = good.clone().into_bytes();
    big.resize(TIMESTAMP_MAX + 1, b'\n');
    assert_eq!(parse_timestamp(&big).err(), Some(Malformed), "larger than TIMESTAMP_MAX");
    // Any byte changed: refused, or read as what it then encodes.
    let file = good.into_bytes();
    for at in 0..file.len() {
        for value in [b'0', b'9', b'a', b'f', b'"', b'\n', b' '] {
            let mut bytes = file.clone();
            if bytes[at] == value { continue; }
            bytes[at] = value;
            if let Ok((stamp, signed)) = parse_timestamp(&bytes) { assert!(timestamp_is(&bytes, &stamp, &signed), "at {}", at); }
        }
    }
}
