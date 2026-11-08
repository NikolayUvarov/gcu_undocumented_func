// The releases the updater keeps in the block store (351-UPD-0033, docs/update/updater.md; MC-9.3). Each one is a tree
// of the release layout (scripts/release.py): its channel, this processor's manifest and signature, and the blobs that
// manifest lists for a slot. The blobs lie under the first two digits of their names (`blobs/ab/ab…`), so that no
// directory is over a block. The updater pins each tree with its own owner (`mind::blockstore::OWNER_UPDATER`), whose
// pins no other client ends. It names none: a name keeps its versions, which would hold a release past its pin. A kept
// release is read back as any source, every block checked against its CID, and nothing read is trusted until the
// updater has checked it. No system calls: tests/updater_host.rs.
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::cid::{Cid, Codec};
use crate::dag::{Blocks, Builder, Error, CHUNK};
use crate::tree::{self, Entry, Kind};

/// A blob of a release: its SHA-256, size and object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blob { pub digest: [u8; 32], pub size: u64, pub cid: Cid }

/// The small files of a release: the channel's name and file, the version, this processor's name, its manifest and the
/// manifest's signature.
pub struct Release<'a> { pub channel: &'a str, pub channel_file: &'a [u8], pub version: u64, pub arch: &'a str, pub manifest: &'a [u8], pub signature: &'a [u8] }

fn hex(digest: &[u8; 32]) -> String { digest.iter().map(|b| format!("{:02x}", b)).collect() }

fn unhex(text: &[u8]) -> Option<[u8; 32]> {
    if text.len() != 64 { return None; }
    let digit = |c: u8| match c { b'0'..=b'9' => Some(c - b'0'), b'a'..=b'f' => Some(c - b'a' + 10), _ => None };
    let mut out = [0u8; 32];
    for (k, pair) in text.chunks_exact(2).enumerate() { out[k] = digit(pair[0])? << 4 | digit(pair[1])?; }
    Some(out)
}

/// Where the release layout's `path` lies in a kept tree: a blob under the first two digits of its name, the rest as
/// they are.
pub fn stored(path: &str) -> String {
    match path.strip_prefix("blobs/") {
        Some(name) if name.len() > 2 && name.is_char_boundary(2) => format!("blobs/{}/{}", &name[..2], name),
        _ => String::from(path),
    }
}

// Puts a directory of `entries`, sorted by name, and checks the CID the store returns.
fn directory<B: Blocks>(blocks: &mut B, entries: &[Entry], buffer: &mut [u8; CHUNK]) -> Result<Cid, Error> {
    let len = tree::encode(entries, buffer)?;
    let cid = blocks.put(Codec::DagCbor, &buffer[..len])?;
    if cid != Cid::of(Codec::DagCbor, &buffer[..len]) { return Err(Error::Corrupt); }
    Ok(cid)
}

// Puts `bytes` as a file; its object and size.
fn file<B: Blocks>(blocks: &mut B, builder: &mut Builder, bytes: &[u8]) -> Result<(Cid, u64), Error> {
    builder.reset();
    builder.write(blocks, bytes)?;
    builder.finish(blocks)
}

/// Puts `release` and its `blobs`, already in the store, as a tree, and returns its root. A digest given twice is one
/// blob.
pub fn put<B: Blocks>(blocks: &mut B, builder: &mut Builder, release: &Release, blobs: &[Blob], buffer: &mut [u8; CHUNK]) -> Result<Cid, Error> {
    let mut groups: BTreeMap<String, BTreeMap<String, Blob>> = BTreeMap::new();
    for blob in blobs {
        let name = hex(&blob.digest);
        groups.entry(String::from(&name[..2])).or_default().insert(name, *blob);
    }
    let mut prefixes = Vec::with_capacity(groups.len());
    for (prefix, group) in &groups {
        let entries: Vec<Entry> = group.iter().map(|(name, b)| Entry { name: name.as_bytes(), kind: Kind::File, size: b.size, cid: b.cid }).collect();
        prefixes.push((prefix, entries.len() as u64, directory(blocks, &entries, buffer)?));
    }
    let entries: Vec<Entry> = prefixes.iter().map(|(name, count, cid)| Entry { name: name.as_bytes(), kind: Kind::Directory, size: *count, cid: *cid }).collect();
    let blobs = directory(blocks, &entries, buffer)?;
    let (manifest, manifest_size) = file(blocks, builder, release.manifest)?;
    let (signature, signature_size) = file(blocks, builder, release.signature)?;
    let arch = directory(blocks, &[Entry { name: b"MANIFEST", kind: Kind::File, size: manifest_size, cid: manifest },
                                   Entry { name: b"MANIFEST.SIG", kind: Kind::File, size: signature_size, cid: signature }], buffer)?;
    let version = format!("{}", release.version);
    let numbered = directory(blocks, &[Entry { name: release.arch.as_bytes(), kind: Kind::Directory, size: 2, cid: arch }], buffer)?;
    let releases = directory(blocks, &[Entry { name: version.as_bytes(), kind: Kind::Directory, size: 1, cid: numbered }], buffer)?;
    let (channel, channel_size) = file(blocks, builder, release.channel_file)?;
    let channels = directory(blocks, &[Entry { name: release.channel.as_bytes(), kind: Kind::File, size: channel_size, cid: channel }], buffer)?;
    directory(blocks, &[Entry { name: b"blobs", kind: Kind::Directory, size: prefixes.len() as u64, cid: blobs },
                        Entry { name: b"channels", kind: Kind::Directory, size: 1, cid: channels },
                        Entry { name: b"releases", kind: Kind::Directory, size: 1, cid: releases }], buffer)
}

