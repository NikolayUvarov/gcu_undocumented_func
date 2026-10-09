//! Host tests of the release metadata (libmind/src/release.rs, 351-UPD-0013): channel files and boot manifests made by
//! scripts/release.py and scripts/sign_manifest.py are read field by field and encode again to exactly their bytes;
//! every other form is refused, and any byte changed in a valid file is refused or reads as what it then encodes.
#![allow(dead_code)]
#[path = "../libmind/src/release.rs"]
mod release;

use release::{encode_channel, encode_file, expires_unix, parse_channel, Malformed, Manifest, CHANNEL_MAX, FILE_LINE_MAX};
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
        "import release, sign_manifest as sm\nbody = release.channel_bytes({name:?}, {version}, {minimum}, {expires:?}, {manifests})\n\
         sys.stdout.buffer.write(body + b'ed25519 ' + sm.sign(release.TEST_RELEASE_SEED, body).hex().encode() + b'\\n')"))
}

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
    assert_eq!(line.trim_end().strip_prefix("ed25519 ").unwrap(), signed.signature.iter().map(|b| format!("{:02x}", b)).collect::<String>());
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
        ("a short signature", changed(&signature[..20], &signature[..18])),
        ("upper-case signature", format!("{}\ned25519 {}\n", body, signature[8..].to_uppercase()).into_bytes()),
        ("no signature line", format!("{}\n", body).into_bytes()),
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
