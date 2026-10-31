//! A text index derived from a journal (issue 312-STO-0035, docs/storage; MC-4.5, 4.6, 4.12, 4.13, 10.3). An index is
//! an object of `tree` under a name of its own: `manifest`, a file holding a canonical DAG-CBOR map of this module's
//! schema (`"v": 1`) that names the journal and the head it was built from, and `terms`, each term a file of its postings
//! (the sequences of the entries whose content holds it) in directories by its first byte, then its first two. Built
//! from what the journal holds in one order, the same head gives the same root. It is never authority: a search checks
//! the journal whole from its head, each answer against its entry, the content tokenized again, and scans the entries
//! above the index's head; a check rebuilds it and compares the roots. No heap and no system calls: the terms and
//! postings lie in a work area the caller gives, and a journal past it has no index: tests/index_host.rs.
use crate::car;
use crate::cid::{self, Cid, Codec};
use crate::dag::{self, fetch, Blocks, In, Out, Same, Sink, CHUNK, LINK};
use crate::journal::{self, Entry, Refused};
use crate::tree::{self, Dir, Kind};

/// The manifest schema's version (`"v"`); another is refused by its number (MC-4.13).
pub const VERSION: u64 = 1;
/// The tokenizer's version; a manifest of another is refused by its number.
pub const TOKENIZER: u64 = 1;
/// The kind of index this module builds.
pub const KIND: &str = "text";
/// Bytes of a term at most: a longer run is cut into pieces of TERM.
pub const TERM: usize = 32;
/// Terms of a searched word at most.
pub const WORD: usize = 8;
/// Bytes of a manifest at most: the map, and each key with its largest value.
pub const MANIFEST_MAX: usize = 1 + (2 + 1) + (4 + 9) + (5 + LINK) + (5 + 1 + KIND.len()) + (6 + 9) + (8 + 9) + (8 + 2 + car::NAME_MAX) + (9 + 9) + (10 + 1);
// The layout's names: the root holds these two.
const MANIFEST: &[u8] = b"manifest";
const TERMS: &[u8] = b"terms";
// Directories under the terms' at most: a first byte's (0-9, a-z), and under one its shards (a one-byte term's own, and
// one for each second byte).
const FIRSTS: usize = 36;
const SHARDS: usize = 1 + 36;
// Bytes kept of each term's file while its shard is written: its CID and size.
const KEPT: usize = cid::BYTES + 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The journal does not check from its head: where its check stopped (`journal::check`).
    Journal(Refused),
    /// The store refused or failed; its own error is the caller's to keep.
    Store,
    /// No index under the name.
    NotFound,
    /// The index's name was removed at this version.
    Removed(u64),
    /// Another writer published under the index's name since it was read: nothing was published.
    Conflict,
    /// A block of the index or of a content the store does not hold.
    Missing,
    /// A block whose bytes do not match its CID.
    Corrupt,
    /// A block of the index out of its layout: a directory, a manifest or a postings file not exactly its encoding.
    Format,
    /// A manifest of another schema version: the one found (MC-4.13).
    Version(u64),
    /// A manifest of another tokenizer: the version found.
    Tokenizer(u64),
    /// A manifest of another kind of index.
    Kind,
    /// An index of another journal: its manifest names another, or the journal's entry at its sequence is another.
    Another,
    /// A journal shorter than the index: its version, below the index's sequence.
    Shorter(u64),
    /// A posting whose entry does not hold the term: a false answer, at this sequence; the index is refused.
    False(u64),
    /// Past the work area: its terms, their bytes or their postings. Nothing was published.
    Full,
    /// A shard of more terms than a directory holds, or a term's postings over a block. Nothing was published.
    TooLarge,
    /// A word that makes no term, or more than WORD.
    Word,
    /// A journal's name the store does not take.
    Name,
}

/// The store as an index needs it: a journal's names and entries, and blocks of either type.
pub trait Store: journal::Store + Blocks {}
impl<S: journal::Store + Blocks> Store for S {}

/// A journal's name, as the store takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Name { len: u8, bytes: [u8; car::NAME_MAX] }
impl Name {
    pub fn new(text: &str) -> Option<Name> {
        let b = text.as_bytes();
        if !car::valid_name(b) { return None; }
        let mut n = Name { len: b.len() as u8, bytes: [0; car::NAME_MAX] };
        n.bytes[..b.len()].copy_from_slice(b);
        Some(n)
    }
    pub fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("") }
}
impl core::fmt::Display for Name { fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { f.write_str(self.as_str()) } }

