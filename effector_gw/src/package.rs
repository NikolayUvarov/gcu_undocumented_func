//! Signed application packages through the gateway (750-EFF-0008, contract 1.5 `mind-package-1`): the Effector archive's
//! `manifest.json` entry for this machine, the MIND package in that entry's directory (750-UPD-0019's MANIFEST and
//! MANIFEST.SIG), and its install under `data/apps/<name>/<version>/` with `current` switched last, in one rename. The
//! signature and the version are checked by `Verify` (`apppkg` in the service), the files by their SHA-256 here, all
//! before `current` changes: any failure leaves the previous version in place. No system calls: host-tested in
//! tests/effector_package_host.rs.
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::json::Reader;
use crate::sha256::Sha256;
use crate::zip::{Archive, Limits};

/// The largest archive the gateway takes: it is held in memory while it is read (a 16 MiB heap).
pub const ARCHIVE_MAX: u64 = 8 << 20;
/// `manifest.json` at most, as contract 1.5 bounds it.
pub const JSON_MAX: usize = 64 * 1024;
/// MANIFEST at most (`apppkg::MANIFEST_MAX`).
pub const MANIFEST_MAX: usize = 64 * 1024;
/// The archive's bounds: entries, a name, an entry and all of them uncompressed, and the ratio.
pub const LIMITS: Limits = Limits { entries: 300, name: 200, entry: 8 << 20, total: 32 << 20, ratio: 200 };

/// Why a package is refused: contract 1.5's codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code { Unsupported, Denied, Digest, Format, Signature, Arch, System, Older, Space, Busy, Io }

impl Code {
    pub fn name(self) -> &'static str {
        match self {
            Code::Unsupported => "unsupported", Code::Denied => "denied", Code::Digest => "digest", Code::Format => "format",
            Code::Signature => "signature", Code::Arch => "arch", Code::System => "system", Code::Older => "older",
            Code::Space => "space", Code::Busy => "busy", Code::Io => "io",
        }
    }
}

/// A refusal: its code and what was checked, for the result's `refused CODE: …` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused { pub code: Code, pub what: String }

fn refused(code: Code, what: impl Into<String>) -> Refused { Refused { code, what: what.into() } }

/// What the command asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation { Install, Reinstall, Deinstall }

impl Operation {
    // The manifest's list of entries the operation reads.
    fn list(self) -> &'static [u8] { match self { Operation::Deinstall => b"deinstall", _ => b"install" } }
}

/// The heartbeat's name of this machine's architecture, and the package directory it maps to.
pub fn directory(arch: &str) -> Option<&'static str> {
    match arch { "x64" => Some("mindcore/x86_64"), "arm64" => Some("mindcore/aarch64"), _ => None }
}

// One entry of a manifest's list, as read.
#[derive(Default)]
struct Rule { os: Option<String>, arch: Option<String>, package: Option<String>, cmd: bool, other: bool }

fn text(r: &mut Reader<'_>) -> Result<String, Refused> {
    let mut buf = [0u8; 256];
    let s = r.string(&mut buf).map_err(|_| refused(Code::Format, "manifest.json: a string"))?;
    core::str::from_utf8(s).map(String::from).map_err(|_| refused(Code::Format, "manifest.json: a string"))
}

fn rule(r: &mut Reader<'_>) -> Result<Rule, Refused> {
    let bad = |_| refused(Code::Format, "manifest.json: an entry");
    let mut rule = Rule::default();
    r.object().map_err(bad)?;
    let mut key = [0u8; 32];
    while let Some(k) = r.key(&mut key).map_err(bad)? {
        match k {
            b"when" => {
                r.object().map_err(bad)?;
                let mut inner = [0u8; 32];
                while let Some(w) = r.key(&mut inner).map_err(bad)? {
                    match w {
                        b"os" => rule.os = Some(text(r)?),
                        b"arch" => rule.arch = Some(text(r)?),
                        _ => { rule.other = true; r.skip().map_err(bad)?; }
                    }
                }
            }
            b"package" => rule.package = Some(text(r)?),
            b"cmd" => { rule.cmd = true; r.skip().map_err(bad)?; }
            _ => { rule.other = true; r.skip().map_err(bad)?; }
        }
    }
    Ok(rule)
}

