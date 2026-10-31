//! Assertions about objects, kept in a journal apart from content (issue 312-STO-0038, docs/storage; MC-4.5, 4.6, 4.12,
//! 4.13). An assertion is one canonical DAG-CBOR map `{"v": 1, "type", "basis", "value", "interp", "subject",
//! "retracts"}`, its keys in DAG-CBOR's order: what is said of `subject`, a type with the version of its meaning, a text or
//! a CID as its value, and what it was derived from, if anything. A withdrawal has no value and names in `retracts` an
//! earlier claim of its source. `subject`, `basis`, `retracts` and a CID value are the CIDs' bytes, not links, so no walk
//! follows them: an assertion mentions its subject and never retains it. It is the content of a journal entry of kind
//! `assertion`, a `raw` block, so the journal gives it its order, its time and its source, and retains it. No heap and no
//! system calls: tests/assertion_host.rs.
use crate::cid::{self, Cid, Codec};
use crate::dag::{self, fetch, Blocks, In, Out, Same, Sink, CHUNK};
use crate::journal::{self, Checked, Entry, Label, Reading, Refused, LABEL};

/// The schema's version (`"v"`); another is refused by its number (MC-4.13).
pub const VERSION: u64 = 1;
/// The kind of a journal entry whose content is an assertion.
pub const KIND: &str = "assertion";
/// Bytes of a text value at most.
pub const TEXT: usize = 1024;
// A CID as a byte string: its head, then its binary form.
const BYTES: usize = 2 + cid::BYTES;
/// Bytes of an assertion at most: the map, and each key with its largest value (a text's, not a withdrawal's).
pub const MAX: usize = 1 + (2 + 1) + (5 + 2 + LABEL) + (6 + BYTES) + (6 + 3 + TEXT) + (7 + 9) + (8 + BYTES);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The journal does not check from its head: where its check stopped (`journal::check`).
    Journal(Refused),
    /// The journal refused the append, with its own error; `Conflict` if another writer appended first: append again.
    Append(journal::Error),
    /// The store refused or failed.
    Store,
    /// An assertion's block the store does not hold.
    Missing,
    /// A block whose bytes do not match its CID.
    Corrupt,
    /// A block that is not exactly an assertion of this schema: also the content of an entry of kind `assertion`.
    Format,
    /// An assertion of another schema version: the one found (MC-4.13).
    Version(u64),
    /// No assertion: a claim without its value or with a text past TEXT, a withdrawal with a value or of no `raw` block;
    /// a withdrawal given to `add`; a source that is no label.
    Invalid,
    /// Retracting: the journal holds no claim with this CID.
    NotFound,
    /// Retracting: the claim with this CID is another source's.
    Another,
    /// Retracting: the source's claim is withdrawn already.
    Withdrawn,
    /// More claims and withdrawals of the subject than the work area holds.
    Full,
}

/// What is said: a text, or a CID (a mention, as the subject is).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value<'a> { Text(&'a str), Cid(Cid) }

/// An assertion: its subject, its type and the version of the type's meaning, and either its value (a claim) or the
/// claim it withdraws; what it was derived from, if anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assertion<'a> { pub subject: Cid, pub ty: Label, pub interp: u64, pub value: Option<Value<'a>>, pub basis: Option<Cid>, pub retracts: Option<Cid> }

impl Assertion<'_> {
    // A value of at most TEXT bytes or a withdrawal of an assertion's block (`raw`), one of the two.
    fn valid(&self) -> bool {
        self.value.is_some() != self.retracts.is_some() && !matches!(self.value, Some(Value::Text(t)) if t.len() > TEXT)
            && self.retracts.is_none_or(|r| r.codec() == Codec::Raw)
    }

    /// Encodes the assertion into `out` (MAX bytes at least); returns its length. One that makes no valid assertion is
    /// Invalid.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, Error> {
        if !self.valid() { return Err(Error::Invalid); }
        let mut o = Out { bytes: out, at: 0 };
        encode_with(self, &mut o);
        Ok(o.at)
    }
}

// A CID as a byte string of its binary form: a mention, never a link (tag 42), so no walk follows it.
fn mention(o: &mut impl Sink, cid: &Cid) { o.head(2, cid::BYTES as u64); o.push(&cid.to_bytes()); }

