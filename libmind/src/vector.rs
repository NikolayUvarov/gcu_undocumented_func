//! A vector index derived from a journal of embeddings (issue 312-STO-0047, docs/storage; MC-4.5, 4.6, 4.12, 4.13, 10.3).
//! An embedding is one canonical DAG-CBOR map `{"v": 1, "model", "vector", "subject"}`: the vector a model made of an
//! object, its components signed bytes, with the CIDs of the model and of the object as bytes, not links, so it mentions
//! both and retains neither. It is the content of a journal entry of kind `embedding`, a `raw` block, so the journal gives
//! it its order, time and source. The store runs no model: a producer appends embeddings, and an index is of one model's.
//! An index is an object of `tree` under a name of its own: `manifest`, a file holding a canonical DAG-CBOR map of this
//! module's schema that names the journal, the head it was built from, the model, its dimensions and the metric, and
//! `vectors`, files of the vectors with their entries' sequences, newest first. The same head gives the same root. It is
//! an exact scan: a search scores every vector of the index and of the entries above its head, by the dot product, and
//! checks each answer against its entry; a check rebuilds it and compares the roots. It is never authority. No heap and
//! no system calls: tests/vector_host.rs.
use crate::car;
use crate::cid::{self, Cid, Codec};
use crate::dag::{self, fetch, Blocks, In, Out, Same, Sink, CHUNK, LINK};
use crate::index::{Name, State, Store, Verdict};
use crate::journal::{self, Entry, Label, Reading, Refused};
use crate::tree::{self, Dir, Kind};

/// The embedding schema's version (`"v"`); another is refused by its number (MC-4.13).
pub const EMBEDDING: u64 = 1;
/// The manifest schema's version (`"v"`); another is refused by its number (MC-4.13).
pub const VERSION: u64 = 1;
/// The kind of a journal entry whose content is an embedding.
pub const ENTRY: &str = "embedding";
/// The kind of index this module builds.
pub const KIND: &str = "vector";
/// The metric: the dot product of two vectors, each component a signed byte; a manifest of another is refused.
pub const METRIC: &str = "dot";
/// Components of a vector at most.
pub const DIMS: usize = 1024;
/// Answers of a search at most.
pub const NEAREST: usize = 16;
/// Files of vectors in an index at most.
pub const FILES: usize = 256;
// A CID as a byte string: its head, then its binary form.
const BYTES: usize = 2 + cid::BYTES;
/// Bytes of an embedding at most: the map, and each key with its largest value.
pub const RECORD_MAX: usize = 1 + (2 + 1) + (6 + BYTES) + (7 + 3 + DIMS) + (8 + BYTES);
/// Bytes of a manifest at most: the map, and each key with its largest value.
pub const MANIFEST_MAX: usize = 1 + (2 + 1) + (4 + 9) + (5 + 3) + (5 + LINK) + (5 + 1 + KIND.len()) + (6 + BYTES) + (7 + 1 + METRIC.len()) + (8 + 9) + (8 + 2 + car::NAME_MAX);
// The layout's names: the root holds these two.
const MANIFEST: &[u8] = b"manifest";
const VECTORS: &[u8] = b"vectors";
// A vector in a file: its entry's sequence (u32, little-endian), then its components.
const SEQ: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The journal does not check from its head: where its check stopped (`journal::check`).
    Journal(Refused),
    /// The journal refused the append, with its own error; `Conflict` if another writer appended first: append again.
    Append(journal::Error),
    /// The store refused or failed; its own error is the caller's to keep.
    Store,
    /// No index under the name.
    NotFound,
    /// The index's name was removed at this version.
    Removed(u64),
    /// Another writer published under the index's name since it was read: nothing was published.
    Conflict,
    /// A block of the index or an embedding the store does not hold.
    Missing,
    /// A block whose bytes do not match its CID.
    Corrupt,
    /// A block out of its layout: a directory, a manifest, a file of vectors or an embedding not exactly its encoding.
    Format,
    /// An embedding or a manifest of another schema version: the one found (MC-4.13).
    Version(u64),
    /// A manifest of another kind of index.
    Kind,
    /// A manifest of another metric.
    Metric,
    /// An index of another journal: its manifest names another, or the journal's entry at its sequence is another.
    Another,
    /// A journal shorter than the index: its version, below the index's sequence.
    Shorter(u64),
    /// A vector of the index whose entry holds no embedding by the model with its score: a false answer, at this
    /// sequence; the index is refused.
    False(u64),
    /// A query by another model than the index's.
    Model,
    /// A vector of other dimensions than the index's or the build's: 0 for the query or the build's, or the sequence of
    /// the entry whose embedding by the model has them. Nothing was published.
    Dims(u64),
    /// A search for no answer, or for more than NEAREST.
    Count,
    /// No embedding: no components or more than DIMS; a source that is no label.
    Invalid,
    /// Past the work area's files. Nothing was published.
    Full,
    /// Past FILES, or a directory over a block. Nothing was published.
    TooLarge,
    /// A journal's name the store does not take.
    Name,
}