/// What an index says of itself: the journal it was built from, the head's sequence and CID, and how many entries hold
/// a term, how many terms, and their postings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Manifest { pub journal: Name, pub seq: u64, pub head: Cid, pub entries: u64, pub terms: u64, pub postings: u64 }

impl Manifest {
    // From 1, of an entry (`dag-cbor`), no more entries holding a term than entries.
    fn valid(&self) -> bool { self.seq >= 1 && self.head.codec() == Codec::DagCbor && self.entries <= self.seq }

    /// Encodes the manifest into `out` (MANIFEST_MAX bytes at least); returns its length. One that makes no valid
    /// manifest is Format.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, Error> {
        if !self.valid() { return Err(Error::Format); }
        let mut o = Out { bytes: out, at: 0 };
        encode_with(self, &mut o);
        Ok(o.at)
    }
}

// The keys in DAG-CBOR's order: shorter first, then by their bytes.
fn encode_with(m: &Manifest, o: &mut impl Sink) {
    o.head(5, 9);
    o.text("v");
    o.head(0, VERSION);
    o.text("seq");
    o.head(0, m.seq);
    o.text("head");
    o.link(&m.head);
    o.text("kind");
    o.text(KIND);
    o.text("terms");
    o.head(0, m.terms);
    o.text("entries");
    o.head(0, m.entries);
    o.text("journal");
    o.text(m.journal.as_str());
    o.text("postings");
    o.head(0, m.postings);
    o.text("tokenizer");
    o.head(0, TOKENIZER);
}

/// Decodes a manifest, only if it is exactly the canonical encoding of this schema; another version, kind or tokenizer
/// is refused as such.
pub fn decode(bytes: &[u8]) -> Result<Manifest, Error> {
    let format = |_| Error::Format;
    let mut i = In { bytes, at: 0 };
    let keys = i.expect(5).map_err(format)?;
    // "v" sorts first, so another schema version is told apart before anything else of it is read.
    i.key("v").map_err(format)?;
    let v = i.expect(0).map_err(format)?;
    if v != VERSION { return Err(Error::Version(v)); }
    if keys != 9 { return Err(Error::Format); }
    i.key("seq").map_err(format)?;
    let seq = i.expect(0).map_err(format)?;
    i.key("head").map_err(format)?;
    let head = i.link().map_err(format)?;
    i.key("kind").map_err(format)?;
    let len = i.expect(3).map_err(format)? as usize;
    if i.take(len).map_err(format)? != KIND.as_bytes() { return Err(Error::Kind); }
    i.key("terms").map_err(format)?;
    let terms = i.expect(0).map_err(format)?;
    i.key("entries").map_err(format)?;
    let entries = i.expect(0).map_err(format)?;
    i.key("journal").map_err(format)?;
    let len = i.expect(3).map_err(format)? as usize;
    let journal = core::str::from_utf8(i.take(len).map_err(format)?).ok().and_then(Name::new).ok_or(Error::Format)?;
    i.key("postings").map_err(format)?;
    let postings = i.expect(0).map_err(format)?;
    i.key("tokenizer").map_err(format)?;
    let tokenizer = i.expect(0).map_err(format)?;
    if tokenizer != TOKENIZER { return Err(Error::Tokenizer(tokenizer)); }
    let m = Manifest { journal, seq, head, entries, terms, postings };
    if i.at != bytes.len() || !m.valid() { return Err(Error::Format); }
    // Shortest integer forms: the encoding of what was read must be the bytes themselves.
    let mut same = Same { bytes, at: 0, equal: true };
    encode_with(&m, &mut same);
    if !same.equal || same.at != bytes.len() { return Err(Error::Format); }
    Ok(m)
}

// Terms of a text as it comes: the run of letters and digits so far, lowercased.
struct Tokens { run: [u8; TERM], len: usize }
impl Tokens {
    const fn new() -> Self { Tokens { run: [0; TERM], len: 0 } }
    fn feed(&mut self, text: &[u8], each: &mut impl FnMut(&[u8]) -> Result<(), Error>) -> Result<(), Error> {
        for &c in text {
            if !c.is_ascii_alphanumeric() { self.end(each)?; continue; }
            self.run[self.len] = c.to_ascii_lowercase();
            self.len += 1;
            if self.len == TERM { self.end(each)?; }
        }
        Ok(())
    }
    // The run so far is a term, if there is one.
    fn end(&mut self, each: &mut impl FnMut(&[u8]) -> Result<(), Error>) -> Result<(), Error> {
        let len = core::mem::take(&mut self.len);
        if len > 0 { each(&self.run[..len])?; }
        Ok(())
    }
}

