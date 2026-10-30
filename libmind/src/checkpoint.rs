//! Checkpoints of a component's state in the block store (306-STO-0009, docs/storage/checkpoints.md; MC-6.10, 6.11,
//! 6.12, Appendix B.4). A checkpoint is a manifest block (this module's format, versioned) and the state's object,
//! published together under two names by one commit, so they change at once or not at all. The names' version is the
//! instance's generation: a save names the version it restored, so a stale instance's save is refused (fenced). The
//! manifest lists resources by logical name and authorities as text, never as capabilities: a restored instance gets
//! its rights from its current grants. Effects begun but not recorded come back pending, to be reconciled, not
//! repeated. Format 2 (313-STO-0026) keeps each client's last operation, so a repeated request is answered and not
//! applied again; how far an input queue was read; and the messages owed to others, sent again until acknowledged.
//! No system calls: tests/checkpoint_host.rs.
use crate::cid::{self, Cid};

pub const MAGIC: &[u8; 8] = b"MIND-CKP";
/// The manifest format's version; another is refused by its number, never read as this one.
pub const FORMAT: u16 = 2;
/// Bytes of a label (a contract, a resource, an authority, an effect) at most.
pub const LABEL: usize = 32;
pub const RESOURCES: usize = 8;
pub const AUTHORITIES: usize = 8;
pub const EFFECTS: usize = 16;
pub const CLIENTS: usize = 16;
pub const OUTBOX: usize = 16;
/// Bytes of an encoded manifest at most.
pub const MAX: usize = 8 + 2 + 1 + LABEL + 2 + 2 + 8 + 8 + cid::BYTES + 5 + 8 + 8 + (RESOURCES + AUTHORITIES) * (1 + LABEL)
    + EFFECTS * (8 + 1 + 1 + LABEL) + CLIENTS * (1 + LABEL + 8 + 8 + 8) + OUTBOX * (8 + 2 * (1 + LABEL));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The store refused or failed; its own error is the caller's to keep.
    Store,
    /// The manifest's block is not one of this format.
    Format,
    /// A manifest of another format version, the one found: not read as this one.
    Version(u16),
    /// A checkpoint of another contract, or another version of it: not read as this one.
    Contract,
    /// A state of another schema version.
    Schema,
    /// The manifest and the state's name disagree (version or root): not restored.
    Inconsistent,
    /// Another instance saved since this one restored: it no longer holds the component (MC-6.12).
    Fenced,
    /// A label too long or not printable, or more entries than the format holds.
    Invalid,
}

/// A short printable text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label { len: u8, bytes: [u8; LABEL] }
impl Label {
    pub const EMPTY: Label = Label { len: 0, bytes: [0; LABEL] };
    pub fn new(text: &str) -> Option<Label> {
        let b = text.as_bytes();
        if b.len() > LABEL || !b.iter().all(|c| (0x21..0x7f).contains(c)) { return None; }
        let mut l = Label { len: b.len() as u8, ..Label::EMPTY };
        l.bytes[..b.len()].copy_from_slice(b);
        Some(l)
    }
    pub fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("") }
}
impl core::fmt::Display for Label { fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { f.write_str(self.as_str()) } }

/// What became of an external effect. Pending: begun and not recorded, so after a restore its outcome is unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome { Pending, Done, Failed }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect { pub id: u64, pub what: Label, pub outcome: Outcome }
impl Effect { pub const EMPTY: Effect = Effect { id: 0, what: Label::EMPTY, outcome: Outcome::Done }; }

/// A client's last operation applied: its id, the result it was answered, and the sequence it was applied at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Client { pub who: Label, pub op: u64, pub result: u64, pub at: u64 }
impl Client { pub const EMPTY: Client = Client { who: Label::EMPTY, op: 0, result: 0, at: 0 }; }

/// What a client's operation is to the state: new, a repeat of the last one (answered with its result), or older.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied { New, Repeat(u64), Stale }

