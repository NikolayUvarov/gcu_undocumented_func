// blocks vector (312-STO-0047, docs/storage): embeddings in a journal and a vector index of one model's (mind::vector),
// a tree under a name of its own, `vector/<journal>` unless one is given. `add` appends an embedding the caller gives,
// components as signed bytes; `build` makes the index from the journal's head; `near` answers the k nearest by the dot
// product, each answer checked against the journal and the entries above the index's head scored; `scan` answers without
// the index; `check` rebuilds it and compares. The store runs no model: the vectors are the caller's.
use mind::dag::CHUNK;
use mind::index::{State, Verdict};
use mind::journal;
use mind::vector::{self, Embedding, Error, Kept, Near, Work, DIMS};

use crate::journal::{now, steps, stopped};
use crate::{cid, Remote};

const USAGE: &str = "Usage: blocks vector add <journal> <cid> <model> <vector> | build <journal> <model> <dims> [name] | near <journal> <model> <vector> <k> [name] | scan <journal> <model> <vector> <k> | check <journal> [name]";
// The source of every embedding `blocks` appends.
const SOURCE: &str = "blocks";
// Times an append reads the head again after a conflict.
const TRIES: usize = 8;
// The files a build keeps for its directory, here rather than on the stack; the blocks are the text index's.
static mut KEPT: [Kept; vector::FILES] = [Kept::EMPTY; vector::FILES];

pub fn command<'a>(mut words: impl Iterator<Item = &'a str>, buffer: &mut [u8; CHUNK], remote: &mut Remote) {
    let w: [Option<&str>; 7] = core::array::from_fn(|_| words.next());
    let mut components = [0u8; DIMS];
    match w {
        [Some("add"), Some(journal), Some(subject), Some(model), Some(text), None, _] => {
            let (Some(subject), Some(model), Some(dims)) = (cid(subject), cid(model), parse(text, &mut components)) else { return };
            add(journal, &Embedding { subject, model, vector: &components[..dims] }, buffer, remote)
        }
        [Some("build"), Some(journal), Some(model), Some(dims), name, None, _] => {
            let Some(model) = cid(model) else { return };
            let Ok(dims) = dims.parse::<usize>() else { return mind::println!("blocks: vector build: dimensions, 1 to {}", DIMS) };
            let mut named = [0u8; 128];
            let Some(name) = named_or(name, journal, &mut named) else { return };
            with_work(|work| build(journal, name, &model, dims, work, remote))
        }
        [Some(what @ ("near" | "scan")), Some(journal), Some(model), Some(text), Some(k), name, None] if what == "near" || name.is_none() => {
            let (Some(model), Some(dims)) = (cid(model), parse(text, &mut components)) else { return };
            let Ok(k) = k.parse::<usize>() else { return mind::println!("blocks: vector {}: answers, 1 to {}", what, vector::NEAREST) };
            let mut named = [0u8; 128];
            let Some(name) = named_or(name, journal, &mut named) else { return };
            with_work(|work| near(what, journal, name, &model, &components[..dims], k, work, remote))
        }
        [Some("check"), Some(journal), name, None, ..] => {
            let mut named = [0u8; 128];
            let Some(name) = named_or(name, journal, &mut named) else { return };
            with_work(|work| check(journal, name, work, remote))
        }
        _ => mind::println!("{}", USAGE),
    }
}

// The components of a vector, signed bytes separated by commas, into `out`; how many, or none, said why.
fn parse(text: &str, out: &mut [u8; DIMS]) -> Option<usize> {
    let mut n = 0;
    for part in text.split(',') {
        match part.parse::<i8>() {
            Ok(v) if n < DIMS => { out[n] = v as u8; n += 1; }
            _ => { mind::println!("blocks: vector: 1 to {} components, each -128 to 127, separated by commas", DIMS); return None }
        }
    }
    Some(n)
}

// `vector/<journal>` unless a name is given; the store refuses one longer than its names.
fn named_or<'a>(name: Option<&'a str>, journal: &str, named: &'a mut [u8; 128]) -> Option<&'a str> {
    match name {
        Some(name) => Some(name),
        None if journal.len() + 7 <= named.len() => {
            named[..7].copy_from_slice(b"vector/");
            named[7..7 + journal.len()].copy_from_slice(journal.as_bytes());
            Some(core::str::from_utf8(&named[..7 + journal.len()]).unwrap_or(""))
        }
        None => { mind::println!("blocks: vector: a journal's name of at most 64 bytes"); None }
    }
}

fn with_work(run: impl FnOnce(&mut Work)) {
    let mut work = unsafe {
        use core::ptr::addr_of_mut;
        Work::new(&mut *addr_of_mut!(KEPT), &mut *addr_of_mut!(crate::index::BLOCKS))
    };
    run(&mut work)
}