// The keys in DAG-CBOR's order: shorter first, then by their bytes.
fn encode_with(a: &Assertion, o: &mut impl Sink) {
    o.head(5, 5 + a.basis.is_some() as u64);
    o.text("v");
    o.head(0, VERSION);
    o.text("type");
    o.text(a.ty.as_str());
    if let Some(basis) = &a.basis {
        o.text("basis");
        mention(o, basis);
    }
    match &a.value {
        Some(Value::Text(t)) => { o.text("value"); o.text(t) }
        Some(Value::Cid(c)) => { o.text("value"); mention(o, c) }
        None => {}
    }
    o.text("interp");
    o.head(0, a.interp);
    o.text("subject");
    mention(o, &a.subject);
    if let Some(retracts) = &a.retracts {
        o.text("retracts");
        mention(o, retracts);
    }
}

// Whether the next key is `name`: read past it if so, left unread if not.
fn next(i: &mut In, name: &str) -> bool {
    let at = i.at;
    if i.key(name).is_ok() { return true; }
    i.at = at;
    false
}

fn mentioned(i: &mut In) -> Result<Cid, Error> {
    if i.expect(2).map_err(|_| Error::Format)? != cid::BYTES as u64 { return Err(Error::Format); }
    Cid::from_bytes(i.take(cid::BYTES).map_err(|_| Error::Format)?).map_err(|_| Error::Format)
}

fn value<'a>(i: &mut In<'a>) -> Result<Value<'a>, Error> {
    match i.bytes.get(i.at).map(|b| b >> 5) {
        // A text past TEXT is read, then refused as no valid assertion.
        Some(3) => {
            let len = i.expect(3).map_err(|_| Error::Format)? as usize;
            core::str::from_utf8(i.take(len).map_err(|_| Error::Format)?).map(Value::Text).map_err(|_| Error::Format)
        }
        Some(2) => mentioned(i).map(Value::Cid),
        _ => Err(Error::Format),
    }
}

/// Decodes an assertion, only if it is exactly the canonical encoding of this schema; another version is refused by its
/// number.
pub fn decode(bytes: &[u8]) -> Result<Assertion<'_>, Error> {
    let format = |_| Error::Format;
    let mut i = In { bytes, at: 0 };
    let keys = i.expect(5).map_err(format)?;
    // "v" sorts first, so another schema version is told apart before anything else of it is read.
    i.key("v").map_err(format)?;
    let v = i.expect(0).map_err(format)?;
    if v != VERSION { return Err(Error::Version(v)); }
    if keys != 5 && keys != 6 { return Err(Error::Format); }
    i.key("type").map_err(format)?;
    let len = i.expect(3).map_err(format)? as usize;
    let ty = core::str::from_utf8(i.take(len).map_err(format)?).ok().and_then(Label::new).ok_or(Error::Format)?;
    let basis = if next(&mut i, "basis") { Some(mentioned(&mut i)?) } else { None };
    let value = if next(&mut i, "value") { Some(value(&mut i)?) } else { None };
    i.key("interp").map_err(format)?;
    let interp = i.expect(0).map_err(format)?;
    i.key("subject").map_err(format)?;
    let subject = mentioned(&mut i)?;
    let retracts = if next(&mut i, "retracts") { Some(mentioned(&mut i)?) } else { None };
    let a = Assertion { subject, ty, interp, value, basis, retracts };
    let read = 4 + basis.is_some() as u64 + value.is_some() as u64 + retracts.is_some() as u64;
    if read != keys || i.at != bytes.len() || !a.valid() { return Err(Error::Format); }
    // Shortest forms: the encoding of what was read must be the bytes themselves.
    let mut same = Same { bytes, at: 0, equal: true };
    encode_with(&a, &mut same);
    if !same.equal || same.at != bytes.len() { return Err(Error::Format); }
    Ok(a)
}

/// The store as assertions need it: a journal's names and entries, and blocks of either type.
pub trait Store: journal::Store + Blocks {}
impl<S: journal::Store + Blocks> Store for S {}

