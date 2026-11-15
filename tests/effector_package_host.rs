//! The gateway's packages (750-EFF-0008, contract 1.5): `manifest.json`'s entry for this machine, then the install
//! under `data/apps/` with `current` switched last, the removal, and each refusal leaving the installed version as it was.
//! The signature check is a stand-in here (`apppkg` has its own tests, and the effector suite runs the real one).
#![allow(dead_code)]
extern crate alloc;
#[path = "../libmind/src/sha256.rs"] mod sha256;
#[path = "../libmind/src/json.rs"] mod json;
#[path = "../libmind/src/inflate.rs"] mod inflate;
#[path = "../libmind/src/zip.rs"] mod zip;
#[path = "../effector_gw/src/package.rs"] mod package;
use package::*;
use std::collections::BTreeMap;

fn sha(data: &[u8]) -> [u8; 32] { let mut h = sha256::Sha256::new(); h.update(data); h.finish() }

// A ZIP of stored entries.
fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let (mut out, mut central) = (Vec::new(), Vec::new());
    for (name, data) in entries {
        let crc = zip::crc32_update(0, data);
        let at = out.len() as u32;
        out.extend_from_slice(&0x04034b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(&0x02014b50u32.to_le_bytes());
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&at.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let (start, size) = (out.len() as u32, central.len() as u32);
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x06054b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

const MANIFEST_JSON: &str = r#"{"manifest_version":1,"package":{"name":"app","version":"2"},
 "entrypoints":{"install":[{"when":{"os":"windows","arch":"x64"},"cmd":"setup.exe"},
   {"when":{"os":"mindcore","arch":"x64"},"package":"mindcore/x86_64"},{"when":{"os":"mindcore","arch":"arm64"},"package":"mindcore/aarch64"}],
  "deinstall":[{"when":{"os":"mindcore","arch":"x64"},"package":"mindcore/x86_64"}]}}"#;

// The stand-in MANIFEST: `NAME VERSION`, then a line `PATH SIZE` a file; the signature is good when all 7s.
fn manifest(name: &str, version: u64, files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut m = format!("{} {}\n", name, version);
    for (path, data) in files { m += &format!("{} {} {}\n", path, data.len(), package::hex(&sha(data))); }
    m.into_bytes()
}

fn package_zip(json: &str, name: &str, version: u64, files: &[(&str, &[u8])], signature: [u8; 64]) -> Vec<u8> {
    let m = manifest(name, version, files);
    let mut entries: Vec<(String, Vec<u8>)> = vec![("manifest.json".into(), json.as_bytes().to_vec()), ("mindcore/x86_64/MANIFEST".into(), m), ("mindcore/x86_64/MANIFEST.SIG".into(), signature.to_vec())];
    for (path, data) in files { entries.push((format!("mindcore/x86_64/{}", path), data.to_vec())); }
    let refs: Vec<(&str, &[u8])> = entries.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
    zip_of(&refs)
}

struct Stand;
impl Verify for Stand {
    fn name(&mut self, manifest: &[u8]) -> Result<String, Refused> {
        Ok(std::str::from_utf8(manifest).unwrap().split_whitespace().next().unwrap().to_string())
    }
    fn verify(&mut self, manifest: &[u8], signature: &[u8; 64], installed: Option<Installed>) -> Result<Checked, Refused> {
        if signature != &[7; 64] { return Err(Refused { code: Code::Signature, what: "MANIFEST.SIG".into() }); }
        let text = std::str::from_utf8(manifest).unwrap();
        let mut lines = text.lines();
        let mut head = lines.next().unwrap().split(' ');
        let (name, version) = (head.next().unwrap().to_string(), head.next().unwrap().parse::<u64>().unwrap());
        if let Some(i) = installed {
            if version < i.version || (version == i.version && i.manifest != sha(manifest)) { return Err(Refused { code: Code::Older, what: "the version".into() }); }
        }
        let files = lines.map(|l| {
            let f: Vec<&str> = l.split(' ').collect();
            let mut sha256 = [0u8; 32];
            for (i, b) in sha256.iter_mut().enumerate() { *b = u8::from_str_radix(&f[2][2 * i..2 * i + 2], 16).unwrap(); }
            File { path: f[0].into(), size: f[1].parse().unwrap(), sha256 }
        }).collect();
        Ok(Checked { name, version, files })
    }
}

#[derive(Default)]
struct Disk { files: BTreeMap<String, Vec<u8>>, full_after: Option<usize> }
impl Apps for Disk {
    fn read(&mut self, path: &str, max: usize) -> Result<Option<Vec<u8>>, Code> {
        Ok(self.files.get(path).map(|d| d[..d.len().min(max)].to_vec()))
    }
    fn write(&mut self, path: &str, data: &[u8]) -> Result<(), Code> {
        if let Some(n) = self.full_after.as_mut() { if *n == 0 { return Err(Code::Space); } *n -= 1; }
        self.files.insert(path.into(), data.to_vec());
        Ok(())
    }
    fn rename(&mut self, from: &str, to: &str) -> Result<(), Code> {
        let moved: Vec<String> = self.files.keys().filter(|k| *k == from || k.starts_with(&format!("{}/", from))).cloned().collect();
        if moved.is_empty() { return Err(Code::Io); }
        for k in moved { let d = self.files.remove(&k).unwrap(); self.files.insert(format!("{}{}", to, &k[from.len()..]), d); }
        Ok(())
    }
    fn remove(&mut self, path: &str) -> Result<(), Code> {
        self.files.retain(|k, _| k != path && !k.starts_with(&format!("{}/", path)));
        Ok(())
    }
    fn exists(&mut self, path: &str) -> bool { self.files.keys().any(|k| k == path || k.starts_with(&format!("{}/", path))) }
}

const PROGRAM: &[u8] = b"\x7fELF a program";
const DATA: &[u8] = b"its data";

#[test]
fn the_mindcore_entry_for_this_machine_is_chosen_and_others_passed_over() {
    assert_eq!(select(MANIFEST_JSON.as_bytes(), Operation::Install, "x64"), Ok("mindcore/x86_64"));
    assert_eq!(select(MANIFEST_JSON.as_bytes(), Operation::Reinstall, "arm64"), Ok("mindcore/aarch64"));
    assert_eq!(select(MANIFEST_JSON.as_bytes(), Operation::Deinstall, "x64"), Ok("mindcore/x86_64"));
    assert_eq!(select(MANIFEST_JSON.as_bytes(), Operation::Deinstall, "arm64").unwrap_err().code, Code::Arch);
}

#[test]
fn a_manifest_json_that_breaks_the_contract_is_refused_as_format() {
    let entry = |e: &str| format!(r#"{{"manifest_version":1,"entrypoints":{{"install":[{}]}}}}"#, e);
    for bad in [
        entry(r#"{"when":{"os":"mindcore","arch":"x64"},"package":"mindcore/x86_64","cmd":"run"}"#),
        entry(r#"{"when":{"os":"mindcore","arch":"x64"},"package":"mindcore/x86_64","env":{}}"#),
        entry(r#"{"when":{"os":"mindcore","arch":"x64","version":"1"},"package":"mindcore/x86_64"}"#),
        entry(r#"{"when":{"os":"mindcore","arch":"x64"},"package":"mindcore/aarch64"}"#),
        entry(r#"{"when":{"os":"mindcore","arch":"x64"},"package":"mindcore/x86_64"},{"when":{"os":"mindcore","arch":"x64"},"package":"mindcore/x86_64"}"#),
        entry(r#"{"when":{"os":"mindcore","arch":"riscv"},"package":"mindcore/riscv"}"#),
        r#"{"manifest_version":2,"entrypoints":{"install":[{"when":{"os":"mindcore","arch":"x64"},"package":"mindcore/x86_64"}]}}"#.into(),
        "[1]".into(), "{\"manifest_version\":1".into(),
    ] {
        assert_eq!(select(bad.as_bytes(), Operation::Install, "x64").map_err(|r| r.code), Err(Code::Format), "{bad}");
    }
}

#[test]
fn an_install_writes_the_version_then_switches_current_and_keeps_the_previous() {
    let mut disk = Disk::default();
    let one = package_zip(MANIFEST_JSON, "app", 1, &[("m", PROGRAM), ("lib/d", DATA)], [7; 64]);
    let report = run(&one, Operation::Install, "x64", &mut Stand, &mut disk).unwrap();
    assert_eq!((report.name.as_str(), report.version, report.previous, report.files), ("app", 1, None, 2));
    assert!(report.line(Operation::Install).starts_with("installed app 1 x86_64 2 "), "{}", report.line(Operation::Install));
    assert_eq!(disk.files["app/current"], b"1\n");
    assert_eq!((disk.files["app/1/m"].as_slice(), disk.files["app/1/lib/d"].as_slice()), (PROGRAM, DATA));
    assert_eq!(disk.files["app/1/MANIFEST.SIG"], [7; 64]);
    let two = package_zip(MANIFEST_JSON, "app", 2, &[("m", b"\x7fELF the next one")], [7; 64]);
    let report = run(&two, Operation::Reinstall, "x64", &mut Stand, &mut disk).unwrap();
    assert_eq!((report.version, report.previous), (2, Some(1)));
    assert_eq!(disk.files["app/current"], b"2\n");
    assert!(disk.files.contains_key("app/1/m"), "the previous version stays");
    assert!(!disk.files.keys().any(|k| k.contains(".new-") || k.contains(".old-") || k.ends_with("current.new")), "{:?}", disk.files.keys());
    // The same version again, the same bytes: installed again in its place.
    assert_eq!(run(&two, Operation::Reinstall, "x64", &mut Stand, &mut disk).unwrap().version, 2);
    assert_eq!(disk.files["app/current"], b"2\n");
}

#[test]
fn a_refusal_leaves_the_installed_version_and_nothing_half_written() {
    let mut disk = Disk::default();
    run(&package_zip(MANIFEST_JSON, "app", 2, &[("m", PROGRAM)], [7; 64]), Operation::Install, "x64", &mut Stand, &mut disk).unwrap();
    let before = disk.files.clone();
    // A bad signature, an older version, the same version with other bytes.
    let bad_sig = package_zip(MANIFEST_JSON, "app", 3, &[("m", PROGRAM)], [8; 64]);
    assert_eq!(run(&bad_sig, Operation::Install, "x64", &mut Stand, &mut disk).unwrap_err().code, Code::Signature);
    let older = package_zip(MANIFEST_JSON, "app", 1, &[("m", PROGRAM)], [7; 64]);
    assert_eq!(run(&older, Operation::Install, "x64", &mut Stand, &mut disk).unwrap_err().code, Code::Older);
    let other = package_zip(MANIFEST_JSON, "app", 2, &[("m", b"other bytes")], [7; 64]);
    assert_eq!(run(&other, Operation::Reinstall, "x64", &mut Stand, &mut disk).unwrap_err().code, Code::Older);
    // A file that differs from its MANIFEST line.
    let mut changed = package_zip(MANIFEST_JSON, "app", 3, &[("m", PROGRAM)], [7; 64]);
    let at = changed.windows(PROGRAM.len()).position(|w| w == PROGRAM).unwrap();
    changed[at + 5] ^= 1;
    let code = run(&changed, Operation::Install, "x64", &mut Stand, &mut disk).unwrap_err().code;
    assert!(code == Code::Digest || code == Code::Format, "{code:?}"); // the CRC-32 or the SHA-256 finds it
    // The disk full after the first file.
    disk.full_after = Some(1);
    let two_files = package_zip(MANIFEST_JSON, "app", 3, &[("m", PROGRAM), ("d", DATA)], [7; 64]);
    assert_eq!(run(&two_files, Operation::Install, "x64", &mut Stand, &mut disk).unwrap_err().code, Code::Space);
    disk.full_after = None;
    assert_eq!(disk.files, before, "the installed version as it was, nothing staged left");
    // Not a ZIP, and a ZIP without the machine's entry.
    assert_eq!(run(b"not a zip", Operation::Install, "x64", &mut Stand, &mut disk).unwrap_err().code, Code::Format);
    let arm_only = r#"{"manifest_version":1,"entrypoints":{"install":[{"when":{"os":"mindcore","arch":"arm64"},"package":"mindcore/aarch64"}]}}"#;
    assert_eq!(run(&package_zip(arm_only, "app", 3, &[("m", PROGRAM)], [7; 64]), Operation::Install, "x64", &mut Stand, &mut disk).unwrap_err().code, Code::Arch);
    assert_eq!(disk.files, before);
}

#[test]
fn a_deinstall_removes_current_then_the_application() {
    let mut disk = Disk::default();
    run(&package_zip(MANIFEST_JSON, "app", 1, &[("m", PROGRAM)], [7; 64]), Operation::Install, "x64", &mut Stand, &mut disk).unwrap();
    run(&package_zip(MANIFEST_JSON, "app", 2, &[("m", PROGRAM)], [7; 64]), Operation::Install, "x64", &mut Stand, &mut disk).unwrap();
    disk.files.insert("other/current".into(), b"5\n".to_vec());
    let removal = package_zip(MANIFEST_JSON, "app", 2, &[("m", PROGRAM)], [7; 64]);
    // Its signature is checked: a bad one removes nothing.
    let unsigned = package_zip(MANIFEST_JSON, "app", 2, &[("m", PROGRAM)], [0; 64]);
    assert_eq!(run(&unsigned, Operation::Deinstall, "x64", &mut Stand, &mut disk).unwrap_err().code, Code::Signature);
    assert!(disk.files.contains_key("app/current"));
    let report = run(&removal, Operation::Deinstall, "x64", &mut Stand, &mut disk).unwrap();
    assert_eq!(report.line(Operation::Deinstall), "removed app 2");
    assert!(!disk.files.keys().any(|k| k.starts_with("app/")), "{:?}", disk.files.keys());
    assert!(disk.files.contains_key("other/current"), "another application stays");
    // Nothing installed: removed, naming none.
    assert_eq!(run(&removal, Operation::Deinstall, "x64", &mut Stand, &mut disk).unwrap().line(Operation::Deinstall), "removed app none");
}

#[test]
fn the_lines_follow_the_contract() {
    let r = Refused { code: Code::Older, what: String::from("version 1 is below the installed 2") };
    assert_eq!(refusal_line(&r), "refused older: version 1 is below the installed 2");
    let names: Vec<&str> = [Code::Unsupported, Code::Denied, Code::Digest, Code::Format, Code::Signature, Code::Arch, Code::System, Code::Older, Code::Space, Code::Busy, Code::Io].iter().map(|c| c.name()).collect();
    assert_eq!(names, ["unsupported", "denied", "digest", "format", "signature", "arch", "system", "older", "space", "busy", "io"]);
}
