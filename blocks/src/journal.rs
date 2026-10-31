// blocks journal (312-STO-0034, docs/storage): an append-only journal under a name (mind::journal). Each append puts the
// content and an entry and publishes the entry by compare-and-swap from the version read; a conflict reads the head
// again and appends after it. `check` reads it whole from its head, `list` its last entries.
use mind::cid::{Cid, Codec};
use mind::dag::{self, Blocks, Builder, CHUNK};
use mind::fs::File;
use mind::idl::blockstore::{self, Error as Refusal};
use mind::idl::codec::Text;
use mind::journal::{self, Clock, Entry, Error, Reading, Refused};

use crate::{report, store, store_silent, Remote};

const USAGE: &str = "Usage: blocks journal append <name> <kind> <text> | append <name> <kind> -f <file> | check <name> | list <name> [n]";
// Times an append reads the head again after a conflict.
const TRIES: usize = 8;
// Bytes of a content `list` prints as text at most.
const SHOWN: usize = 96;

/// The store as a journal needs it.
impl journal::Store for Remote {
    fn put(&mut self, entry: &[u8]) -> Result<Cid, Error> { Blocks::put(self, Codec::DagCbor, entry).map_err(|_| Error::Store) }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> {
        Blocks::get(self, cid, out).map_err(|e| match e { dag::Error::NotFound => Error::Missing, dag::Error::Corrupt => Error::Corrupt, _ => Error::Store })
    }
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), Error> {
        let text = Text::<64>::new(name).ok_or(Error::Store)?;
        match blockstore::snapshot(store(), &[text]) {
            Ok(Ok(heads)) => heads.as_slice().first().map(|h| (h.version, Cid::from_bytes(h.root.as_slice()).ok())).ok_or(Error::Store),
            Ok(Err(e)) => { self.fail(e); Err(Error::Store) }
            Err(_) => Err(Error::Store),
        }
    }
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<u64, Error> {
        match blockstore::publish(store(), name, expected, &root.to_bytes()) {
            Ok(Ok(done)) => { self.durability = done.durability as u8; Ok(done.version) }
            Ok(Err(Refusal::Conflict)) => Err(Error::Conflict),
            Ok(Err(e)) => { self.fail(e); Err(if e == Refusal::Incomplete { Error::Missing } else { Error::Store }) }
            Err(_) => Err(Error::Store),
        }
    }
}

pub fn command<'a>(mut words: impl Iterator<Item = &'a str> + Clone, builder: &mut Builder, buffer: &mut [u8; CHUNK], data: &mut [u8; CHUNK], remote: &mut Remote) {
    match (words.next(), words.next()) {
        (Some("append"), Some(name)) => {
            let (Some(kind), rest) = (words.next(), words.clone()) else { return mind::println!("{}", USAGE) };
            let content = match (words.next(), words.next(), words.next()) {
                (Some("-f"), Some(path), None) => put_file(path, builder, data, remote),
                (Some(_), ..) => put_text(rest, data, remote),
                _ => return mind::println!("{}", USAGE),
            };
            if let Some(content) = content { append(name, kind, &content, buffer, remote); }
        }
        (Some("check"), Some(name)) if words.next().is_none() => check(name, buffer, remote),
        (Some("list"), Some(name)) => match (words.next().map(|n| n.parse::<u64>()), words.next()) {
            (None, None) => list(name, 10, buffer, data, remote),
            (Some(Ok(n)), None) => list(name, n, buffer, data, remote),
            _ => mind::println!("{}", USAGE),
        },
        _ => mind::println!("{}", USAGE),
    }
}

// The text's words joined by one space, as a raw block.
fn put_text<'a>(words: impl Iterator<Item = &'a str>, data: &mut [u8; CHUNK], remote: &mut Remote) -> Option<Cid> {
    let mut len = 0;
    for word in words {
        let sep = (len > 0) as usize;
        if len + sep + word.len() > CHUNK { mind::println!("blocks: journal append: a text of at most {} bytes", CHUNK); return None; }
        if sep == 1 { data[len] = b' '; }
        data[len + sep..len + sep + word.len()].copy_from_slice(word.as_bytes());
        len += sep + word.len();
    }
    match remote.put(Codec::Raw, &data[..len]) { Ok(cid) => Some(cid), Err(e) => { report("journal append", e, remote); None } }
}

// A file of any size, as an object (mind::dag).
fn put_file(path: &str, builder: &mut Builder, data: &mut [u8; CHUNK], remote: &mut Remote) -> Option<Cid> {
    let Ok(mut file) = File::open(path) else { mind::println!("blocks: cannot open {}", path); return None };
    builder.reset();
    loop {
        let n = match file.read(data) { Ok(n) => n, Err(e) => { mind::println!("blocks: reading {}: {:?}", path, e); return None } };
        if n == 0 { break; }
        if let Err(e) = builder.write(remote, &data[..n]) { report("journal append", e, remote); return None; }
    }
    match builder.finish(remote) { Ok((root, _)) => Some(root), Err(e) => { report("journal append", e, remote); None } }
}

