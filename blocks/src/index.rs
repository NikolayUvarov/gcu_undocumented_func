// blocks index (312-STO-0035, docs/storage): a text index of a journal (mind::index), a tree under a name of its own,
// `index/<journal>` unless one is given. `build` makes it from the journal's head; `search` answers a word from it, each
// answer checked against the journal and the entries above the index's head scanned; `check` rebuilds it and compares.
use mind::dag::CHUNK;
use mind::index::{self, Error, Posting, State, Term, Verdict, Work};

use crate::journal::{steps, stopped};
use crate::Remote;

// The work area: terms, their bytes, postings, and three blocks; about 430 KiB, kept here rather than on the stack.
const TERMS: usize = 4096;
const TEXT: usize = 64 * 1024;
const POSTINGS: usize = 32 * 1024;
static mut TERM_AREA: [Term; TERMS] = [Term::EMPTY; TERMS];
static mut TEXT_AREA: [u8; TEXT] = [0; TEXT];
static mut POSTING_AREA: [Posting; POSTINGS] = [Posting::EMPTY; POSTINGS];
static mut BLOCKS: [[u8; CHUNK]; 3] = [[0; CHUNK]; 3];
// Sequences on one line of a search's answer at most.
const LINE: usize = 16;

const USAGE: &str = "Usage: blocks index build <journal> [name] | search <journal> <word> [name] | check <journal> [name]";

pub fn command<'a>(mut words: impl Iterator<Item = &'a str>, remote: &mut Remote) {
    let w: [Option<&str>; 5] = core::array::from_fn(|_| words.next());
    let (what, journal, word, name) = match w {
        [Some(what @ ("build" | "check")), Some(journal), name, None, _] => (what, journal, "", name),
        [Some("search"), Some(journal), Some(word), name, None] => ("search", journal, word, name),
        _ => return mind::println!("{}", USAGE),
    };
    // `index/<journal>` unless a name is given; the store refuses one longer than its names.
    let mut named = [0u8; 128];
    let name = match name {
        Some(name) => name,
        None if journal.len() + 6 <= named.len() => {
            named[..6].copy_from_slice(b"index/");
            named[6..6 + journal.len()].copy_from_slice(journal.as_bytes());
            core::str::from_utf8(&named[..6 + journal.len()]).unwrap_or("")
        }
        None => return mind::println!("blocks: index: a journal's name of at most 64 bytes"),
    };
    let Some(steps) = steps() else { return };
    let mut work = unsafe {
        use core::ptr::addr_of_mut;
        Work::new(&mut *addr_of_mut!(TERM_AREA), &mut *addr_of_mut!(TEXT_AREA), &mut *addr_of_mut!(POSTING_AREA), &mut *addr_of_mut!(BLOCKS))
    };
    match what {
        "build" => build(journal, name, steps, &mut work, remote),
        "search" => search(journal, word, name, steps, &mut work, remote),
        _ => check(journal, name, steps, &mut work, remote),
    }
}

fn refused(what: &str, journal: &str, error: Error, remote: &Remote) {
    match (error, remote.last) {
        (Error::Journal(r), _) => stopped(journal, r, remote),
        (Error::Store, Some(e)) => mind::println!("blocks: {} {}: Store (the store: {:?})", what, journal, e),
        (e, _) => mind::println!("blocks: {} {}: {:?}", what, journal, e),
    }
}

// build: the counts and the head it was built from, then the name's version and root.
fn build(journal: &str, name: &str, steps: u64, work: &mut Work, remote: &mut Remote) {
    match index::build(remote, journal, name, steps, work) {
        Ok(built) => {
            let (i, m) = (built.index, built.index.manifest);
            mind::println!("INDEXED {} TO SEQUENCE {}: {} ENTRIES WITH TERMS, {} TERMS, {} POSTINGS", journal, m.seq, m.entries, m.terms, m.postings);
            if built.published {
                mind::println!("PUBLISHED {} VERSION {} ROOT {} ({})", name, i.version, i.root, mind::blockstore::class(remote.durability));
            } else {
                mind::println!("UNCHANGED {} VERSION {} ROOT {}", name, i.version, i.root);
            }
        }
        Err(e) => refused("index build", journal, e, remote),
    }
}

// The sequences of one line, each after a space.
struct Sequences<'a>(&'a [u64]);
impl core::fmt::Display for Sequences<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { self.0.iter().try_for_each(|s| write!(f, " {}", s)) }
}

// search: the answers, newest first, LINE to a line, once every one is checked; then the index's state and the count.
fn search(journal: &str, word: &str, name: &str, steps: u64, work: &mut Work, remote: &mut Remote) {
    let (mut line, mut n) = ([0u64; LINE], 0);
    let found = index::search(remote, journal, name, word, steps, work, |seq| {
        line[n] = seq;
        n += 1;
        if n == LINE { mind::println!("SEQUENCES{}", Sequences(&line)); n = 0; }
    });
    if n > 0 { mind::println!("SEQUENCES{}", Sequences(&line[..n])); }
    match found {
        Ok(f) => {
            let seq = f.index.manifest.seq;
            match f.state {
                State::Current => mind::println!("INDEX {} OF {} AT SEQUENCE {}: CURRENT", name, journal, seq),
                State::Stale(lacks) => mind::println!("INDEX {} OF {} AT SEQUENCE {}: STALE, LACKS {} ENTRIES, SCANNED", name, journal, seq, lacks),
            }
            mind::println!("FOUND IN {} ENTRIES OF {}, NEWEST FIRST, EACH CHECKED AGAINST THE JOURNAL", f.entries, journal);
        }
        Err(e) => refused("index search", journal, e, remote),
    }
}

// check: the index rebuilt from its own head and compared, `EQUAL`, `STALE <n>` or `DIFFERS`.
fn check(journal: &str, name: &str, steps: u64, work: &mut Work, remote: &mut Remote) {
    match index::verify(remote, journal, name, steps, work) {
        Ok((i, Verdict::Equal)) => mind::println!("INDEX {} OF {} AT SEQUENCE {}: EQUAL", name, journal, i.manifest.seq),
        Ok((i, Verdict::Stale(n))) => mind::println!("INDEX {} OF {} AT SEQUENCE {}: STALE {}", name, journal, i.manifest.seq, n),
        Ok((i, Verdict::Differs)) => mind::println!("INDEX {} OF {} AT SEQUENCE {}: DIFFERS", name, journal, i.manifest.seq),
        Err(e) => refused("index check", journal, e, remote),
    }
}