/// Each term of `text` in order, as tokenizer version 1 makes them: a run of ASCII letters and digits, lowercased, 1 to
/// TERM bytes, a longer run cut into pieces of TERM; every other byte separates, so text beyond ASCII makes no term.
pub fn terms(text: &[u8], mut each: impl FnMut(&[u8])) {
    let mut t = Tokens::new();
    let mut each = |term: &[u8]| { each(term); Ok::<(), Error>(()) };
    let _ = t.feed(text, &mut each).and_then(|()| t.end(&mut each));
}

// A read of the index or of a content, as this module tells what it met.
fn fault(e: dag::Error) -> Error {
    match e {
        dag::Error::NotFound => Error::Missing,
        dag::Error::Corrupt => Error::Corrupt,
        dag::Error::TooLarge => Error::TooLarge,
        dag::Error::Store | dag::Error::Full => Error::Store,
        dag::Error::Format | dag::Error::Version | dag::Error::Shape => Error::Format,
    }
}

// The store's answer for a name.
fn stored(e: journal::Error) -> Error {
    match e { journal::Error::Conflict => Error::Conflict, journal::Error::Missing => Error::Missing, journal::Error::Corrupt => Error::Corrupt, _ => Error::Store }
}

// Each term of `content`, a file read whole, every block checked against its CID; a directory holds none.
fn tokens<S: Store>(store: &mut S, content: &Cid, buffer: &mut [u8; CHUNK], chunk: &mut [u8; CHUNK], mut each: impl FnMut(&[u8]) -> Result<(), Error>) -> Result<(), Error> {
    if content.codec() == Codec::DagCbor {
        let len = fetch(store, content, buffer).map_err(fault)?;
        if tree::decode(&buffer[..len]).is_ok() { return Ok(()); }
    }
    let (mut t, mut failed) = (Tokens::new(), None);
    let read = dag::each_chunk(store, content, buffer, chunk, |_, bytes| t.feed(bytes, &mut each).map_err(|e| { failed = Some(e); dag::Error::Store }));
    if let Some(e) = failed { return Err(e); }
    read.map_err(fault)?;
    t.end(&mut each)
}

// Checks the journal whole from its head (`journal::check_each`), `each` hearing the store, each entry and its CID, and
// two blocks for its content's: once `each` fails it hears no more, and a journal that does not check is told first.
fn through<S: Store>(store: &mut S, journal: &str, steps: u64, buffer: &mut [u8; CHUNK], b: &mut [u8; CHUNK], c: &mut [u8; CHUNK],
                     mut each: impl FnMut(&mut S, &Entry, &Cid, &mut [u8; CHUNK], &mut [u8; CHUNK]) -> Result<(), Error>) -> Result<journal::Checked, Error> {
    let mut failed = None;
    let checked = journal::check_each(store, journal, steps, buffer, |s, e, cid| {
        if failed.is_none() { failed = each(s, e, cid, b, c).err(); }
    });
    let checked = checked.map_err(Error::Journal)?;
    failed.map_or(Ok(checked), Err)
}

// An entry met on the way down from the journal's head at `version`, against an index of the entry `head` at `seq`: a
// journal shorter than the index, or whose entry at that sequence is another, is refused.
fn meets(seq: u64, head: &Cid, version: u64, e: &Entry, cid: &Cid) -> Result<(), Error> {
    if version < seq { return Err(Error::Shorter(version)); }
    if e.seq == seq && cid != head { return Err(Error::Another); }
    Ok(())
}

/// A term in a work area: where its bytes lie, the first of its postings and how many.
#[derive(Clone, Copy, Debug)]
pub struct Term { at: u32, len: u8, first: u32, count: u32 }
impl Term { pub const EMPTY: Term = Term { at: 0, len: 0, first: 0, count: 0 }; }

/// A posting in a work area: an entry's sequence, and the next of its term's postings.
#[derive(Clone, Copy, Debug)]
pub struct Posting { seq: u32, next: u32 }
impl Posting { pub const EMPTY: Posting = Posting { seq: 0, next: 0 }; }