// A directory read and checked: against its CID, and its entries against the count its parent gives (none: the root).
fn read<'b, B: Blocks>(blocks: &mut B, cid: &Cid, count: Option<u64>, buffer: &'b mut [u8; CHUNK]) -> Result<tree::Dir<'b>, Error> {
    if cid.codec() != Codec::DagCbor { return Err(Error::Shape); }
    let len = blocks.get(cid, buffer)?;
    if !cid.matches(&buffer[..len]) { return Err(Error::Corrupt); }
    let buffer: &'b [u8; CHUNK] = buffer;
    let dir = tree::decode(&buffer[..len])?;
    if count.is_some_and(|n| n != dir.len() as u64) { return Err(Error::Shape); }
    Ok(dir)
}

/// What the layout's `path` names in the kept tree `root`: a file's object and size, each directory on the way checked;
/// none where it holds no such file.
pub fn find<B: Blocks>(blocks: &mut B, root: &Cid, path: &str, buffer: &mut [u8; CHUNK]) -> Result<Option<(Cid, u64)>, Error> {
    let path = stored(path);
    let (mut cid, mut count) = (*root, None);
    let mut names = path.split('/').peekable();
    while let Some(name) = names.next() {
        let Some(e) = read(blocks, &cid, count, buffer)?.find(name.as_bytes()) else { return Ok(None) };
        match (e.kind, names.peek().is_some()) {
            (Kind::Directory, true) => (cid, count) = (e.cid, Some(e.size)),
            (Kind::File, false) => return Ok(Some((e.cid, e.size))),
            _ => return Ok(None),
        }
    }
    Ok(None)
}

// The one entry of the directory `name` at the root of `root`: its name and what it names.
fn only<B: Blocks>(blocks: &mut B, root: &Cid, name: &[u8], buffer: &mut [u8; CHUNK]) -> Result<(String, Kind, Cid, u64), Error> {
    let e = read(blocks, root, None, buffer)?.find(name).ok_or(Error::Shape)?;
    if e.kind != Kind::Directory || e.size != 1 { return Err(Error::Shape); }
    let (cid, count) = (e.cid, e.size);
    let e = read(blocks, &cid, Some(count), buffer)?.entry(0);
    Ok((String::from(core::str::from_utf8(e.name).map_err(|_| Error::Shape)?), e.kind, e.cid, e.size))
}

/// The release a kept tree holds: its version, the one directory under `releases/`, with a manifest for `arch`; else
/// Shape.
pub fn version<B: Blocks>(blocks: &mut B, root: &Cid, arch: &str, buffer: &mut [u8; CHUNK]) -> Result<u64, Error> {
    let (name, kind, ..) = only(blocks, root, b"releases", buffer)?;
    // A name that is not the number written plainly (`05`) holds no `releases/5/`: the lookup refuses it.
    let version = name.parse::<u64>().ok().filter(|v| *v > 0 && kind == Kind::Directory).ok_or(Error::Shape)?;
    find(blocks, root, &format!("releases/{}/{}/MANIFEST", version, arch), buffer)?.ok_or(Error::Shape)?;
    Ok(version)
}

/// The name of a kept tree's channel, the one file under `channels/`; else Shape.
pub fn channel<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK]) -> Result<String, Error> {
    match only(blocks, root, b"channels", buffer)? { (name, Kind::File, ..) => Ok(name), _ => Err(Error::Shape) }
}

/// Each blob a kept tree holds, by its digest; at most `left` entries are read, each directory checked.
pub fn blobs<B: Blocks>(blocks: &mut B, root: &Cid, buffer: &mut [u8; CHUNK], left: &mut u64, mut visit: impl FnMut(Blob)) -> Result<(), Error> {
    let mut path = [0u8; 2 * 66 + 16];
    let Some(e) = read(blocks, root, None, buffer)?.find(b"blobs") else { return Ok(()) };
    if e.kind != Kind::Directory { return Err(Error::Shape); }
    // `files` reads its first directory with no count: the root's is checked here.
    let (cid, count) = (e.cid, e.size);
    read(blocks, &cid, Some(count), buffer)?;
    // Each digest under the prefix its name starts with: a blob put under another one is not the release's.
    tree::files(blocks, &cid, buffer, &mut path, left, |_, at, size, object| {
        let (prefix, name) = at.split_at(at.iter().position(|&b| b == b'/').ok_or(Error::Shape)?);
        let digest = unhex(&name[1..]).filter(|_| name.len() == 65 && name[1..3] == *prefix).ok_or(Error::Shape)?;
        visit(Blob { digest, size, cid: *object });
        Ok(())
    })?;
    Ok(())
}
