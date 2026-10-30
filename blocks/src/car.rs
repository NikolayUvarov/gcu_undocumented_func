// blocks export | import (309-STO-0031, docs/storage): named roots copied into a file as a CAR v1 (mind::car), and back.
// An import checks the whole file before it puts anything, puts each block (a copy the store holds damaged is stored
// again, and a collection frees the damaged one), and publishes each name by compare-and-swap, never over another root.
use mind::car::{self, Outcome, Refused, Source, Target};
use mind::cid::Cid;
use mind::dag::{self, CHUNK};
use mind::fs::File;
use mind::idl::blockstore;
use mind::idl::codec::Text;

use crate::models::hex;
use crate::{report, store, store_silent, Remote};

const USAGE: &str = "Usage: blocks export <file> <name>... | import [--check] <file>";

/// The store as copies need it: names read at one point, and a collection.
impl car::Store for Remote {
    fn snapshot(&mut self, names: &[&str], heads: &mut [(u64, Option<Cid>)]) -> Result<(), dag::Error> {
        let mut texts = [Text::<64>::default(); car::NAMES];
        for (text, name) in texts.iter_mut().zip(names) { *text = Text::new(name).ok_or(dag::Error::Store)?; }
        match blockstore::snapshot(store(), &texts[..names.len()]) {
            Ok(Ok(got)) if got.as_slice().len() == names.len() => {
                for (head, got) in heads.iter_mut().zip(got.as_slice()) { *head = (got.version, Cid::from_bytes(got.root.as_slice()).ok()); }
                Ok(())
            }
            Ok(Ok(_)) | Err(_) => Err(dag::Error::Store),
            Ok(Err(e)) => Err(self.fail(e)),
        }
    }
    fn collect(&mut self) -> Result<u64, dag::Error> {
        match blockstore::collect(store()) { Ok(Ok(c)) => Ok(c.blocks as u64), Ok(Err(e)) => Err(self.fail(e)), Err(_) => Err(dag::Error::Store) }
    }
}

// A copy read from a file of the user's. The file system takes offsets of 32 bits.
struct Input(File);
impl Source for Input {
    fn read_at(&mut self, at: u64, out: &mut [u8]) -> Result<usize, car::Error> {
        let Ok(at) = u32::try_from(at) else { return Ok(0) };
        let most = out.len().min((u32::MAX - at) as usize);
        let out = &mut out[..most];
        self.0.read_at(at as usize, out).map_err(|_| car::Error::File)
    }
}

// A copy written to a file, made at the first write: a refusal before it leaves a file of that name as it was.
struct Output<'p> { path: &'p str, file: Option<File>, size: u64 }
impl Output<'_> {
    // Written whole and flushed; else the part written, no copy, goes.
    fn close(self, whole: bool) -> bool {
        let Some(file) = self.file else { return false };
        if whole && file.flush().is_ok() { return true; }
        drop(file);
        let _ = mind::fs::remove(self.path);
        false
    }
}
impl Target for Output<'_> {
    fn write(&mut self, data: &[u8]) -> Result<(), car::Error> {
        if self.size + data.len() as u64 > u32::MAX as u64 { return Err(car::Error::File); }
        if self.file.is_none() { self.file = Some(File::create(self.path).map_err(|_| car::Error::File)?); }
        let file = self.file.as_mut().unwrap();
        if file.write(data).ok() != Some(data.len()) { return Err(car::Error::File); }
        self.size += data.len() as u64;
        Ok(())
    }
}

pub fn command<'a>(which: &str, mut words: impl Iterator<Item = &'a str>, buffer: &mut [u8; CHUNK], data: &mut [u8; CHUNK], remote: &mut Remote) {
    match (which, words.next()) {
        ("export", Some(path)) => {
            // One more than a copy holds, so that many is refused by mind::car as too many.
            let mut names = [""; car::NAMES + 1];
            let mut n = 0;
            for name in words {
                if n == names.len() { mind::println!("blocks: export: at most {} names", car::NAMES); return; }
                names[n] = name;
                n += 1;
            }
            export(path, &names[..n], buffer, data, remote)
        }
        ("import", Some("--check")) => match (words.next(), words.next()) {
            (Some(path), None) => check(path, buffer, data),
            _ => mind::println!("{}", USAGE),
        },
        ("import", Some(path)) if words.next().is_none() => import(path, buffer, data, remote),
        _ => mind::println!("{}", USAGE),
    }
}