// The terms gathered, sorted by their bytes, each with its postings: a list through `postings`, the lowest first.
struct Table<'w> { terms: &'w mut [Term], text: &'w mut [u8], postings: &'w mut [Posting], count: usize, used: usize, posted: usize }

impl Table<'_> {
    fn term(&self, k: usize) -> &[u8] { let t = self.terms[k]; &self.text[t.at as usize..t.at as usize + t.len as usize] }

    // The postings of term `k`, ascending.
    fn sequences(&self, k: usize) -> impl Iterator<Item = u64> + '_ {
        let t = self.terms[k];
        let mut at = t.first;
        (0..t.count).map(move |_| { let p = self.postings[at as usize]; at = p.next; p.seq as u64 })
    }

    // Adds `seq` to the postings of `term`, once: the entries come newest first, so each goes before the term's others.
    fn add(&mut self, term: &[u8], seq: u32) -> Result<(), Error> {
        let text = &*self.text;
        let k = match self.terms[..self.count].binary_search_by(|t| text[t.at as usize..t.at as usize + t.len as usize].cmp(term)) {
            Ok(k) if self.postings[self.terms[k].first as usize].seq == seq => return Ok(()),
            Ok(k) => k,
            Err(k) => {
                if self.count == self.terms.len() || self.used + term.len() > self.text.len() { return Err(Error::Full); }
                self.text[self.used..self.used + term.len()].copy_from_slice(term);
                self.terms.copy_within(k..self.count, k + 1);
                self.terms[k] = Term { at: self.used as u32, len: term.len() as u8, first: 0, count: 0 };
                (self.count, self.used) = (self.count + 1, self.used + term.len());
                k
            }
        };
        if self.posted == self.postings.len() { return Err(Error::Full); }
        self.postings[self.posted] = Posting { seq, next: self.terms[k].first };
        (self.terms[k].first, self.terms[k].count) = (self.posted as u32, self.terms[k].count + 1);
        self.posted += 1;
        Ok(())
    }
}

/// The work area: the terms a build gathers, their bytes and their postings, and the blocks every call reads and writes
/// through. The caller's, so a program keeps them in statics; its bounds are the slices' lengths.
pub struct Work<'w> { table: Table<'w>, blocks: &'w mut [[u8; CHUNK]; 3] }

impl<'w> Work<'w> {
    pub fn new(terms: &'w mut [Term], text: &'w mut [u8], postings: &'w mut [Posting], blocks: &'w mut [[u8; CHUNK]; 3]) -> Self {
        Work { table: Table { terms, text, postings, count: 0, used: 0, posted: 0 }, blocks }
    }
}

// Gathers into the work area the terms of the journal's entries up to `upto`, the index's sequence and head (none: the
// journal's head): the journal checked whole from its head, each content read whole and tokenized. Returns the journal's
// version and the manifest of what was gathered.
fn gather<S: Store>(store: &mut S, journal: &str, upto: Option<(u64, Cid)>, steps: u64, work: &mut Work) -> Result<(u64, Manifest), Error> {
    let name = Name::new(journal).ok_or(Error::Name)?;
    let Work { table, blocks } = work;
    let [a, b, c] = &mut **blocks;
    (table.count, table.used, table.posted) = (0, 0, 0);
    let (mut top, mut head, mut entries) = (None, None, 0);
    let checked = through(store, journal, steps, a, b, c, |s, e, cid, b, c| {
        // The head comes first: without `upto`, the index is of it.
        let version = *top.get_or_insert(e.seq);
        let (seq, at) = upto.unwrap_or((version, *cid));
        meets(seq, &at, version, e, cid)?;
        if e.seq > seq { return Ok(()); }
        if e.seq == seq { head = Some(*cid); }
        let posted = u32::try_from(e.seq).map_err(|_| Error::Full)?;
        let mut any = false;
        tokens(s, &e.content, b, c, |term| { any = true; table.add(term, posted) })?;
        entries += any as u64;
        Ok(())
    })?;
    let seq = upto.map_or(checked.version, |(seq, _)| seq);
    let head = head.ok_or(Error::Store)?;
    Ok((checked.version, Manifest { journal: name, seq, head, entries, terms: table.count as u64, postings: table.posted as u64 }))
}

// The bytes of a CBOR head of `value` in its shortest form.
fn head_len(value: u64) -> usize { match value { 0..=23 => 1, 24..=0xff => 2, 0x100..=0xffff => 3, 0x1_0000..=0xffff_ffff => 5, _ => 9 } }