/// A message owed to another: its sequence among this component's, where it goes and what it is (its bytes are in the
/// state).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message { pub seq: u64, pub to: Label, pub what: Label }
impl Message { pub const EMPTY: Message = Message { seq: 0, to: Label::EMPTY, what: Label::EMPTY }; }

/// A checkpoint's manifest: the contract it follows, the instance generation and consistency point it records, the
/// state's root, the resources, authorities and effects the state depends on, its clients' last operations, how far
/// its input was read and the messages it owes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub contract: Label,
    pub contract_version: u16,
    pub schema: u16,
    /// The generation of the instance that saved it: the version of the checkpoint's names it wrote.
    pub epoch: u64,
    /// The consistency point: requests applied to the state.
    pub sequence: u64,
    pub state: Cid,
    /// How far an input queue was read: the number its source gave the last message taken.
    pub inbox: u64,
    // The last sequence a message was given, so none is given twice.
    sent: u64,
    resources: [Label; RESOURCES],
    authorities: [Label; AUTHORITIES],
    effects: [Effect; EFFECTS],
    clients: [Client; CLIENTS],
    outbox: [Message; OUTBOX],
    counts: [u8; 5],
}

impl Manifest {
    pub fn new(contract: Label, contract_version: u16, schema: u16, state: Cid) -> Self {
        Manifest {
            contract, contract_version, schema, epoch: 0, sequence: 0, state, inbox: 0, sent: 0, resources: [Label::EMPTY; RESOURCES],
            authorities: [Label::EMPTY; AUTHORITIES], effects: [Effect::EMPTY; EFFECTS], clients: [Client::EMPTY; CLIENTS],
            outbox: [Message::EMPTY; OUTBOX], counts: [0; 5],
        }
    }
    pub fn resources(&self) -> &[Label] { &self.resources[..self.counts[0] as usize] }
    pub fn authorities(&self) -> &[Label] { &self.authorities[..self.counts[1] as usize] }
    pub fn effects(&self) -> &[Effect] { &self.effects[..self.counts[2] as usize] }
    pub fn clients(&self) -> &[Client] { &self.clients[..self.counts[3] as usize] }
    pub fn outbox(&self) -> &[Message] { &self.outbox[..self.counts[4] as usize] }
    /// Effects begun and never recorded: each must be reconciled before it is tried again (Appendix B.4).
    pub fn pending(&self) -> impl Iterator<Item = &Effect> { self.effects().iter().filter(|e| e.outcome == Outcome::Pending) }
    pub fn add_resource(&mut self, name: &str) -> Result<(), Error> {
        let l = Label::new(name).ok_or(Error::Invalid)?;
        let n = self.counts[0] as usize;
        if n == RESOURCES { return Err(Error::Invalid); }
        self.resources[n] = l;
        self.counts[0] += 1;
        Ok(())
    }
    pub fn add_authority(&mut self, name: &str) -> Result<(), Error> {
        let l = Label::new(name).ok_or(Error::Invalid)?;
        let n = self.counts[1] as usize;
        if n == AUTHORITIES { return Err(Error::Invalid); }
        self.authorities[n] = l;
        self.counts[1] += 1;
        Ok(())
    }
    /// Records an effect about to begin, as pending; returns its id. Done and failed effects are dropped first when
    /// the journal is full.
    pub fn begin(&mut self, what: &str) -> Result<u64, Error> {
        let what = Label::new(what).ok_or(Error::Invalid)?;
        if self.counts[2] as usize == EFFECTS {
            let keep = self.effects.iter().position(|e| e.outcome != Outcome::Pending).ok_or(Error::Invalid)?;
            self.effects.copy_within(keep + 1.., keep);
            self.counts[2] -= 1;
        }
        let id = self.effects().iter().map(|e| e.id).max().unwrap_or(0) + 1;
        self.effects[self.counts[2] as usize] = Effect { id, what, outcome: Outcome::Pending };
        self.counts[2] += 1;
        Ok(id)
    }
    /// Records the outcome of effect `id`; false if there is no such effect.
    pub fn settle(&mut self, id: u64, outcome: Outcome) -> bool {
        let n = self.counts[2] as usize;
        match self.effects[..n].iter_mut().find(|e| e.id == id) { Some(e) => { e.outcome = outcome; true } None => false }
    }