// The clocks now: the monotonic one, and the calendar time from the clock chip if the rtc service gives it.
fn now() -> Reading {
    let monotonic = mind::time::monotonic_ns();
    match mind::rtc::local_now_in(mind::rtc::Zone::UTC) {
        // Days since 2000-01-01 and seconds since midnight; 10957 days from 1970-01-01.
        Some((days, seconds)) => Reading { monotonic, calendar: (days as u64 + 10957) * 86_400 + seconds as u64, clock: Clock::Chip },
        None => Reading { monotonic, calendar: 0, clock: Clock::None },
    }
}

fn append(name: &str, kind: &str, content: &Cid, buffer: &mut [u8; CHUNK], remote: &mut Remote) {
    for _ in 0..TRIES {
        match journal::append(remote, name, now(), "blocks", kind, content, buffer) {
            Ok(done) => {
                let e = done.entry;
                return mind::println!("APPENDED {} SEQUENCE {} BOOT {} ENTRY {} CONTENT {} ({})", name, e.seq, e.time.boot, done.cid, content, mind::blockstore::class(remote.durability));
            }
            Err(Error::Conflict) => mind::println!("CONFLICT {}: ANOTHER WRITER APPENDED; READING THE HEAD AGAIN", name),
            Err(Error::Invalid) => return mind::println!("blocks: journal append: a kind of 1 to {} printable bytes, no spaces", journal::LABEL),
            Err(e) => return refused("journal append", name, e, remote),
        }
    }
    mind::println!("blocks: journal append {}: {} conflicts; nothing appended", name, TRIES);
}

fn refused(what: &str, name: &str, error: Error, remote: &Remote) {
    match (error, remote.last) {
        (Error::Store, Some(e)) => mind::println!("blocks: {} {}: {:?} (the store: {:?})", what, name, error, e),
        _ => mind::println!("blocks: {} {}: {:?}", what, name, error),
    }
}

// Entries a check reads at most: the store's own bound on a walk, four for each block its index holds, and 64.
pub(crate) fn steps() -> Option<u64> {
    match blockstore::stat(store()) {
        Ok(Ok(s)) => Some(4 * s.capacity as u64 + 64),
        Ok(Err(e)) => { mind::println!("blocks: stat: {:?}", e); None }
        Err(e) => { store_silent(e); None }
    }
}

pub(crate) fn stopped(name: &str, r: Refused, remote: &Remote) {
    match r.error {
        Error::NotFound | Error::Removed(_) if r.checked == 0 => refused("journal", name, r.error, remote),
        e => mind::println!("REFUSED {} AT VERSION {}: SEQUENCE {}, AFTER {} ENTRIES CHECKED: {:?}", name, r.version, r.at, r.checked, e),
    }
}

// check: the count, the first and last sequence numbers, and the result.
fn check(name: &str, buffer: &mut [u8; CHUNK], remote: &mut Remote) {
    let Some(steps) = steps() else { return };
    match journal::check(remote, name, steps, buffer) {
        Ok(c) => mind::println!("CHECKED {}: {} ENTRIES, SEQUENCE {} TO {}, AT VERSION {}: WHOLE", name, c.entries, c.first, c.last, c.version),
        Err(r) => stopped(name, r, remote),
    }
}

// list: the last `most` entries, newest first, each on two lines; a short printable content as its text.
fn list(name: &str, most: u64, buffer: &mut [u8; CHUNK], data: &mut [u8; CHUNK], remote: &mut Remote) {
    let listed = journal::list(remote, name, most, buffer, |remote, e: &Entry, cid| {
        let clock = match e.time.clock { Clock::None => "NONE", Clock::Chip => "CHIP", Clock::Network => "NETWORK" };
        mind::println!("ENTRY {} {} BOOT {} MONOTONIC {} CALENDAR {} {}", e.seq, cid, e.time.boot, e.time.monotonic, e.time.calendar, clock);
        let text = match e.content.codec() {
            Codec::Raw => remote.get(&e.content, &mut data[..]).ok().filter(|&n| n <= SHOWN).and_then(|n| core::str::from_utf8(&data[..n]).ok())
                .filter(|t| t.bytes().all(|c| (0x20..0x7f).contains(&c))),
            Codec::DagCbor => None,
        };
        match text {
            Some(t) => mind::println!("  SOURCE {} KIND {} CONTENT {} TEXT {}", e.source, e.kind, e.content, t),
            None => mind::println!("  SOURCE {} KIND {} CONTENT {}", e.source, e.kind, e.content),
        }
    });
    match listed {
        Ok(c) => mind::println!("{} ENTRIES OF {} AT VERSION {}, NEWEST FIRST", c.entries, name, c.version),
        Err(r) => stopped(name, r, remote),
    }
}