// Encodes term `k`'s postings file into `out`: a canonical array of their sequences, ascending. Over a block is TooLarge.
fn postings_file(table: &Table, k: usize, out: &mut [u8; CHUNK]) -> Result<usize, Error> {
    let count = table.terms[k].count as u64;
    if head_len(count) + table.sequences(k).map(head_len).sum::<usize>() > CHUNK { return Err(Error::TooLarge); }
    let mut o = Out { bytes: out, at: 0 };
    o.head(4, count);
    for seq in table.sequences(k) { o.head(0, seq); }
    Ok(o.at)
}

// A directory written, as its parent's entry needs it: its name (a first byte, or the first two), entries and CID.
#[derive(Clone, Copy)]
struct Written { name: [u8; 2], len: usize, count: u64, cid: Cid }
impl Written {
    fn entry(&self) -> tree::Entry<'_> { tree::Entry { name: &self.name[..self.len], kind: Kind::Directory, size: self.count, cid: self.cid } }
}

// Puts a directory of `count` entries, given one by one; its CID.
fn directory<'a>(count: usize, entry: impl Fn(usize) -> tree::Entry<'a>, out: &mut [u8; CHUNK], put: &mut impl FnMut(Codec, &[u8]) -> Result<Cid, Error>) -> Result<Cid, Error> {
    let len = tree::encode_each(count, entry, out).map_err(|e| if e == dag::Error::TooLarge { Error::TooLarge } else { Error::Format })?;
    put(Codec::DagCbor, &out[..len])
}

// Writes the index of what `table` gathered: each term's postings file, then its shard, then the shards' first byte's
// directory, all in the terms' order; the terms' directory, the manifest and the root. `put` stores each block, or for a
// check only names it; `out` holds what is written, `kept` the files of the shard being written. Returns the root.
fn write_out(table: &Table, manifest: &Manifest, out: &mut [u8; CHUNK], kept: &mut [u8; CHUNK], put: &mut impl FnMut(Codec, &[u8]) -> Result<Cid, Error>) -> Result<Cid, Error> {
    let none = Written { name: [0; 2], len: 0, count: 0, cid: manifest.head };
    let (mut shards, mut firsts) = ([none; SHARDS], [none; FIRSTS]);
    let (mut ns, mut nf, mut k) = (0, 0, 0);
    let shard = |k: usize| { let t = table.term(k); &t[..t.len().min(2)] };
    while k < table.count {
        let key = shard(k);
        let end = (k..table.count).find(|&j| shard(j) != key).unwrap_or(table.count);
        if end - k > CHUNK / KEPT { return Err(Error::TooLarge); }
        for j in k..end {
            let len = postings_file(table, j, out)?;
            let cid = put(Codec::Raw, &out[..len])?;
            let at = (j - k) * KEPT;
            kept[at..at + cid::BYTES].copy_from_slice(&cid.to_bytes());
            kept[at + cid::BYTES..at + KEPT].copy_from_slice(&(len as u32).to_le_bytes());
        }
        let file = |i: usize| {
            let at = i * KEPT;
            let size = u32::from_le_bytes(kept[at + cid::BYTES..at + KEPT].try_into().unwrap()) as u64;
            tree::Entry { name: table.term(k + i), kind: Kind::File, size, cid: Cid::from_bytes(&kept[at..at + cid::BYTES]).unwrap() }
        };
        let cid = directory(end - k, file, out, put)?;
        shards[ns] = Written { name: [key[0], key.get(1).copied().unwrap_or(0)], len: key.len(), count: (end - k) as u64, cid };
        ns += 1;
        // A first byte's directory, once its last shard is written.
        if end == table.count || table.term(end)[0] != key[0] {
            let cid = directory(ns, |i| shards[i].entry(), out, put)?;
            firsts[nf] = Written { name: [key[0], 0], len: 1, count: ns as u64, cid };
            (nf, ns) = (nf + 1, 0);
        }
        k = end;
    }
    let terms = directory(nf, |i| firsts[i].entry(), out, put)?;
    let len = manifest.encode(&mut out[..])?;
    let file = put(Codec::Raw, &out[..len])?;
    let root = [tree::Entry { name: MANIFEST, kind: Kind::File, size: len as u64, cid: file }, tree::Entry { name: TERMS, kind: Kind::Directory, size: nf as u64, cid: terms }];
    directory(root.len(), |i| root[i], out, put)
}