    /// What operation `op` of client `who` is: new if above its last one (or the client is not kept), a repeat of the
    /// last one with its result, or stale. Repeats are told only for the CLIENTS clients applied most recently.
    pub fn applied(&self, who: &str, op: u64) -> Applied {
        match self.clients().iter().find(|c| c.who.as_str() == who) {
            Some(c) if op == c.op => Applied::Repeat(c.result),
            Some(c) if op < c.op => Applied::Stale,
            _ => Applied::New,
        }
    }
    /// Records operation `op` of client `who` as applied at this sequence, with the result it is answered; the client
    /// applied longest ago is dropped when CLIENTS are kept. Invalid for an id not above the client's last.
    pub fn record(&mut self, who: &str, op: u64, result: u64) -> Result<(), Error> {
        let label = Label::new(who).ok_or(Error::Invalid)?;
        let n = self.counts[3] as usize;
        let k = match self.clients[..n].iter().position(|c| c.who == label) {
            Some(k) if op <= self.clients[k].op => return Err(Error::Invalid),
            Some(k) => k,
            None if op == 0 => return Err(Error::Invalid),
            None if n < CLIENTS => { self.counts[3] += 1; n }
            None => (0..n).min_by_key(|&k| self.clients[k].at).unwrap(),
        };
        self.clients[k] = Client { who: label, op, result, at: self.sequence };
        Ok(())
    }

    /// Owes message `what` to `to`; returns its sequence, above every one given before. Invalid when OUTBOX are owed.
    pub fn send(&mut self, to: &str, what: &str) -> Result<u64, Error> {
        let (to, what) = (Label::new(to).ok_or(Error::Invalid)?, Label::new(what).ok_or(Error::Invalid)?);
        let n = self.counts[4] as usize;
        if n == OUTBOX { return Err(Error::Invalid); }
        self.sent += 1;
        self.outbox[n] = Message { seq: self.sent, to, what };
        self.counts[4] += 1;
        Ok(self.sent)
    }
    /// Message `seq` was acknowledged: it is owed no more. False if it was not owed.
    pub fn delivered(&mut self, seq: u64) -> bool {
        let n = self.counts[4] as usize;
        let Some(k) = self.outbox[..n].iter().position(|m| m.seq == seq) else { return false };
        self.outbox.copy_within(k + 1..n, k);
        self.outbox[n - 1] = Message::EMPTY;
        self.counts[4] -= 1;
        true
    }

    /// The canonical bytes; `out` holds MAX.
    pub fn encode(&self, out: &mut [u8]) -> usize {
        let mut w = Out { out, at: 0 };
        w.put(MAGIC);
        w.put(&FORMAT.to_le_bytes());
        w.label(&self.contract);
        w.put(&self.contract_version.to_le_bytes());
        w.put(&self.schema.to_le_bytes());
        w.put(&self.epoch.to_le_bytes());
        w.put(&self.sequence.to_le_bytes());
        w.put(&self.state.to_bytes());
        w.put(&self.counts);
        w.put(&self.inbox.to_le_bytes());
        w.put(&self.sent.to_le_bytes());
        for l in self.resources().iter().chain(self.authorities()) { w.label(l); }
        for e in self.effects() {
            w.put(&e.id.to_le_bytes());
            w.put(&[match e.outcome { Outcome::Pending => 0, Outcome::Done => 1, Outcome::Failed => 2 }]);
            w.label(&e.what);
        }
        for c in self.clients() {
            w.label(&c.who);
            for v in [c.op, c.result, c.at] { w.put(&v.to_le_bytes()); }
        }
        for m in self.outbox() {
            w.put(&m.seq.to_le_bytes());
            w.label(&m.to);
            w.label(&m.what);
        }
        w.at
    }