/// The package directory `manifest.json` names for `operation` on a machine of `arch` (the heartbeat's `x64`, `arm64`).
/// Its `mindcore` entries in that list must each have `when` (`os`, `arch` and nothing else) and `package` (the
/// directory their arch maps to) and no `cmd` or other field, with one entry an architecture at most; other platforms'
/// entries are the server's and are passed over.
pub fn select(json: &[u8], operation: Operation, arch: &str) -> Result<&'static str, Refused> {
    let bad = |_| refused(Code::Format, "manifest.json is not a JSON object of the contract's form");
    let own = directory(arch).ok_or_else(|| refused(Code::Arch, format!("no package directory for {}", arch)))?;
    let mut r = Reader::new(json).map_err(bad)?;
    let (mut version, mut found, mut seen) = (None, None::<&'static str>, Vec::<String>::new());
    r.object().map_err(bad)?;
    let mut key = [0u8; 32];
    while let Some(k) = r.key(&mut key).map_err(bad)? {
        match k {
            b"manifest_version" => version = Some(r.integer().map_err(bad)?),
            b"entrypoints" => {
                r.object().map_err(bad)?;
                let mut list = [0u8; 32];
                while let Some(l) = r.key(&mut list).map_err(bad)? {
                    if l != operation.list() { r.skip().map_err(bad)?; continue; }
                    r.array().map_err(bad)?;
                    while r.item().map_err(bad)? {
                        let rule = rule(&mut r)?;
                        if rule.os.as_deref() != Some("mindcore") { continue; }
                        let arch_of = rule.arch.clone().unwrap_or_default();
                        if rule.cmd || rule.other { return Err(refused(Code::Format, format!("manifest.json: the mindcore entry for {} has a field beside when and package", arch_of))); }
                        let Some(dir) = directory(&arch_of) else { return Err(refused(Code::Format, format!("manifest.json: a mindcore entry for {:?}", arch_of))) };
                        if rule.package.as_deref() != Some(dir) { return Err(refused(Code::Format, format!("manifest.json: the {} entry's package is not {}", arch_of, dir))); }
                        if seen.contains(&arch_of) { return Err(refused(Code::Format, format!("manifest.json: two mindcore entries for {}", arch_of))); }
                        seen.push(arch_of);
                        if dir == own { found = Some(dir); }
                    }
                }
            }
            _ => r.skip().map_err(bad)?,
        }
    }
    r.end().map_err(bad)?;
    if version != Some(1) { return Err(refused(Code::Format, "manifest.json: manifest_version is not 1")); }
    found.ok_or_else(|| refused(Code::Arch, format!("manifest.json has no mindcore entry for {} to {}", arch, core::str::from_utf8(operation.list()).unwrap_or(""))))
}

/// One file of a checked package: its path under the version's directory, size and SHA-256.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File { pub path: String, pub size: u64, pub sha256: [u8; 32] }

/// What `Verify` found in a package whose signature and version passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked { pub name: String, pub version: u64, pub files: Vec<File> }

/// The application installed already: its version and its MANIFEST's SHA-256.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Installed { pub version: u64, pub manifest: [u8; 32] }

/// The checks `apppkg` makes, under the owner's policy: the name a MANIFEST gives, then its signature under a key the
/// policy trusts for that name, this machine's architecture and release, and the version against the installed one.
pub trait Verify {
    fn name(&mut self, manifest: &[u8]) -> Result<String, Refused>;
    fn verify(&mut self, manifest: &[u8], signature: &[u8; 64], installed: Option<Installed>) -> Result<Checked, Refused>;
}

/// The gateway's writable scope of `data/apps/`: paths are below it, `/`-separated.
pub trait Apps {
    /// A file's bytes, at most `max`; None if there is none.
    fn read(&mut self, path: &str, max: usize) -> Result<Option<Vec<u8>>, Code>;
    /// A file written whole and flushed, its directories made as needed.
    fn write(&mut self, path: &str, data: &[u8]) -> Result<(), Code>;
    /// A file or directory renamed; an existing file at `to` is replaced.
    fn rename(&mut self, from: &str, to: &str) -> Result<(), Code>;
    /// A file, or a directory with all it holds; nothing there is no error.
    fn remove(&mut self, path: &str) -> Result<(), Code>;
    fn exists(&mut self, path: &str) -> bool;
}

/// What a package did: the `installed` or `removed` line's fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report { pub name: String, pub version: u64, pub previous: Option<u64>, pub arch: &'static str, pub files: u32, pub manifest: [u8; 32] }

impl Report {
    /// The result's line: `installed NAME VERSION ARCH FILES MANIFEST_SHA256` or `removed NAME VERSION`.
    pub fn line(&self, operation: Operation) -> String {
        match operation {
            Operation::Deinstall => format!("removed {} {}", self.name, self.previous.map_or(String::from("none"), |v| format!("{}", v))),
            _ => format!("installed {} {} {} {} {}", self.name, self.version, self.arch, self.files, hex(&self.manifest)),
        }
    }
}

/// The refusal's line: `refused CODE: what was checked`.
pub fn refusal_line(r: &Refused) -> String { format!("refused {}: {}", r.code.name(), r.what) }

pub fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{:02x}", b)).collect() }

fn sha(data: &[u8]) -> [u8; 32] { let mut h = Sha256::new(); h.update(data); h.finish() }

// An entry of the archive read whole, at most `max` bytes.
fn entry(archive: &Archive<'_>, name: &str, max: usize) -> Result<Vec<u8>, Refused> {
    let e = archive.find(name).ok_or_else(|| refused(Code::Format, format!("{} is not in the archive", name)))?;
    if e.size > max as u64 { return Err(refused(Code::Format, format!("{} is larger than {} bytes", name, max))); }
    let mut out = Vec::with_capacity(e.size as usize);
    archive.read(&e, &mut |b| { out.extend_from_slice(b); true }).map_err(|err| refused(Code::Format, format!("{}: {:?}", name, err)))?;
    Ok(out)
}