/// An index as its name gives it: the name's version and root, and the manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Indexed { pub version: u64, pub root: Cid, pub manifest: Manifest }

// The directory `cid` names, of `count` entries if its parent gives them: read, checked against its CID and the schema.
fn read_dir<'b, S: Store>(store: &mut S, cid: &Cid, count: Option<u64>, buffer: &'b mut [u8; CHUNK]) -> Result<Dir<'b>, Error> {
    if cid.codec() != Codec::DagCbor { return Err(Error::Format); }
    let len = fetch(store, cid, buffer).map_err(fault)?;
    let buffer: &'b [u8; CHUNK] = buffer;
    match tree::decode(&buffer[..len]) { Ok(dir) if count.is_none_or(|n| n == dir.len() as u64) => Ok(dir), _ => Err(Error::Format) }
}

// A file of the index: one `raw` block of `size` bytes, read and checked against its CID.
fn read_file<'b, S: Store>(store: &mut S, cid: &Cid, size: u64, buffer: &'b mut [u8; CHUNK]) -> Result<&'b [u8], Error> {
    if cid.codec() != Codec::Raw || size > CHUNK as u64 { return Err(Error::Format); }
    let len = fetch(store, cid, buffer).map_err(fault)?;
    if len as u64 != size { return Err(Error::Format); }
    Ok(&buffer[..len])
}

// The index under `name`, of `journal`: its root and manifest read and checked, and the terms' directory (CID, entries).
fn open<S: Store>(store: &mut S, name: &str, journal: &str, buffer: &mut [u8; CHUNK]) -> Result<(Indexed, (Cid, u64)), Error> {
    let (version, root) = journal::Store::head(store, name).map_err(stored)?;
    let root = root.ok_or(if version == 0 { Error::NotFound } else { Error::Removed(version) })?;
    let dir = read_dir(store, &root, None, buffer)?;
    let (m, t) = match (dir.len(), dir.find(MANIFEST), dir.find(TERMS)) {
        (2, Some(m), Some(t)) if m.kind == Kind::File && t.kind == Kind::Directory => ((m.cid, m.size), (t.cid, t.size)),
        _ => return Err(Error::Format),
    };
    let manifest = decode(read_file(store, &m.0, m.1, buffer)?)?;
    if manifest.journal.as_str() != journal { return Err(Error::Another); }
    Ok((Indexed { version, root, manifest }, t))
}

// The postings file of `term` under the terms' directory (CID, entries): none if the index holds no such term.
fn lookup<S: Store>(store: &mut S, terms: (Cid, u64), term: &[u8], buffer: &mut [u8; CHUNK]) -> Result<Option<(Cid, u64)>, Error> {
    let mut at = terms;
    // Down by its first byte, then its first two, to its file.
    for (key, kind) in [(&term[..1], Kind::Directory), (&term[..term.len().min(2)], Kind::Directory), (term, Kind::File)] {
        let dir = read_dir(store, &at.0, Some(at.1), buffer)?;
        match dir.find(key) {
            None => return Ok(None),
            Some(e) if e.kind == kind => at = (e.cid, e.size),
            Some(_) => return Err(Error::Format),
        }
    }
    Ok(Some(at))
}

// A CBOR head of `major` read only in its shortest form.
fn shortest(i: &mut In, major: u8) -> Result<u64, Error> {
    let start = i.at;
    let value = i.expect(major).map_err(|_| Error::Format)?;
    if i.at - start != head_len(value) { return Err(Error::Format); }
    Ok(value)
}

// The sequences of a postings file, each from 1 to `upto`, ascending and once, as `each` hears them in order: only
// exactly the encoding a build writes is read.
fn sequences(bytes: &[u8], upto: u64, mut each: impl FnMut(u64) -> Result<(), Error>) -> Result<(), Error> {
    let mut i = In { bytes, at: 0 };
    let count = shortest(&mut i, 4)?;
    let mut last = 0;
    for _ in 0..count {
        let seq = shortest(&mut i, 0)?;
        if seq <= last || seq > upto { return Err(Error::Format); }
        each(seq)?;
        last = seq;
    }
    if count == 0 || i.at != bytes.len() { return Err(Error::Format); }
    Ok(())
}