    /// The manifest `bytes` encode, if they are exactly one of this format version.
    pub fn decode(bytes: &[u8]) -> Result<Manifest, Error> {
        let mut r = In { bytes, at: 0 };
        if r.take(8)? != MAGIC { return Err(Error::Format); }
        match r.u16()? { FORMAT => {} other => return Err(Error::Version(other)) }
        let contract = r.label()?;
        let (contract_version, schema, epoch, sequence) = (r.u16()?, r.u16()?, r.u64()?, r.u64()?);
        let state = Cid::from_bytes(r.take(cid::BYTES)?).map_err(|_| Error::Format)?;
        let mut m = Manifest::new(contract, contract_version, schema, state);
        (m.epoch, m.sequence) = (epoch, sequence);
        let counts = r.take(5)?;
        if counts[0] as usize > RESOURCES || counts[1] as usize > AUTHORITIES || counts[2] as usize > EFFECTS
            || counts[3] as usize > CLIENTS || counts[4] as usize > OUTBOX { return Err(Error::Format); }
        m.counts.copy_from_slice(counts);
        (m.inbox, m.sent) = (r.u64()?, r.u64()?);
        for k in 0..counts[0] as usize { m.resources[k] = r.label()?; }
        for k in 0..counts[1] as usize { m.authorities[k] = r.label()?; }
        for k in 0..counts[2] as usize {
            let id = r.u64()?;
            let outcome = match r.take(1)?[0] { 0 => Outcome::Pending, 1 => Outcome::Done, 2 => Outcome::Failed, _ => return Err(Error::Format) };
            m.effects[k] = Effect { id, what: r.label()?, outcome };
        }
        for k in 0..counts[3] as usize {
            let who = r.label()?;
            let (op, result, at) = (r.u64()?, r.u64()?, r.u64()?);
            // Each client once, with an operation applied; at a sequence not past the manifest's.
            if op == 0 || at > m.sequence || m.clients[..k].iter().any(|c| c.who == who) { return Err(Error::Format); }
            m.clients[k] = Client { who, op, result, at };
        }
        for k in 0..counts[4] as usize {
            let seq = r.u64()?;
            // Owed in the order given, none past the last sequence given.
            if seq == 0 || seq > m.sent || (k > 0 && seq <= m.outbox[k - 1].seq) { return Err(Error::Format); }
            m.outbox[k] = Message { seq, to: r.label()?, what: r.label()? };
        }
        if r.at != bytes.len() { return Err(Error::Format); }
        Ok(m)
    }
}

struct Out<'a> { out: &'a mut [u8], at: usize }
impl Out<'_> {
    fn put(&mut self, b: &[u8]) { self.out[self.at..self.at + b.len()].copy_from_slice(b); self.at += b.len(); }
    fn label(&mut self, l: &Label) { self.put(&[l.len]); self.put(&l.bytes[..l.len as usize]); }
}

struct In<'a> { bytes: &'a [u8], at: usize }
impl<'a> In<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let b = self.bytes.get(self.at..self.at + n).ok_or(Error::Format)?;
        self.at += n;
        Ok(b)
    }
    fn u16(&mut self) -> Result<u16, Error> { Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap())) }
    fn u64(&mut self) -> Result<u64, Error> { Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap())) }
    fn label(&mut self) -> Result<Label, Error> {
        let len = self.take(1)?[0] as usize;
        core::str::from_utf8(self.take(len)?).ok().and_then(Label::new).ok_or(Error::Format)
    }
}