// export: each name's version and root, then the blocks and bytes written and the file's SHA-256.
fn export(path: &str, names: &[&str], buffer: &mut [u8; CHUNK], data: &mut [u8; CHUNK], remote: &mut Remote) {
    // A walk takes the steps the store gives its own: four for each block its index can hold, and 64.
    let steps = match blockstore::stat(store()) {
        Ok(Ok(s)) => 4 * s.capacity as u64 + 64,
        Ok(Err(e)) => return mind::println!("blocks: export: {:?}", e),
        Err(e) => return store_silent(e),
    };
    let mut output = Output { path, file: None, size: 0 };
    let done = car::export(remote, names, &mut output, steps, buffer, data, |name, version, root| match root {
        Some(root) => mind::println!("EXPORT {} VERSION {} ROOT {}", name, version, root),
        None if version == 0 => mind::println!("blocks: export: {}: no such name", name),
        None => mind::println!("blocks: export: {}: removed at version {}", name, version),
    }).map(|(_, written)| written);
    let whole = output.close(done.is_ok());
    match done {
        Ok(w) if whole => mind::println!("EXPORTED {} NAMES, {} BLOCKS, {} BYTES TO {}; SHA-256 {}", w.names, w.blocks, w.bytes, path, hex(&w.digest)),
        Ok(_) => mind::println!("blocks: export: writing {} failed; it is removed", path),
        Err(car::Error::Name) => mind::println!("blocks: export: 1 to {} names the store takes, each once", car::NAMES),
        Err(car::Error::Missing) => mind::println!("blocks: export: nothing written"),
        Err(car::Error::Store(e)) => report("export", e, remote),
        Err(e) => mind::println!("blocks: export: {:?}", e),
    }
}

fn refused(what: &str, path: &str, r: Refused) { mind::println!("blocks: {}: {} refused at byte {}: {:?}", what, path, r.at, r.error) }

// import --check: each name in the file, then its blocks, bytes and SHA-256; nothing put.
fn check(path: &str, buffer: &mut [u8; CHUNK], data: &mut [u8; CHUNK]) {
    let Ok(file) = File::open(path) else { return mind::println!("blocks: cannot open {}", path) };
    match car::check(Input(file), buffer, data, |n| mind::println!("NAME {} VERSION {} ROOT {}", n.name, n.version, n.root)) {
        Ok(c) => mind::println!("CHECKED {}: {} NAMES, {} BLOCKS, {} BYTES; SHA-256 {}", path, c.names, c.blocks, c.bytes, hex(&c.digest)),
        Err(r) => refused("import --check", path, r),
    }
}

// import: what became of each name, then the blocks stored, repaired and held; after a repair, the collection.
fn import(path: &str, buffer: &mut [u8; CHUNK], data: &mut [u8; CHUNK], remote: &mut Remote) {
    let Ok(file) = File::open(path) else { return mind::println!("blocks: cannot open {}", path) };
    let mut outcomes = [(Text::<64>::default(), None, Outcome::Same(0)); car::NAMES];
    let mut n = 0;
    let done = car::import(remote, Input(file), buffer, data, |named, outcome| {
        outcomes[n] = (Text::new(named.name).unwrap_or_default(), Some(named.root), outcome);
        n += 1;
    });
    for &(name, root, outcome) in &outcomes[..n] {
        let Some(root) = root else { continue };
        match outcome {
            Outcome::Published(v) => mind::println!("PUBLISHED {} VERSION {} ROOT {} ({})", name, v, root, mind::blockstore::class(remote.durability)),
            Outcome::Same(v) => mind::println!("SAME {} VERSION {} ROOT {}", name, v, root),
            Outcome::Conflict { version, root: Some(current) } => mind::println!("CONFLICT {}: VERSION {} ROOT {}, NOT {}; LEFT AS IT IS", name, version, current, root),
            Outcome::Conflict { version, root: None } => mind::println!("CONFLICT {}: VERSION {} REMOVED, NOT {}; LEFT AS IT IS", name, version, root),
        }
    }
    match done {
        Ok(i) => {
            let c = i.checked;
            mind::println!("IMPORTED {}: {} NAMES, {} BLOCKS ({} STORED, {} REPAIRED, {} HELD), {} BYTES; SHA-256 {}", path, c.names, c.blocks, i.stored, i.repaired, i.held, c.bytes, hex(&c.digest));
            match i.collected {
                Some(Ok(freed)) => mind::println!("COLLECTED {} BLOCKS: THE DAMAGED COPIES FREED", freed),
                Some(Err(e)) => report("import: collect", e, remote),
                None => {}
            }
        }
        Err(Refused { error: car::Error::Store(e), .. }) => report("import", e, remote),
        Err(r) => refused("import", path, r),
    }
}