// A searched word's terms, each once.
struct Query { terms: [[u8; TERM]; WORD], lens: [u8; WORD], count: usize }
impl Query {
    fn of(word: &str) -> Result<Query, Error> {
        let (mut q, mut over) = (Query { terms: [[0; TERM]; WORD], lens: [0; WORD], count: 0 }, false);
        terms(word.as_bytes(), |term| {
            if q.iter().any(|t| t == term) { return; }
            if q.count == WORD { over = true; return; }
            q.terms[q.count][..term.len()].copy_from_slice(term);
            q.lens[q.count] = term.len() as u8;
            q.count += 1;
        });
        if over || q.count == 0 { return Err(Error::Word); }
        Ok(q)
    }
    fn iter(&self) -> impl Iterator<Item = &[u8]> { (0..self.count).map(|k| &self.terms[k][..self.lens[k] as usize]) }
}

// Whether `content` holds every term of `query`.
fn holds<S: Store>(store: &mut S, content: &Cid, query: &Query, buffer: &mut [u8; CHUNK], chunk: &mut [u8; CHUNK]) -> Result<bool, Error> {
    let mut met = 0u32;
    tokens(store, content, buffer, chunk, |term| {
        for (k, t) in query.iter().enumerate() { if t == term { met |= 1 << k; } }
        Ok(())
    })?;
    Ok(met == (1 << query.count) - 1)
}

// A sequence kept in the work area's postings: past it, or past u32, is Full.
fn keep(postings: &mut [Posting], at: usize, seq: u64) -> Result<(), Error> {
    postings.get_mut(at).ok_or(Error::Full)?.seq = u32::try_from(seq).map_err(|_| Error::Full)?;
    Ok(())
}

/// What a build made: the index under its name, at the version that holds its root; `published` if this build
/// published it, not if the name held that root already.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Built { pub index: Indexed, pub published: bool }

/// Builds the index of `journal` from its head and publishes it under `name` by compare-and-swap from the version read
/// first: the journal checked whole from its head, within `steps` entries, and each entry's content read whole, every
/// block checked against its CID, and tokenized. The same head gives the same root, and a name that holds it already is
/// not published again. Past the work area is Full, a shard or a term's postings over a block TooLarge, and another
/// writer's publication since Conflict: nothing is published.
pub fn build<S: Store>(store: &mut S, journal: &str, name: &str, steps: u64, work: &mut Work) -> Result<Built, Error> {
    let (version, current) = journal::Store::head(store, name).map_err(stored)?;
    let (_, manifest) = gather(store, journal, None, steps, work)?;
    let Work { table, blocks } = work;
    let [_, b, c] = &mut **blocks;
    let root = write_out(table, &manifest, b, c, &mut |codec, data| {
        let cid = Blocks::put(store, codec, data).map_err(fault)?;
        // The store's answer is checked, as `dag::Builder` checks it.
        if cid != Cid::of(codec, data) { return Err(Error::Corrupt); }
        Ok(cid)
    })?;
    if current == Some(root) { return Ok(Built { index: Indexed { version, root, manifest }, published: false }); }
    let version = journal::Store::publish(store, name, version, &root).map_err(stored)?;
    Ok(Built { index: Indexed { version, root, manifest }, published: true })
}

/// Whether an index is of its journal's head, or of an entry below it: the journal grown by this many entries since.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State { Current, Stale(u64) }

/// The index under `name` and whether it is current: the journal checked whole from its head, its entry at the index's
/// sequence the index's head. Refused: a journal that does not check (Journal), whose entry there is another (Another),
/// or that is shorter (Shorter).
pub fn state<S: Store>(store: &mut S, journal: &str, name: &str, steps: u64, work: &mut Work) -> Result<(Indexed, State), Error> {
    let [a, b, c] = &mut *work.blocks;
    let (index, _) = open(store, name, journal, b)?;
    let m = index.manifest;
    let mut top = None;
    let checked = through(store, journal, steps, a, b, c, |_, e, cid, _, _| meets(m.seq, &m.head, *top.get_or_insert(e.seq), e, cid))?;
    Ok((index, if checked.version == m.seq { State::Current } else { State::Stale(checked.version - m.seq) }))
}

/// What a search answered: the index read, whether it is current or how many entries it lacks, and how many found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found { pub index: Indexed, pub state: State, pub entries: u64 }