/// An embedding: the vector `model` made of `subject`, each byte a component, a signed byte (two's complement).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Embedding<'a> { pub subject: Cid, pub model: Cid, pub vector: &'a [u8] }

impl Embedding<'_> {
    fn valid(&self) -> bool { (1..=DIMS).contains(&self.vector.len()) }

    /// Encodes the embedding into `out` (RECORD_MAX bytes at least); returns its length. No components, or more than
    /// DIMS, is Invalid.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, Error> {
        if !self.valid() { return Err(Error::Invalid); }
        let mut o = Out { bytes: out, at: 0 };
        encode_embedding(self, &mut o);
        Ok(o.at)
    }
}

// A CID as a byte string of its binary form: a mention, never a link (tag 42), so no walk follows it.
fn mention(o: &mut impl Sink, cid: &Cid) { o.head(2, cid::BYTES as u64); o.push(&cid.to_bytes()); }

fn mentioned(i: &mut In) -> Result<Cid, Error> {
    if i.expect(2).map_err(|_| Error::Format)? != cid::BYTES as u64 { return Err(Error::Format); }
    Cid::from_bytes(i.take(cid::BYTES).map_err(|_| Error::Format)?).map_err(|_| Error::Format)
}

// The keys in DAG-CBOR's order: shorter first, then by their bytes.
fn encode_embedding(e: &Embedding, o: &mut impl Sink) {
    o.head(5, 4);
    o.text("v");
    o.head(0, EMBEDDING);
    o.text("model");
    mention(o, &e.model);
    o.text("vector");
    o.head(2, e.vector.len() as u64);
    o.push(e.vector);
    o.text("subject");
    mention(o, &e.subject);
}

/// Decodes an embedding, only if it is exactly the canonical encoding of this schema; another version is refused by its
/// number.
pub fn decode_embedding(bytes: &[u8]) -> Result<Embedding<'_>, Error> {
    let format = |_| Error::Format;
    let mut i = In { bytes, at: 0 };
    let keys = i.expect(5).map_err(format)?;
    // "v" sorts first, so another schema version is told apart before anything else of it is read.
    i.key("v").map_err(format)?;
    let v = i.expect(0).map_err(format)?;
    if v != EMBEDDING { return Err(Error::Version(v)); }
    if keys != 4 { return Err(Error::Format); }
    i.key("model").map_err(format)?;
    let model = mentioned(&mut i)?;
    i.key("vector").map_err(format)?;
    let len = usize::try_from(i.expect(2).map_err(format)?).map_err(|_| Error::Format)?;
    let vector = i.take(len).map_err(format)?;
    i.key("subject").map_err(format)?;
    let subject = mentioned(&mut i)?;
    let e = Embedding { subject, model, vector };
    if i.at != bytes.len() || !e.valid() { return Err(Error::Format); }
    // Shortest forms: the encoding of what was read must be the bytes themselves.
    let mut same = Same { bytes, at: 0, equal: true };
    encode_embedding(&e, &mut same);
    if !same.equal || same.at != bytes.len() { return Err(Error::Format); }
    Ok(e)
}