/// The store as checkpoints need it: raw blocks, and two names read and changed at once.
pub trait Store {
    fn put(&mut self, data: &[u8]) -> Result<Cid, Error>;
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error>;
    /// Each name's current version and root (0 and none: absent; a version and none: removed), read at one point.
    fn snapshot(&mut self, names: [&str; 2]) -> Result<[(u64, Option<Cid>); 2], Error>;
    /// Both names from `expected` to their roots, at once or not at all; Fenced if either moved on.
    fn commit(&mut self, names: [&str; 2], expected: u64, roots: [Cid; 2]) -> Result<u64, Error>;
}

/// The two names of `component`'s checkpoint: its manifest and its state.
pub struct Names { manifest: [u8; 64], state: [u8; 64], len: usize }
impl Names {
    pub fn of(component: &str) -> Option<Names> {
        if component.is_empty() || component.len() > LABEL || !component.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c)) { return None; }
        let mut n = Names { manifest: [0; 64], state: [0; 64], len: 11 + component.len() };
        n.manifest[..11].copy_from_slice(b"checkpoint/");
        n.manifest[11..n.len].copy_from_slice(component.as_bytes());
        n.state[..n.len].copy_from_slice(&n.manifest[..n.len]);
        n.state[n.len..n.len + 6].copy_from_slice(b"/state");
        Some(n)
    }
    pub fn manifest(&self) -> &str { core::str::from_utf8(&self.manifest[..self.len]).unwrap_or("") }
    pub fn state(&self) -> &str { core::str::from_utf8(&self.state[..self.len + 6]).unwrap_or("") }
}

/// What a restore found: no checkpoint (none yet, or removed at `epoch`), or the last one; a save follows from its
/// epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restored { Fresh { epoch: u64 }, Found(Manifest) }
impl Restored {
    pub fn epoch(&self) -> u64 { match self { Restored::Fresh { epoch } => *epoch, Restored::Found(m) => m.epoch } }
}

/// The last checkpoint of `component`, if it follows `contract` at `version` with state of `schema`, and its two names
/// agree. `buffer` holds MAX. Its authorities are text: the caller checks them against what it holds now (`rebind`).
pub fn restore(store: &mut impl Store, names: &Names, contract: &str, version: u16, schema: u16, buffer: &mut [u8]) -> Result<Restored, Error> {
    let [(mv, manifest), (sv, state)] = store.snapshot([names.manifest(), names.state()])?;
    let (manifest, state) = match (manifest, state) {
        (None, None) if mv == sv => return Ok(Restored::Fresh { epoch: mv }),
        (Some(m), Some(s)) if mv == sv => (m, s),
        _ => return Err(Error::Inconsistent),
    };
    let n = store.get(&manifest, &mut buffer[..MAX])?;
    let m = Manifest::decode(&buffer[..n])?;
    if m.contract.as_str() != contract || m.contract_version != version { return Err(Error::Contract); }
    if m.schema != schema { return Err(Error::Schema); }
    if m.state != state || m.epoch != mv { return Err(Error::Inconsistent); }
    Ok(Restored::Found(m))
}

/// Saves `manifest` (its state's object already stored) as the next checkpoint after the one restored at epoch
/// `from` (0: none): its epoch becomes `from + 1`, both names change at once, and the change is durable when this
/// returns. Fenced if another instance saved since `from`. `buffer` holds MAX.
pub fn save(store: &mut impl Store, names: &Names, manifest: &mut Manifest, from: u64, buffer: &mut [u8]) -> Result<u64, Error> {
    manifest.epoch = from + 1;
    let n = manifest.encode(&mut buffer[..MAX]);
    let root = store.put(&buffer[..n])?;
    store.commit([names.manifest(), names.state()], from, [root, manifest.state])
}

/// The authorities the checkpoint records that `holds` says the instance does not hold now: protected actions that
/// need them must be refused or degraded (MC-6.11). A recorded authority is never turned back into a right.
pub fn rebind<'m>(manifest: &'m Manifest, holds: impl Fn(&str) -> bool) -> impl Iterator<Item = &'m Label> {
    manifest.authorities().iter().filter(move |a| !holds(a.as_str()))
}