// A read of an assertion's block, as this module tells what it met.
fn fault(e: dag::Error) -> Error {
    match e { dag::Error::NotFound => Error::Missing, dag::Error::Corrupt => Error::Corrupt, _ => Error::Store }
}

/// The assertion `cid` names: a `raw` block, read and checked against its CID, then decoded.
pub fn read<'b, S: Store>(store: &mut S, cid: &Cid, buffer: &'b mut [u8; CHUNK]) -> Result<Assertion<'b>, Error> {
    if cid.codec() != Codec::Raw { return Err(Error::Format); }
    let len = fetch(store, cid, buffer).map_err(fault)?;
    let buffer: &'b [u8; CHUNK] = buffer;
    decode(&buffer[..len])
}

/// An assertion of a journal, as `about` answers it: its entry's sequence and source, and its CID. A work area holds
/// these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim { pub seq: u64, len: u8, source: [u8; LABEL], key: [u8; cid::BYTES] }
impl Claim {
    pub const EMPTY: Claim = Claim { seq: 0, len: 0, source: [0; LABEL], key: [0; cid::BYTES] };
    fn new(seq: u64, source: &Label, cid: &Cid) -> Claim {
        let mut c = Claim { seq, len: source.as_str().len() as u8, source: [0; LABEL], key: cid.to_bytes() };
        c.source[..c.len as usize].copy_from_slice(source.as_str().as_bytes());
        c
    }
    pub fn source(&self) -> &str { core::str::from_utf8(&self.source[..self.len as usize]).unwrap_or("") }
    pub fn cid(&self) -> Cid { Cid::from_bytes(&self.key).unwrap() }
    fn is(&self, source: &Label, cid: &Cid) -> bool { self.source() == source.as_str() && self.key == cid.to_bytes() }
}