/// The metric: the dot product of two vectors of as many components, each a signed byte. Exact: at most DIMS products
/// of at most 2^14 each.
pub fn score(a: &[u8], b: &[u8]) -> i32 { a.iter().zip(b).map(|(&x, &y)| x as i8 as i32 * y as i8 as i32).sum() }

/// What an index says of itself: the journal it was built from, the head's sequence and CID, the model whose embeddings
/// it holds, their dimensions, and how many.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Manifest { pub journal: Name, pub seq: u64, pub head: Cid, pub model: Cid, pub dims: u64, pub entries: u64 }

impl Manifest {
    // From 1, of an entry (`dag-cbor`), no more vectors than entries, 1 to DIMS components.
    fn valid(&self) -> bool { self.seq >= 1 && self.head.codec() == Codec::DagCbor && self.entries <= self.seq && (1..=DIMS as u64).contains(&self.dims) }

    /// Encodes the manifest into `out` (MANIFEST_MAX bytes at least); returns its length. One that makes no valid
    /// manifest is Format.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, Error> {
        if !self.valid() { return Err(Error::Format); }
        let mut o = Out { bytes: out, at: 0 };
        encode_manifest(self, &mut o);
        Ok(o.at)
    }
}

// The keys in DAG-CBOR's order: shorter first, then by their bytes.
fn encode_manifest(m: &Manifest, o: &mut impl Sink) {
    o.head(5, 9);
    o.text("v");
    o.head(0, VERSION);
    o.text("seq");
    o.head(0, m.seq);
    o.text("dims");
    o.head(0, m.dims);
    o.text("head");
    o.link(&m.head);
    o.text("kind");
    o.text(KIND);
    o.text("model");
    mention(o, &m.model);
    o.text("metric");
    o.text(METRIC);
    o.text("entries");
    o.head(0, m.entries);
    o.text("journal");
    o.text(m.journal.as_str());
}

/// Decodes a manifest, only if it is exactly the canonical encoding of this schema; another version, kind or metric is
/// refused as such.
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
    i.key("dims").map_err(format)?;
    let dims = i.expect(0).map_err(format)?;
    i.key("head").map_err(format)?;
    let head = i.link().map_err(format)?;
    i.key("kind").map_err(format)?;
    let len = usize::try_from(i.expect(3).map_err(format)?).map_err(|_| Error::Format)?;
    if i.take(len).map_err(format)? != KIND.as_bytes() { return Err(Error::Kind); }
    i.key("model").map_err(format)?;
    let model = mentioned(&mut i)?;
    i.key("metric").map_err(format)?;
    let len = usize::try_from(i.expect(3).map_err(format)?).map_err(|_| Error::Format)?;
    if i.take(len).map_err(format)? != METRIC.as_bytes() { return Err(Error::Metric); }
    i.key("entries").map_err(format)?;
    let entries = i.expect(0).map_err(format)?;
    i.key("journal").map_err(format)?;
    let len = usize::try_from(i.expect(3).map_err(format)?).map_err(|_| Error::Format)?;
    let journal = core::str::from_utf8(i.take(len).map_err(format)?).ok().and_then(Name::new).ok_or(Error::Format)?;
    let m = Manifest { journal, seq, head, model, dims, entries };
    if i.at != bytes.len() || !m.valid() { return Err(Error::Format); }
    // Shortest integer forms: the encoding of what was read must be the bytes themselves.
    let mut same = Same { bytes, at: 0, equal: true };
    encode_manifest(&m, &mut same);
    if !same.equal || same.at != bytes.len() { return Err(Error::Format); }
    Ok(m)
}

// A read of the index or of an embedding, as this module tells what it met.
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

