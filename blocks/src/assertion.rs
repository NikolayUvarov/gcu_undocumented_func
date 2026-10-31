// blocks assert (312-STO-0038, docs/storage): assertions about objects in a journal (mind::assertion), each the content of
// an entry of kind `assertion` whose source is `blocks`. `add` says something of an object, `about` lists what is said of
// one newest first, the journal checked whole and every assertion against its CID, and `retract` withdraws a claim of
// `blocks`. The subject is mentioned, never retained.
use mind::assertion::{self, Assertion, Claim, Error, Value, Work, TEXT};
use mind::dag::CHUNK;
use mind::journal::{self, Label};

use crate::journal::{now, steps, stopped};
use crate::{cid, Remote};

const USAGE: &str = "Usage: blocks assert add <journal> <cid> <type> <text> | add <journal> <cid> <type> -c <cid> | about <journal> <cid> | retract <journal> <assertion>";
// The source of every assertion `blocks` writes, and the interpretation of its types.
const SOURCE: &str = "blocks";
const INTERP: u64 = 1;
// Times an append reads the head again after a conflict.
const TRIES: usize = 8;
// The claims and withdrawals of one subject `about` keeps: 256, about 20 KiB, kept here rather than on the stack.
const CLAIMS: usize = 256;
static mut CLAIM_AREA: [Claim; CLAIMS] = [Claim::EMPTY; CLAIMS];
// Bytes of a text `about` prints at most; a longer one, or one with a control character, is told by its length.
const SHOWN: usize = 192;

pub fn command<'a>(mut words: impl Iterator<Item = &'a str>, buffer: &mut [u8; CHUNK], data: &mut [u8; CHUNK], remote: &mut Remote) {
    match (words.next(), words.next(), words.next()) {
        (Some("add"), Some(journal), Some(subject)) => {
            let (Some(ty), Some(first)) = (words.next(), words.next()) else { return mind::println!("{}", USAGE) };
            let Some(subject) = cid(subject) else { return };
            let Some(ty) = Label::new(ty) else { return mind::println!("blocks: assert add: a type of 1 to {} printable bytes, no spaces", journal::LABEL) };
            let mut text = [0u8; TEXT];
            let value = match (first, words.next(), words.next()) {
                ("-c", Some(value), None) => match cid(value) { Some(c) => Value::Cid(c), None => return },
                ("-c", ..) => return mind::println!("{}", USAGE),
                (first, second, third) => match join(first, second.into_iter().chain(third).chain(words), &mut text) {
                    Some(len) => Value::Text(core::str::from_utf8(&text[..len]).unwrap_or("")),
                    None => return mind::println!("blocks: assert add: a text of at most {} bytes", TEXT),
                },
            };
            let a = Assertion { subject, ty, interp: INTERP, value: Some(value), basis: None, retracts: None };
            for _ in 0..TRIES {
                match assertion::add(remote, journal, now(), SOURCE, &a, buffer) {
                    Ok(done) => return mind::println!("ASSERTED {} SEQUENCE {} ASSERTION {} ABOUT {} ({})", journal, done.seq, done.cid, subject, mind::blockstore::class(remote.durability)),
                    Err(Error::Append(journal::Error::Conflict)) => mind::println!("CONFLICT {}: ANOTHER WRITER APPENDED; READING THE HEAD AGAIN", journal),
                    Err(e) => return refused("assert add", journal, e, remote),
                }
            }
            mind::println!("blocks: assert add {}: {} conflicts; nothing appended", journal, TRIES);
        }
        (Some("about"), Some(journal), Some(subject)) if words.next().is_none() => {
            let Some(subject) = cid(subject) else { return };
            let Some(steps) = steps() else { return };
            let mut work = Work::new(unsafe { &mut *core::ptr::addr_of_mut!(CLAIM_AREA) }, buffer, data);
            let answered = assertion::about(remote, journal, &subject, steps, &mut work, |c, a| {
                mind::println!("ASSERTION {} {} SOURCE {} TYPE {} INTERP {}", c.seq, c.cid(), c.source(), a.ty, a.interp);
                if let Some(basis) = a.basis { mind::println!("  BASIS {}", basis); }
                match a.value {
                    Some(Value::Text(t)) if t.len() <= SHOWN && !t.chars().any(char::is_control) => mind::println!("  TEXT {}", t),
                    Some(Value::Text(t)) => mind::println!("  TEXT OF {} BYTES", t.len()),
                    Some(Value::Cid(c)) => mind::println!("  CID {}", c),
                    None => {}
                }
            });
            match answered {
                Ok(a) => mind::println!("{} ASSERTIONS ABOUT {} IN {} AT VERSION {}, NEWEST FIRST", a.claims, subject, journal, a.version),
                Err(e) => refused("assert about", journal, e, remote),
            }
        }
        (Some("retract"), Some(journal), Some(target)) if words.next().is_none() => {
            let Some(target) = cid(target) else { return };
            let Some(steps) = steps() else { return };
            let mut work = Work::new(unsafe { &mut *core::ptr::addr_of_mut!(CLAIM_AREA) }, buffer, data);
            for _ in 0..TRIES {
                match assertion::retract(remote, journal, now(), SOURCE, &target, steps, &mut work) {
                    Ok(done) => return mind::println!("RETRACTED {} IN {} SEQUENCE {} ASSERTION {} ({})", target, journal, done.seq, done.cid, mind::blockstore::class(remote.durability)),
                    Err(Error::Append(journal::Error::Conflict)) => mind::println!("CONFLICT {}: ANOTHER WRITER APPENDED; READING THE HEAD AGAIN", journal),
                    Err(e) => return refused("assert retract", journal, e, remote),
                }
            }
            mind::println!("blocks: assert retract {}: {} conflicts; nothing appended", journal, TRIES);
        }
        _ => mind::println!("{}", USAGE),
    }
}

// The words joined by one space into `out`; none if past it.
fn join<'a>(first: &'a str, rest: impl Iterator<Item = &'a str>, out: &mut [u8]) -> Option<usize> {
    let mut len = 0;
    for word in core::iter::once(first).chain(rest) {
        let sep = (len > 0) as usize;
        if len + sep + word.len() > out.len() { return None; }
        if sep == 1 { out[len] = b' '; }
        out[len + sep..len + sep + word.len()].copy_from_slice(word.as_bytes());
        len += sep + word.len();
    }
    Some(len)
}

fn refused(what: &str, journal: &str, error: Error, remote: &Remote) {
    match (error, remote.last) {
        (Error::Journal(r), _) => stopped(journal, r, remote),
        (Error::Store | Error::Append(journal::Error::Store), Some(e)) => mind::println!("blocks: {} {}: {:?} (the store: {:?})", what, journal, error, e),
        (e, _) => mind::println!("blocks: {} {}: {:?}", what, journal, e),
    }
}