/// The work area: the claims and withdrawals of one subject `about` keeps while it reads the journal, and two blocks it
/// reads through (an entry's, an assertion's). The caller's, so a program keeps them in statics; its bound is the claims'.
pub struct Work<'w> { claims: &'w mut [Claim], entry: &'w mut [u8; CHUNK], block: &'w mut [u8; CHUNK] }
impl<'w> Work<'w> {
    pub fn new(claims: &'w mut [Claim], entry: &'w mut [u8; CHUNK], block: &'w mut [u8; CHUNK]) -> Self { Work { claims, entry, block } }
}

// Checks the journal whole from its head (`journal::check_each`), `each` hearing each entry of kind `assertion` with its
// assertion read and checked against its CID; entries of another kind are skipped. Once a read or `each` fails no more
// are heard, and a journal that does not check is told first.
fn through<S: Store>(store: &mut S, journal: &str, steps: u64, entry: &mut [u8; CHUNK], block: &mut [u8; CHUNK],
                     mut each: impl FnMut(&Entry, &Assertion) -> Result<(), Error>) -> Result<Checked, Error> {
    let mut failed = None;
    let checked = journal::check_each(store, journal, steps, entry, |s, e, _| {
        if failed.is_some() || e.kind.as_str() != KIND { return; }
        failed = read(s, &e.content, block).and_then(|a| each(e, &a)).err();
    });
    let checked = checked.map_err(Error::Journal)?;
    failed.map_or(Ok(checked), Err)
}

/// What an `add` or a `retract` appended: the entry's sequence and CID, and the assertion's CID, its content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Added { pub seq: u64, pub cid: Cid, pub entry: Cid }

// Puts the assertion as a `raw` block and appends it to the journal as an entry of kind `assertion`.
fn append<S: Store>(store: &mut S, journal: &str, now: Reading, source: &str, a: &Assertion, buffer: &mut [u8; CHUNK]) -> Result<Added, Error> {
    if Label::new(source).is_none() { return Err(Error::Invalid); }
    let len = a.encode(&mut buffer[..])?;
    let cid = Blocks::put(store, Codec::Raw, &buffer[..len]).map_err(fault)?;
    // The store's answer is checked, as `dag::Builder` checks it.
    if cid != Cid::of(Codec::Raw, &buffer[..len]) { return Err(Error::Corrupt); }
    let done = journal::append(store, journal, now, source, KIND, &cid, buffer).map_err(Error::Append)?;
    Ok(Added { seq: done.entry.seq, cid, entry: done.cid })
}

/// Appends the claim `a` to `journal` from `source`: the assertion put as a `raw` block, the content of an entry of kind
/// `assertion` (`journal::append`, by compare-and-swap; `Append(Conflict)` if another writer appended first, nothing
/// appended). A withdrawal is `retract`'s, and a source that is no label is refused (Invalid), nothing put. The subject
/// need not be held: it is mentioned, not retained.
pub fn add<S: Store>(store: &mut S, journal: &str, now: Reading, source: &str, a: &Assertion, buffer: &mut [u8; CHUNK]) -> Result<Added, Error> {
    if a.retracts.is_some() { return Err(Error::Invalid); }
    append(store, journal, now, source, a, buffer)
}

/// What `about` read: the journal's version, and how many claims it answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Answered { pub version: u64, pub claims: u64 }

/// The claims of `journal` about `subject`, newest first, without those a later withdrawal of their own source names:
/// the journal checked whole from its head, within `steps` entries, and every assertion in it read and checked against
/// its CID. A withdrawal by another source withdraws nothing. `each` hears each claim, read and checked again, once the
/// whole journal is checked: a journal that does not check answers nothing. Past the work area is Full.
pub fn about<S: Store>(store: &mut S, journal: &str, subject: &Cid, steps: u64, work: &mut Work, mut each: impl FnMut(&Claim, &Assertion)) -> Result<Answered, Error> {
    let Work { claims, entry, block } = work;
    // The claims from the first slot up, the withdrawals from the last down.
    let (mut n, mut w) = (0, claims.len());
    let checked = through(store, journal, steps, entry, block, |e, a| {
        if a.subject != *subject { return Ok(()); }
        // Met from the head down, a withdrawal comes before the claims it withdraws.
        if a.retracts.is_none() && claims[w..].iter().any(|c| c.is(&e.source, &e.content)) { return Ok(()); }
        if n == w { return Err(Error::Full); }
        match a.retracts {
            Some(r) => { w -= 1; claims[w] = Claim::new(e.seq, &e.source, &r); }
            None => { claims[n] = Claim::new(e.seq, &e.source, &e.content); n += 1; }
        }
        Ok(())
    })?;
    for c in &claims[..n] { each(c, &read(store, &c.cid(), block)?); }
    Ok(Answered { version: checked.version, claims: n as u64 })
}

/// Withdraws the claim `target` of `source` in `journal`: the journal checked whole from its head, within `steps`
/// entries, every assertion read and checked, then a withdrawal of it with its subject, type and interpretation appended
/// as `add` appends. Only the source's own claim that stands is withdrawn: one the journal does not hold (NotFound),
/// another source's (Another) or withdrawn already (Withdrawn) is refused, and nothing is appended.
pub fn retract<S: Store>(store: &mut S, journal: &str, now: Reading, source: &str, target: &Cid, steps: u64, work: &mut Work) -> Result<Added, Error> {
    let label = Label::new(source).ok_or(Error::Invalid)?;
    let Work { entry, block, .. } = work;
    // The source's claim of `target` (its bytes, so the same in each), and whether the newest of that claim and the
    // source's withdrawals of it is the claim; another source's claim of it is told apart.
    let (mut said, mut stands, mut other) = (None, None, false);
    through(store, journal, steps, entry, block, |e, a| {
        let claim = a.retracts.is_none() && e.content == *target;
        if e.source != label { other |= claim; return Ok(()); }
        if claim { said.get_or_insert((a.subject, a.ty, a.interp)); stands.get_or_insert(true); }
        if a.retracts == Some(*target) { stands.get_or_insert(false); }
        Ok(())
    })?;
    let (subject, ty, interp) = match (said, stands) {
        (Some(_), Some(false)) => return Err(Error::Withdrawn),
        (Some(said), _) => said,
        (None, _) if other => return Err(Error::Another),
        (None, _) => return Err(Error::NotFound),
    };
    let withdrawal = Assertion { subject, ty, interp, value: None, basis: None, retracts: Some(*target) };
    append(store, journal, now, source, &withdrawal, entry)
}