/// The entries of `journal` whose content holds every term of `word`, newest first, from the index under `name`: the
/// postings of the word's terms, then the journal read from its head as `state` reads it, each entry above the index's
/// head scanned, and each answer below it checked, its content tokenized again. A posting whose entry does not hold the
/// terms is a false answer (False), and the index is refused; so an answer is the exact scan's (`scan`). `each` hears
/// the answers once all are checked.
pub fn search<S: Store>(store: &mut S, journal: &str, name: &str, word: &str, steps: u64, work: &mut Work, mut each: impl FnMut(u64)) -> Result<Found, Error> {
    let query = Query::of(word)?;
    let Work { table, blocks } = work;
    let [a, b, c] = &mut **blocks;
    let found = &mut *table.postings;
    let (index, terms) = open(store, name, journal, b)?;
    let m = index.manifest;
    // The entries up to the index's head that every term names: their sequences, ascending.
    let mut n = 0;
    for (k, term) in query.iter().enumerate() {
        let Some((cid, size)) = lookup(store, terms, term, b)? else { n = 0; break };
        let (mut kept, mut at) = (0, 0);
        sequences(read_file(store, &cid, size, b)?, m.seq, |seq| {
            if k == 0 {
                keep(found, kept, seq)?;
                kept += 1;
            } else {
                while at < n && (found[at].seq as u64) < seq { at += 1; }
                if at < n && found[at].seq as u64 == seq {
                    found[kept].seq = seq as u32;
                    (kept, at) = (kept + 1, at + 1);
                }
            }
            Ok(())
        })?;
        n = kept;
    }
    // From the head down: the entries above the index's head scanned, their answers kept after the postings; each
    // posting's entry checked.
    let (mut top, mut next, mut above) = (None, n, n);
    let checked = through(store, journal, steps, a, b, c, |s, e, cid, b, c| {
        meets(m.seq, &m.head, *top.get_or_insert(e.seq), e, cid)?;
        if e.seq > m.seq {
            if holds(s, &e.content, &query, b, c)? { keep(found, above, e.seq)?; above += 1; }
        } else if next > 0 && found[next - 1].seq as u64 == e.seq {
            if !holds(s, &e.content, &query, b, c)? { return Err(Error::False(e.seq)); }
            next -= 1;
        }
        Ok(())
    })?;
    for k in (n..above).chain((0..n).rev()) { each(found[k].seq as u64); }
    let state = if checked.version == m.seq { State::Current } else { State::Stale(checked.version - m.seq) };
    Ok(Found { index, state, entries: above as u64 })
}

/// The exact scan, the answer without an index: the entries of `journal` whose content holds every term of `word`,
/// newest first, the journal checked whole from its head and every content tokenized. Returns how many; `each` hears
/// them once the journal is checked.
pub fn scan<S: Store>(store: &mut S, journal: &str, word: &str, steps: u64, work: &mut Work, mut each: impl FnMut(u64)) -> Result<u64, Error> {
    let query = Query::of(word)?;
    let Work { table, blocks } = work;
    let [a, b, c] = &mut **blocks;
    let found = &mut *table.postings;
    let mut n = 0;
    through(store, journal, steps, a, b, c, |s, e, _, b, c| {
        if holds(s, &e.content, &query, b, c)? { keep(found, n, e.seq)?; n += 1; }
        Ok(())
    })?;
    for p in &found[..n] { each(p.seq as u64); }
    Ok(n as u64)
}

/// What a check of an index found: equal to its rebuild; stale, equal to its rebuild from its own head, the journal grown
/// by this many entries since; or it differs from its rebuild, a posting dropped among what this tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict { Equal, Stale(u64), Differs }

/// Checks the index under `name`: rebuilt in the work area from the index's own head, nothing put, and its root compared
/// with the one published. Refused as `state` refuses, and as a build is past the work area.
pub fn verify<S: Store>(store: &mut S, journal: &str, name: &str, steps: u64, work: &mut Work) -> Result<(Indexed, Verdict), Error> {
    let (index, _) = open(store, name, journal, &mut work.blocks[1])?;
    let m = index.manifest;
    let (version, manifest) = gather(store, journal, Some((m.seq, m.head)), steps, work)?;
    let Work { table, blocks } = work;
    let [_, b, c] = &mut **blocks;
    let root = write_out(table, &manifest, b, c, &mut |codec, data| Ok(Cid::of(codec, data)))?;
    let verdict = if root != index.root { Verdict::Differs } else if version == m.seq { Verdict::Equal } else { Verdict::Stale(version - m.seq) };
    Ok((index, verdict))
}