// The installed version of `name`: `current`'s version and that version's MANIFEST.
fn installed(apps: &mut dyn Apps, name: &str) -> Result<Option<Installed>, Refused> {
    let io = |c: Code| refused(c, format!("{}/current not read", name));
    let Some(current) = apps.read(&format!("{}/current", name), 32).map_err(io)? else { return Ok(None) };
    let text = core::str::from_utf8(&current).ok().and_then(|t| t.strip_suffix('\n')).filter(|t| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()));
    let Some(version) = text.and_then(|t| t.parse::<u64>().ok()) else { return Err(refused(Code::Io, format!("{}/current does not name a version", name))) };
    let manifest = apps.read(&format!("{}/{}/MANIFEST", name, version), MANIFEST_MAX).map_err(io)?
        .ok_or_else(|| refused(Code::Io, format!("{}/{}/MANIFEST is missing", name, version)))?;
    Ok(Some(Installed { version, manifest: sha(&manifest) }))
}

/// Does `operation` with the archive: the entry for this `arch`, the package checked, then written beside the installed
/// version and switched to in one rename of `current` (install, reinstall), or the application removed, `current`
/// first (deinstall). The previous version stays after an install.
pub fn run(archive: &[u8], operation: Operation, arch: &str, verify: &mut dyn Verify, apps: &mut dyn Apps) -> Result<Report, Refused> {
    let archive = Archive::open(archive, LIMITS).map_err(|e| refused(Code::Format, format!("the ZIP: {:?}", e)))?;
    let json = entry(&archive, "manifest.json", JSON_MAX)?;
    let dir = select(&json, operation, arch)?;
    let manifest = entry(&archive, &format!("{}/MANIFEST", dir), MANIFEST_MAX)?;
    let signature: [u8; 64] = entry(&archive, &format!("{}/MANIFEST.SIG", dir), 64)?.try_into()
        .map_err(|_| refused(Code::Format, "MANIFEST.SIG is not 64 bytes"))?;
    let name = verify.name(&manifest)?;
    let before = installed(apps, &name)?;
    let arch_name = &dir["mindcore/".len()..];
    if operation == Operation::Deinstall {
        // The signature shows the remover holds the package; the version is not compared.
        let checked = verify.verify(&manifest, &signature, None)?;
        let report = Report { name: name.clone(), version: checked.version, previous: before.map(|b| b.version), arch: arch_name, files: checked.files.len() as u32, manifest: sha(&manifest) };
        if before.is_some() {
            apps.remove(&format!("{}/current", name)).map_err(|c| refused(c, format!("{}/current not removed", name)))?;
            apps.remove(&name).map_err(|c| refused(c, format!("{} not removed", name)))?;
        }
        return Ok(report);
    }
    let checked = verify.verify(&manifest, &signature, before)?;
    // Each file read from the archive and checked against its MANIFEST line before it is written.
    let (staging, home) = (format!("{}/.new-{}", name, checked.version), format!("{}/{}", name, checked.version));
    apps.remove(&staging).map_err(|c| refused(c, "the staging directory not cleared"))?;
    let written = (|| {
        for f in &checked.files {
            let data = entry(&archive, &format!("{}/{}", dir, f.path), f.size as usize)?;
            if data.len() as u64 != f.size || sha(&data) != f.sha256 { return Err(refused(Code::Digest, format!("{} differs from its MANIFEST line", f.path))); }
            apps.write(&format!("{}/{}", staging, f.path), &data).map_err(|c| refused(c, format!("{} not written", f.path)))?;
        }
        apps.write(&format!("{}/MANIFEST", staging), &manifest).map_err(|c| refused(c, "MANIFEST not written"))?;
        apps.write(&format!("{}/MANIFEST.SIG", staging), &signature).map_err(|c| refused(c, "MANIFEST.SIG not written"))?;
        // The same version again (a reinstall): the old copy steps aside until the new one has its place.
        let aside = format!("{}/.old-{}", name, checked.version);
        if apps.exists(&home) {
            apps.remove(&aside).map_err(|c| refused(c, "the old copy's place not cleared"))?;
            apps.rename(&home, &aside).map_err(|c| refused(c, "the old copy not set aside"))?;
        }
        if let Err(c) = apps.rename(&staging, &home) {
            // The old copy back where `current` names it.
            if apps.exists(&aside) { let _ = apps.rename(&aside, &home); }
            return Err(refused(c, format!("{} not put in place", home)));
        }
        let _ = apps.remove(&aside);
        Ok(())
    })();
    if let Err(r) = written { let _ = apps.remove(&staging); return Err(r); }
    // The switch: `current` names the new version, in one rename.
    let next = format!("{}/current.new", name);
    apps.write(&next, format!("{}\n", checked.version).as_bytes()).map_err(|c| refused(c, "current.new not written"))?;
    apps.rename(&next, &format!("{}/current", name)).map_err(|c| refused(c, "current not switched"))?;
    Ok(Report { name, version: checked.version, previous: before.map(|b| b.version), arch: arch_name, files: checked.files.len() as u32, manifest: sha(&manifest) })
}