fn refused(what: &str, journal: &str, error: Error, remote: &Remote) {
    match (error, remote.last) {
        (Error::Journal(r), _) => stopped(journal, r, remote),
        (Error::Store | Error::Append(journal::Error::Store), Some(e)) => mind::println!("blocks: {} {}: {:?} (the store: {:?})", what, journal, error, e),
        (e, _) => mind::println!("blocks: {} {}: {:?}", what, journal, e),
    }
}

// add: the entry's sequence and CID, and the embedding's CID.
fn add(journal: &str, e: &Embedding, buffer: &mut [u8; CHUNK], remote: &mut Remote) {
    for _ in 0..TRIES {
        match vector::add(remote, journal, now(), SOURCE, e, buffer) {
            Ok(done) => return mind::println!("EMBEDDED {} SEQUENCE {} ENTRY {} EMBEDDING {} OF {} BY {}, {} DIMS ({})", journal, done.seq, done.entry, done.cid, e.subject, e.model, e.vector.len(), mind::blockstore::class(remote.durability)),
            Err(Error::Append(journal::Error::Conflict)) => mind::println!("CONFLICT {}: ANOTHER WRITER APPENDED; READING THE HEAD AGAIN", journal),
            Err(error) => return refused("vector add", journal, error, remote),
        }
    }
    mind::println!("blocks: vector add {}: {} conflicts; nothing appended", journal, TRIES);
}

// build: the count and the head it was built from, then the name's version and root.
fn build(journal: &str, name: &str, model: &mind::cid::Cid, dims: usize, work: &mut Work, remote: &mut Remote) {
    let Some(steps) = steps() else { return };
    match vector::build(remote, journal, name, model, dims, steps, work) {
        Ok(built) => {
            let (i, m) = (built.index, built.index.manifest);
            mind::println!("INDEXED {} TO SEQUENCE {}: {} VECTORS OF {} DIMS BY {}", journal, m.seq, m.entries, m.dims, m.model);
            if built.published {
                mind::println!("PUBLISHED {} VERSION {} ROOT {} ({})", name, i.version, i.root, mind::blockstore::class(remote.durability));
            } else {
                mind::println!("UNCHANGED {} VERSION {} ROOT {}", name, i.version, i.root);
            }
        }
        Err(e) => refused("vector build", journal, e, remote),
    }
}

// near and scan: the answers, highest first, once every one is checked; then the index's state and the counts.
#[allow(clippy::too_many_arguments)]
fn near(what: &str, journal: &str, name: &str, model: &mind::cid::Cid, query: &[u8], k: usize, work: &mut Work, remote: &mut Remote) {
    let Some(steps) = steps() else { return };
    let print = |n: &Near| mind::println!("NEAR {} SCORE {} SUBJECT {}", n.seq, n.score, n.subject);
    if what == "scan" {
        return match vector::scan(remote, journal, model, query, k, steps, work, print) {
            Ok(scored) => mind::println!("NEAREST {} OF {} VECTORS IN {}, AN EXACT SCAN", k.min(scored as usize), scored, journal),
            Err(e) => refused("vector scan", journal, e, remote),
        };
    }
    match vector::nearest(remote, journal, name, model, query, k, steps, work, print) {
        Ok(f) => {
            let seq = f.index.manifest.seq;
            match f.state {
                State::Current => mind::println!("INDEX {} OF {} AT SEQUENCE {}: CURRENT", name, journal, seq),
                State::Stale(lacks) => mind::println!("INDEX {} OF {} AT SEQUENCE {}: STALE, LACKS {} ENTRIES, SCANNED", name, journal, seq, lacks),
            }
            mind::println!("NEAREST {} OF {} VECTORS IN {}, EACH CHECKED AGAINST THE JOURNAL", f.nearest, f.vectors, journal);
        }
        Err(e) => refused("vector near", journal, e, remote),
    }
}

// check: the index rebuilt from its own head and compared, `EQUAL`, `STALE <n>` or `DIFFERS`.
fn check(journal: &str, name: &str, work: &mut Work, remote: &mut Remote) {
    let Some(steps) = steps() else { return };
    match vector::verify(remote, journal, name, steps, work) {
        Ok((i, Verdict::Equal)) => mind::println!("INDEX {} OF {} AT SEQUENCE {}: EQUAL", name, journal, i.manifest.seq),
        Ok((i, Verdict::Stale(n))) => mind::println!("INDEX {} OF {} AT SEQUENCE {}: STALE {}", name, journal, i.manifest.seq, n),
        Ok((i, Verdict::Differs)) => mind::println!("INDEX {} OF {} AT SEQUENCE {}: DIFFERS", name, journal, i.manifest.seq),
        Err(e) => refused("vector check", journal, e, remote),
    }
}