/// The embedding `cid` names: a `raw` block, read and checked against its CID, then decoded.
pub fn read<'b, S: Store>(store: &mut S, cid: &Cid, buffer: &'b mut [u8; CHUNK]) -> Result<Embedding<'b>, Error> {
    if cid.codec() != Codec::Raw { return Err(Error::Format); }
    let len = fetch(store, cid, buffer).map_err(fault)?;
    let buffer: &'b [u8; CHUNK] = buffer;
    decode_embedding(&buffer[..len])
}

// The embedding by `model` that entry `e` holds, if it holds one: of `dims` components, or refused at its sequence. An
// entry of kind `embedding` whose content is no embedding is refused as its read is.
fn of_model<'b, S: Store>(s: &mut S, e: &Entry, model: &Cid, dims: usize, block: &'b mut [u8; CHUNK]) -> Result<Option<Embedding<'b>>, Error> {
    if e.kind.as_str() != ENTRY { return Ok(None); }
    let r = read(s, &e.content, block)?;
    if r.model != *model { return Ok(None); }
    if r.vector.len() != dims { return Err(Error::Dims(e.seq)); }
    Ok(Some(r))
}

// Checks the journal whole from its head (`journal::check_each`), `each` hearing the store, each entry, its CID and a
// block to read through: once `each` fails it hears no more, and a journal that does not check is told first.
fn through<S: Store>(store: &mut S, journal: &str, steps: u64, entry: &mut [u8; CHUNK], block: &mut [u8; CHUNK],
                     mut each: impl FnMut(&mut S, &Entry, &Cid, &mut [u8; CHUNK]) -> Result<(), Error>) -> Result<journal::Checked, Error> {
    let mut failed = None;
    let checked = journal::check_each(store, journal, steps, entry, |s, e, cid| {
        if failed.is_none() { failed = each(s, e, cid, block).err(); }
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

fn stale(version: u64, seq: u64) -> State { if version == seq { State::Current } else { State::Stale(version - seq) } }

/// A file of vectors as its directory needs it, kept while the next ones are written: its CID and size.
#[derive(Clone, Copy, Debug)]
pub struct Kept { key: [u8; cid::BYTES], size: u32 }
impl Kept { pub const EMPTY: Kept = Kept { key: [0; cid::BYTES], size: 0 }; }

/// The work area: the files a build keeps for its directory, and the blocks every call reads and writes through. The
/// caller's, so a program keeps them in statics; its bound is the files'.
pub struct Work<'w> { kept: &'w mut [Kept], blocks: &'w mut [[u8; CHUNK]; 3] }
impl<'w> Work<'w> {
    pub fn new(kept: &'w mut [Kept], blocks: &'w mut [[u8; CHUNK]; 3]) -> Self { Work { kept, blocks } }
}

// The name of file `k` under `vectors`: four lowercase hexadecimal digits, so the names' order is the files'.
fn file_name(k: usize) -> [u8; 4] { core::array::from_fn(|i| b"0123456789abcdef"[(k >> (12 - 4 * i)) & 15]) }

// Vectors in a file of `dims` components: the bytes of one, and how many a file holds.
fn sizes(dims: usize) -> (usize, usize) { (SEQ + dims, CHUNK / (SEQ + dims)) }

// Puts a file of vectors and keeps its CID and size for the directory: past FILES is TooLarge, past the work area Full.
fn file<S>(s: &mut S, data: &[u8], kept: &mut [Kept], files: &mut usize, put: &mut impl FnMut(&mut S, Codec, &[u8]) -> Result<Cid, Error>) -> Result<(), Error> {
    if *files == FILES { return Err(Error::TooLarge); }
    if *files == kept.len() { return Err(Error::Full); }
    let cid = put(s, Codec::Raw, data)?;
    kept[*files] = Kept { key: cid.to_bytes(), size: data.len() as u32 };
    *files += 1;
    Ok(())
}

// Puts a directory of `count` entries, given one by one; its CID.
fn directory<'a, S>(s: &mut S, count: usize, entry: impl Fn(usize) -> tree::Entry<'a>, out: &mut [u8; CHUNK], put: &mut impl FnMut(&mut S, Codec, &[u8]) -> Result<Cid, Error>) -> Result<Cid, Error> {
    let len = tree::encode_each(count, entry, out).map_err(|e| if e == dag::Error::TooLarge { Error::TooLarge } else { Error::Format })?;
    put(s, Codec::DagCbor, &out[..len])
}

// Writes the index of the embeddings by `model` in `journal` up to `upto`, the index's sequence and head (none: the
// journal's head): the journal checked whole from its head, within `steps` entries, each embedding read and checked
// against its CID, and its vector put into the file being filled as it comes, newest first, each file full but the last;
// then the vectors' directory, the manifest and the root. `put` stores each block, or for a check only names it. Returns
// the journal's version, the manifest and the root.
fn write_out<S: Store>(store: &mut S, journal: &str, upto: Option<(u64, Cid)>, model: &Cid, dims: usize, steps: u64, work: &mut Work,
                       put: &mut impl FnMut(&mut S, Codec, &[u8]) -> Result<Cid, Error>) -> Result<(u64, Manifest, Cid), Error> {
    let name = Name::new(journal).ok_or(Error::Name)?;
    if !(1..=DIMS).contains(&dims) { return Err(Error::Dims(0)); }
    let Work { kept, blocks } = work;
    let [a, b, c] = &mut **blocks;
    let (size, per) = sizes(dims);
    let (mut top, mut head, mut entries, mut fill, mut files) = (None, None, 0, 0, 0);
    let checked = through(store, journal, steps, a, b, |s, e, cid, b| {
        // The head comes first: without `upto`, the index is of it.
        let version = *top.get_or_insert(e.seq);
        let (seq, at) = upto.unwrap_or((version, *cid));
        meets(seq, &at, version, e, cid)?;
        if e.seq > seq { return Ok(()); }
        if e.seq == seq { head = Some(*cid); }
        let Some(r) = of_model(s, e, model, dims, b)? else { return Ok(()) };
        let posted = u32::try_from(e.seq).map_err(|_| Error::Full)?;
        c[fill..fill + SEQ].copy_from_slice(&posted.to_le_bytes());
        c[fill + SEQ..fill + size].copy_from_slice(r.vector);
        (fill, entries) = (fill + size, entries + 1);
        if fill == per * size {
            file(s, &c[..fill], kept, &mut files, put)?;
            fill = 0;
        }
        Ok(())
    })?;
    if fill > 0 { file(store, &c[..fill], kept, &mut files, put)?; }
    let seq = upto.map_or(checked.version, |(seq, _)| seq);
    let head = head.ok_or(Error::Store)?;
    let manifest = Manifest { journal: name, seq, head, model: *model, dims: dims as u64, entries };
    let names: [[u8; 4]; FILES] = core::array::from_fn(file_name);
    let kept = &**kept;
    let vectors = directory(store, files, |k| tree::Entry { name: &names[k], kind: Kind::File, size: kept[k].size as u64, cid: Cid::from_bytes(&kept[k].key).unwrap() }, a, put)?;
    let len = manifest.encode(&mut b[..])?;
    let file = put(store, Codec::Raw, &b[..len])?;
    let root = [tree::Entry { name: MANIFEST, kind: Kind::File, size: len as u64, cid: file }, tree::Entry { name: VECTORS, kind: Kind::Directory, size: files as u64, cid: vectors }];
    let root = directory(store, root.len(), |i| root[i], a, put)?;
    Ok((checked.version, manifest, root))
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

// The index under `name`, of `journal`: its root and manifest read and checked, and the vectors' directory (CID,
// entries).
fn open<S: Store>(store: &mut S, name: &str, journal: &str, buffer: &mut [u8; CHUNK]) -> Result<(Indexed, (Cid, u64)), Error> {
    let (version, root) = journal::Store::head(store, name).map_err(stored)?;
    let root = root.ok_or(if version == 0 { Error::NotFound } else { Error::Removed(version) })?;
    let dir = read_dir(store, &root, None, buffer)?;
    let (m, v) = match (dir.len(), dir.find(MANIFEST), dir.find(VECTORS)) {
        (2, Some(m), Some(v)) if m.kind == Kind::File && v.kind == Kind::Directory => ((m.cid, m.size), (v.cid, v.size)),
        _ => return Err(Error::Format),
    };
    let manifest = decode(read_file(store, &m.0, m.1, buffer)?)?;
    if manifest.journal.as_str() != journal { return Err(Error::Another); }
    Ok((Indexed { version, root, manifest }, v))
}

// Each vector of the index's files under `vectors` (CID, files), with its entry's sequence, newest first: only exactly
// the layout a build writes is read (each file named by its place, full but the last, the sequences descending from the
// manifest's and as many as it says), every file checked against its CID.
fn vectors_of<S: Store>(store: &mut S, vectors: (Cid, u64), m: &Manifest, dir: &mut [u8; CHUNK], block: &mut [u8; CHUNK], mut each: impl FnMut(u64, &[u8])) -> Result<(), Error> {
    let (size, per) = sizes(m.dims as usize);
    let full = (per * size) as u64;
    let dir = read_dir(store, &vectors.0, Some(vectors.1), dir)?;
    let (mut last, mut count) = (m.seq + 1, 0);
    for k in 0..dir.len() {
        let f = dir.entry(k);
        let shape = k < FILES && f.name == &file_name(k)[..] && f.kind == Kind::File && f.size > 0 && f.size % size as u64 == 0
            && if k + 1 < dir.len() { f.size == full } else { f.size <= full };
        if !shape { return Err(Error::Format); }
        for v in read_file(store, &f.cid, f.size, block)?.chunks_exact(size) {
            let seq = u32::from_le_bytes([v[0], v[1], v[2], v[3]]) as u64;
            if seq == 0 || seq >= last { return Err(Error::Format); }
            each(seq, &v[SEQ..]);
            (last, count) = (seq, count + 1);
        }
    }
    if count != m.entries { return Err(Error::Format); }
    Ok(())
}

/// What a build made: the index under its name, at the version that holds its root; `published` if this build
/// published it, not if the name held that root already.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Built { pub index: Indexed, pub published: bool }

/// Builds the index of the embeddings by `model`, of `dims` components, in `journal` from its head and publishes it under
/// `name` by compare-and-swap from the version read first: the journal checked whole from its head, within `steps`
/// entries, and each embedding read and checked against its CID. Embeddings by another model are not in it; one by the
/// model of other dimensions is refused (Dims), as is an entry of kind `embedding` whose content is no embedding. The
/// same head gives the same root, and a name that holds it already is not published again. Past the work area is Full,
/// past FILES TooLarge, and another writer's publication since Conflict: nothing is published. Files written before a
/// refusal are retained by nothing, and a collection frees them.
pub fn build<S: Store>(store: &mut S, journal: &str, name: &str, model: &Cid, dims: usize, steps: u64, work: &mut Work) -> Result<Built, Error> {
    let (version, current) = journal::Store::head(store, name).map_err(stored)?;
    let (_, manifest, root) = write_out(store, journal, None, model, dims, steps, work, &mut |s, codec, data| {
        let cid = Blocks::put(s, codec, data).map_err(fault)?;
        // The store's answer is checked, as `dag::Builder` checks it.
        if cid != Cid::of(codec, data) { return Err(Error::Corrupt); }
        Ok(cid)
    })?;
    if current == Some(root) { return Ok(Built { index: Indexed { version, root, manifest }, published: false }); }
    let version = journal::Store::publish(store, name, version, &root).map_err(stored)?;
    Ok(Built { index: Indexed { version, root, manifest }, published: true })
}

/// The index under `name` and whether it is current: the journal checked whole from its head, its entry at the index's
/// sequence the index's head. Refused: a journal that does not check (Journal), whose entry there is another (Another),
/// or that is shorter (Shorter).
pub fn state<S: Store>(store: &mut S, journal: &str, name: &str, steps: u64, work: &mut Work) -> Result<(Indexed, State), Error> {
    let [a, b, _] = &mut *work.blocks;
    let (index, _) = open(store, name, journal, b)?;
    let m = index.manifest;
    let mut top = None;
    let checked = through(store, journal, steps, a, b, |_, e, cid, _| meets(m.seq, &m.head, *top.get_or_insert(e.seq), e, cid))?;
    Ok((index, stale(checked.version, m.seq)))
}

/// An answer: an entry's sequence, its vector's score against the query, and the object its embedding is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Near { pub seq: u64, pub score: i32, pub subject: Cid }

// The best answers so far, at most `k`: by score, highest first, then by sequence, newest first; each with its subject
// once its entry is read.
struct Best { at: [(u64, i32, Option<Cid>); NEAREST], len: usize, k: usize }
impl Best {
    fn new(k: usize) -> Result<Best, Error> {
        if !(1..=NEAREST).contains(&k) { return Err(Error::Count); }
        Ok(Best { at: [(0, 0, None); NEAREST], len: 0, k })
    }
    fn offer(&mut self, seq: u64, score: i32, subject: Option<Cid>) {
        let at = self.at[..self.len].iter().position(|&(s, v, _)| score > v || (score == v && seq > s)).unwrap_or(self.len);
        if at == self.k { return; }
        let end = self.len.min(self.k - 1);
        self.at.copy_within(at..end, at + 1);
        self.at[at] = (seq, score, subject);
        self.len = (self.len + 1).min(self.k);
    }
    fn find(&self, seq: u64) -> Option<usize> { self.at[..self.len].iter().position(|a| a.0 == seq) }
    // Each answer in order, every one with its subject.
    fn each(&self, mut each: impl FnMut(&Near)) -> Result<(), Error> {
        for &(seq, score, subject) in &self.at[..self.len] { each(&Near { seq, score, subject: subject.ok_or(Error::False(seq))? }); }
        Ok(())
    }
}

/// What a search answered: the index read, whether it is current or how many entries it lacks, how many vectors were
/// scored (the index's, and the model's among the entries above its head), and how many answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found { pub index: Indexed, pub state: State, pub vectors: u64, pub nearest: u64 }

/// The `k` entries of `journal` whose embeddings by `model` score highest against `query` (the dot product), highest
/// first and of equal scores the newest first, from the index under `name`: every vector of the index scored, then the
/// journal read from its head as `state` reads it, the embeddings by the model above the index's head scored, and each
/// answer below it checked, its entry's embedding read again and scored. An answer whose entry holds no embedding by the
/// model with that score is a false answer (False), and the index is refused; so each answer is true. A vector the
/// index lacks or holds lower than its entry's is not found by a search, and `verify` tells it (Differs). A query by
/// another model (Model) or of other dimensions (Dims) is refused. `each` hears the answers once all are checked.
pub fn nearest<S: Store>(store: &mut S, journal: &str, name: &str, model: &Cid, query: &[u8], k: usize, steps: u64, work: &mut Work, each: impl FnMut(&Near)) -> Result<Found, Error> {
    let mut best = Best::new(k)?;
    let [a, b, c] = &mut *work.blocks;
    let (index, vectors) = open(store, name, journal, b)?;
    let m = index.manifest;
    if m.model != *model { return Err(Error::Model); }
    if query.len() as u64 != m.dims { return Err(Error::Dims(0)); }
    vectors_of(store, vectors, &m, a, c, |seq, v| best.offer(seq, score(query, v), None))?;
    let (mut top, mut above) = (None, 0);
    let checked = through(store, journal, steps, a, b, |s, e, cid, b| {
        meets(m.seq, &m.head, *top.get_or_insert(e.seq), e, cid)?;
        if e.seq > m.seq {
            if let Some(r) = of_model(s, e, model, query.len(), b)? {
                best.offer(e.seq, score(query, r.vector), Some(r.subject));
                above += 1;
            }
        } else if let Some(at) = best.find(e.seq) {
            match of_model(s, e, model, query.len(), b)? {
                Some(r) if score(query, r.vector) == best.at[at].1 => best.at[at].2 = Some(r.subject),
                _ => return Err(Error::False(e.seq)),
            }
        }
        Ok(())
    })?;
    best.each(each)?;
    Ok(Found { index, state: stale(checked.version, m.seq), vectors: m.entries + above, nearest: best.len as u64 })
}

/// The exact scan, the answer without an index: the `k` entries of `journal` whose embeddings by `model` score highest
/// against `query`, in `nearest`'s order, the journal checked whole from its head and every embedding by the model read
/// and scored; one of other dimensions is refused (Dims). Returns how many it scored; `each` hears the answers once the
/// journal is checked.
pub fn scan<S: Store>(store: &mut S, journal: &str, model: &Cid, query: &[u8], k: usize, steps: u64, work: &mut Work, each: impl FnMut(&Near)) -> Result<u64, Error> {
    let mut best = Best::new(k)?;
    if !(1..=DIMS).contains(&query.len()) { return Err(Error::Dims(0)); }
    let [a, b, _] = &mut *work.blocks;
    let mut n = 0;
    through(store, journal, steps, a, b, |s, e, _, b| {
        if let Some(r) = of_model(s, e, model, query.len(), b)? {
            best.offer(e.seq, score(query, r.vector), Some(r.subject));
            n += 1;
        }
        Ok(())
    })?;
    best.each(each)?;
    Ok(n)
}

/// Checks the index under `name`: rebuilt from the index's own head with its manifest's model and dimensions, nothing
/// put, and its root compared with the one published: Equal, Stale by how many entries the journal has grown since, or
/// Differs. Refused as `state` refuses, and as a build is.
pub fn verify<S: Store>(store: &mut S, journal: &str, name: &str, steps: u64, work: &mut Work) -> Result<(Indexed, Verdict), Error> {
    let (index, _) = open(store, name, journal, &mut work.blocks[1])?;
    let m = index.manifest;
    let (version, _, root) = write_out(store, journal, Some((m.seq, m.head)), &m.model, m.dims as usize, steps, work, &mut |_, codec, data| Ok(Cid::of(codec, data)))?;
    let verdict = if root != index.root { Verdict::Differs } else if version == m.seq { Verdict::Equal } else { Verdict::Stale(version - m.seq) };
    Ok((index, verdict))
}

/// What an `add` appended: the entry's sequence and CID, and the embedding's CID, its content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Added { pub seq: u64, pub cid: Cid, pub entry: Cid }

/// Appends the embedding `e` to `journal` from `source`: put as a `raw` block, the content of an entry of kind
/// `embedding` (`journal::append`, by compare-and-swap; `Append(Conflict)` if another writer appended first, nothing
/// appended). A source that is no label, or no embedding, is refused (Invalid), nothing put. The subject and the model
/// need not be held: they are mentioned, not retained.
pub fn add<S: Store>(store: &mut S, journal: &str, now: Reading, source: &str, e: &Embedding, buffer: &mut [u8; CHUNK]) -> Result<Added, Error> {
    if Label::new(source).is_none() { return Err(Error::Invalid); }
    let len = e.encode(&mut buffer[..])?;
    let cid = Blocks::put(store, Codec::Raw, &buffer[..len]).map_err(fault)?;
    // The store's answer is checked, as `dag::Builder` checks it.
    if cid != Cid::of(Codec::Raw, &buffer[..len]) { return Err(Error::Corrupt); }
    let done = journal::append(store, journal, now, source, ENTRY, &cid, buffer).map_err(Error::Append)?;
    Ok(Added { seq: done.entry.seq, cid, entry: done.cid })
}
