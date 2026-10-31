//! Checkpoints of a component's state in the block store (306-STO-0009, docs/storage/checkpoints.md; MC-6.10, 6.11,
//! 6.12, Appendix B.4). A checkpoint is a manifest block (this module's format, versioned) and the state's object,
//! published together under two names by one commit, so they change at once or not at all. The names' version is the
//! instance's generation: a save names the version it restored, so a stale instance's save is refused (fenced). The
//! manifest lists resources by logical name and authorities as text, never as capabilities: a restored instance gets
//! its rights from its current grants. Effects begun but not recorded come back pending, to be reconciled, not
//! repeated. Format 2 (313-STO-0026) keeps each client's last operation, so a repeated request is answered and not
//! applied again; how far an input queue was read; and the messages owed to others, sent again until acknowledged.
//! Between components (313-STO-0030) the receiver reads the sender's checkpoint and applies what it owes once, the
//! sender its client; the sender clears what the receiver's checkpoint shows applied. Each writes only its own.
//! A move (313-STO-0033) releases the component, both names to a release record by one compare-and-swap, so no instance
//! in this store saves it again, and its last checkpoint, which the names' history keeps, goes to a copy; imported into
//! a store that never held its names, a manifest takes their first version as its epoch. Format 3 (313-STO-0036) saves
//! with an effect's intent the key its target tells it by, a publication by compare-and-swap: `reconcile` asks the
//! target whether it was done, and does it now if it was not, never twice. No system calls: tests/checkpoint_host.rs.
use crate::cid::{self, Cid};

pub const MAGIC: &[u8; 8] = b"MIND-CKP";
/// A release record's magic: the component was moved out of this store.
pub const RELEASED: &[u8; 8] = b"MIND-RLS";
/// The manifest format's version; another is refused by its number, never read as this one.
pub const FORMAT: u16 = 3;
/// The release record's format version: its layout is the one of the manifest's format 2.
pub const RELEASE_FORMAT: u16 = 2;
/// Bytes of a label (a contract, a resource, an authority, an effect) at most.
pub const LABEL: usize = 32;
/// Bytes of a publication's name at most: a name of the store, 1 to 64 of `A-Z a-z 0-9 . _ / -`.
pub const NAME: usize = 64;
pub const RESOURCES: usize = 8;
pub const AUTHORITIES: usize = 8;
pub const EFFECTS: usize = 16;
pub const CLIENTS: usize = 16;
pub const OUTBOX: usize = 16;
/// Bytes of an encoded manifest at most.
pub const MAX: usize = 8 + 2 + 1 + LABEL + 2 + 2 + 8 + 8 + cid::BYTES + 5 + 8 + 8 + (RESOURCES + AUTHORITIES) * (1 + LABEL)
    + EFFECTS * (8 + 1 + 1 + LABEL + 1 + 1 + NAME + 8 + cid::BYTES) + CLIENTS * (1 + LABEL + 8 + 8 + 8) + OUTBOX * (8 + 2 * (1 + LABEL));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The store refused or failed; its own error is the caller's to keep.
    Store,
    /// The manifest's block is not one of this format.
    Format,
    /// A manifest of another format version, the one found: not read as this one.
    Version(u16),
    /// A checkpoint of another contract, or another version of it, the version found: not read as this one.
    Contract(u16),
    /// A state of another schema version, the one found.
    Schema(u16),
    /// The manifest and the state's name disagree (version or root): not restored.
    Inconsistent,
    /// Another instance saved since this one restored: it no longer holds the component (MC-6.12).
    Fenced,
    /// Released at this epoch, moved out of this store: no instance here restores or saves it again (MC-6.12).
    Released(u64),
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

/// An external effect: its id, what it is, its outcome, and the key its target tells it by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect { pub id: u64, pub what: Label, pub outcome: Outcome, pub key: Key }
impl Effect { pub const EMPTY: Effect = Effect { id: 0, what: Label::EMPTY, outcome: Outcome::Done, key: Key::None }; }

/// What an effect's target tells it by, saved with its intent (format 3): none, so only a person reconciles it, or a
/// publication by compare-and-swap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key { None, Publication(Publication) }

/// A publication by compare-and-swap: `root` under a name of the store from its version `from`, so the name takes it
/// at `from + 1` once, or never.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Publication { name: [u8; NAME], len: u8, pub from: u64, pub root: Cid }
impl Publication {
    /// None unless `name` is a name of the store.
    pub fn new(name: &str, from: u64, root: Cid) -> Option<Publication> {
        let b = name.as_bytes();
        if !(1..=NAME).contains(&b.len()) || !b.iter().all(|c| c.is_ascii_alphanumeric() || b"._/-".contains(c)) { return None; }
        let mut p = Publication { name: [0; NAME], len: b.len() as u8, from, root };
        p.name[..b.len()].copy_from_slice(b);
        Some(p)
    }
    pub fn name(&self) -> &str { core::str::from_utf8(&self.name[..self.len as usize]).unwrap_or("") }
    /// What `target` tells of it now: the name's current version, and past `from + 1` the versions the target keeps.
    /// A name below `from` was forgotten by the target since: unknown.
    pub fn tell(&self, target: &mut impl Target) -> Result<Told, Error> {
        let (now, root) = target.head(self.name())?;
        if now == self.from { return Ok(Told::NotDone); }
        if now < self.from { return Ok(Told::Unknown); }
        let at = if now == self.from + 1 { Some(root) } else { target.version(self.name(), self.from + 1)? };
        Ok(match at { Some(Some(r)) if r == self.root => Told::Done, Some(_) => Told::Failed, None => Told::Unknown })
    }
    /// Done now by its compare-and-swap from `from`: Now if it lands; refused, the name moved on, and the target is
    /// asked again, so another instance's swap of the same root is told done.
    pub fn apply(&self, target: &mut impl Target) -> Result<Told, Error> {
        if target.publish(self.name(), self.from, &self.root)?.is_some() { return Ok(Told::Now); }
        self.tell(target)
    }
}

/// What a publication's target tells: done (the name took its root at `from + 1`), not done (the name still at
/// `from`), done now (it was not, and this swap landed), failed (`from + 1` holds another root or removed the name), or
/// unknown (the name past `from + 1`, a version the target no longer keeps: for a person).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Told { Done, NotDone, Now, Failed, Unknown }
impl Told {
    /// The outcome it settles; pending if it settles none.
    pub fn outcome(self) -> Outcome {
        match self { Told::Done | Told::Now => Outcome::Done, Told::Failed => Outcome::Failed, Told::NotDone | Told::Unknown => Outcome::Pending }
    }
}

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
    /// The generation of the instance that saved it: the version of the checkpoint's names it wrote; restored after a
    /// move, their first version in this store.
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
    pub fn begin(&mut self, what: &str) -> Result<u64, Error> { self.begin_keyed(what, Key::None) }
    /// The same, with the key its target tells it by, saved with the intent (format 3).
    pub fn begin_keyed(&mut self, what: &str, key: Key) -> Result<u64, Error> {
        let what = Label::new(what).ok_or(Error::Invalid)?;
        if self.counts[2] as usize == EFFECTS {
            let keep = self.effects.iter().position(|e| e.outcome != Outcome::Pending).ok_or(Error::Invalid)?;
            self.effects.copy_within(keep + 1.., keep);
            self.counts[2] -= 1;
        }
        let id = self.effects().iter().map(|e| e.id).max().unwrap_or(0) + 1;
        self.effects[self.counts[2] as usize] = Effect { id, what, outcome: Outcome::Pending, key };
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
    /// The messages owed to `to`, in sequence order.
    pub fn owed<'m>(&'m self, to: &'m str) -> impl Iterator<Item = &'m Message> + 'm { self.outbox().iter().filter(move |m| m.to.as_str() == to) }

    /// Applies the messages `sender` (its last checkpoint) owes `me`, this component, that it has not applied, in
    /// sequence order: `from` is the sender as a client, each sequence its operation, so one applied before is not
    /// applied again. `apply` gives each one's result to record; its error stops the rest. Returns how many it applied.
    pub fn receive(&mut self, me: &str, from: &str, sender: &Manifest, mut apply: impl FnMut(&Message) -> Result<u64, Error>) -> Result<usize, Error> {
        let mut applied = 0;
        for m in sender.owed(me) {
            if self.applied(from, m.seq) != Applied::New { continue; }
            let result = apply(m)?;
            self.record(from, m.seq, result)?;
            applied += 1;
        }
        Ok(applied)
    }
    /// Clears the messages owed to `to` that `receiver`, its last checkpoint, shows applied: those up to the last
    /// operation its clients table holds for `me`, this component. `each` sees each one; returns how many.
    pub fn acknowledge(&mut self, me: &str, to: &str, receiver: &Manifest, mut each: impl FnMut(&Message)) -> usize {
        let Some(last) = receiver.clients().iter().find(|c| c.who.as_str() == me).map(|c| c.op) else { return 0 };
        let mut cleared = 0;
        loop {
            let Some(m) = self.owed(to).find(|m| m.seq <= last).copied() else { return cleared };
            each(&m);
            self.delivered(m.seq);
            cleared += 1;
        }
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
            match &e.key {
                Key::None => w.put(&[0]),
                Key::Publication(p) => {
                    w.put(&[1, p.len]);
                    w.put(&p.name[..p.len as usize]);
                    w.put(&p.from.to_le_bytes());
                    w.put(&p.root.to_bytes());
                }
            }
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
            let what = r.label()?;
            let key = match r.take(1)?[0] {
                0 => Key::None,
                1 => {
                    let len = r.take(1)?[0] as usize;
                    let name = core::str::from_utf8(r.take(len)?).map_err(|_| Error::Format)?;
                    let from = r.u64()?;
                    let root = Cid::from_bytes(r.take(cid::BYTES)?).map_err(|_| Error::Format)?;
                    Key::Publication(Publication::new(name, from, root).ok_or(Error::Format)?)
                }
                _ => return Err(Error::Format),
            };
            m.effects[k] = Effect { id, what, outcome, key };
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

/// A release: the epoch of the component's last checkpoint in this store, and that checkpoint's manifest and state,
/// which the names' history retains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Release { pub epoch: u64, pub manifest: Cid, pub state: Cid }
impl Release {
    /// The canonical bytes: RELEASED, the format version, the epoch and the two CIDs; `out` holds MAX.
    pub fn encode(&self, out: &mut [u8]) -> usize {
        let mut w = Out { out, at: 0 };
        w.put(RELEASED);
        w.put(&RELEASE_FORMAT.to_le_bytes());
        w.put(&self.epoch.to_le_bytes());
        w.put(&self.manifest.to_bytes());
        w.put(&self.state.to_bytes());
        w.at
    }
    /// The release `bytes` encode, if they are exactly one of this format version.
    pub fn decode(bytes: &[u8]) -> Result<Release, Error> {
        let mut r = In { bytes, at: 0 };
        if r.take(8)? != RELEASED { return Err(Error::Format); }
        match r.u16()? { RELEASE_FORMAT => {} other => return Err(Error::Version(other)) }
        let epoch = r.u64()?;
        let manifest = Cid::from_bytes(r.take(cid::BYTES)?).map_err(|_| Error::Format)?;
        let state = Cid::from_bytes(r.take(cid::BYTES)?).map_err(|_| Error::Format)?;
        if r.at != bytes.len() { return Err(Error::Format); }
        Ok(Release { epoch, manifest, state })
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

/// A publication's target as reconciling asks it: a name's current version, a version it keeps, and the
/// compare-and-swap.
pub trait Target {
    /// `name`'s current version and root (0 and none: never published; a version and none: removed).
    fn head(&mut self, name: &str) -> Result<(u64, Option<Cid>), Error>;
    /// Version `version` of `name` if the target still keeps it: its root, or none if that version removed the name.
    fn version(&mut self, name: &str, version: u64) -> Result<Option<Option<Cid>>, Error>;
    /// `root` published under `name` if it is at `expected`: its new version; None if it is not.
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<Option<u64>, Error>;
}

/// Reconciles each pending effect with a key by asking `target`, never trying it blindly (Appendix B.4): done before,
/// or not done and done now by its own compare-and-swap, it is settled done; failed, failed; unknown, or the target's
/// error, it stays pending. `each` sees each effect asked and the answer. Returns how many it settled, to be saved
/// before a request is applied.
pub fn reconcile(manifest: &mut Manifest, target: &mut impl Target, mut each: impl FnMut(&Effect, Result<Told, Error>)) -> usize {
    let mut settled = 0;
    for k in 0..manifest.counts[2] as usize {
        let (Outcome::Pending, Key::Publication(p)) = (manifest.effects[k].outcome, manifest.effects[k].key) else { continue };
        let told = p.tell(target).and_then(|t| if t == Told::NotDone { p.apply(target) } else { Ok(t) });
        let outcome = told.map_or(Outcome::Pending, Told::outcome);
        if outcome != Outcome::Pending { manifest.effects[k].outcome = outcome; settled += 1; }
        each(&manifest.effects[k], told);
    }
    settled
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
/// agree; its epoch is their version. A manifest of another epoch is taken only at their first version, moved in from
/// another store; elsewhere Inconsistent. Released if both names hold a release, never fresh. `buffer` holds MAX. Its authorities are text: the caller checks them against what it
/// holds now (`rebind`).
pub fn restore(store: &mut impl Store, names: &Names, contract: &str, version: u16, schema: u16, buffer: &mut [u8]) -> Result<Restored, Error> {
    let [(mv, manifest), (sv, state)] = store.snapshot([names.manifest(), names.state()])?;
    let (manifest, state) = match (manifest, state) {
        (None, None) if mv == sv => return Ok(Restored::Fresh { epoch: mv }),
        (Some(m), Some(s)) if mv == sv => (m, s),
        _ => return Err(Error::Inconsistent),
    };
    let n = store.get(&manifest, &mut buffer[..MAX])?;
    if buffer[..n].starts_with(RELEASED) {
        Release::decode(&buffer[..n])?;
        return Err(if state == manifest { Error::Released(mv) } else { Error::Inconsistent });
    }
    let mut m = Manifest::decode(&buffer[..n])?;
    if m.contract.as_str() != contract || m.contract_version != version { return Err(Error::Contract(m.contract_version)); }
    if m.schema != schema { return Err(Error::Schema(m.schema)); }
    // Another epoch only at the names' first version, as an import into a store that never held them publishes it.
    if m.state != state || (m.epoch != mv && mv != 1) { return Err(Error::Inconsistent); }
    m.epoch = mv;
    Ok(Restored::Found(m))
}

/// Releases the component whose checkpoint was restored at epoch `from`: both names to a release record by one commit
/// from `from`, so every instance of that epoch is fenced and every later restore in this store refused (Released). The
/// checkpoint stays the names' version `from`, which their history keeps, and the release names it. Fenced if another
/// instance saved since; Invalid if no checkpoint is at `from`. Returns the release and its epoch. `buffer` holds MAX.
pub fn release(store: &mut impl Store, names: &Names, from: u64, buffer: &mut [u8]) -> Result<(Release, u64), Error> {
    let [(mv, manifest), (sv, state)] = store.snapshot([names.manifest(), names.state()])?;
    if mv != from || sv != from { return Err(Error::Fenced); }
    let (Some(manifest), Some(state)) = (manifest, state) else { return Err(Error::Invalid) };
    // Both names at one block: a release already, never a checkpoint (its manifest names its state).
    if manifest == state { return Err(Error::Released(from)); }
    let r = Release { epoch: from, manifest, state };
    let n = r.encode(&mut buffer[..MAX]);
    let root = store.put(&buffer[..n])?;
    Ok((r, store.commit([names.manifest(), names.state()], from, [root, root])?))
}

/// The release of a component moved out of this store, which `restore` refuses: it names the last checkpoint, which the
/// names' history keeps, to be written to a copy again. Inconsistent unless both names hold one release. `buffer` holds
/// MAX.
pub fn released(store: &mut impl Store, names: &Names, buffer: &mut [u8]) -> Result<Release, Error> {
    match store.snapshot([names.manifest(), names.state()])? {
        [(mv, Some(manifest)), (sv, Some(state))] if mv == sv && manifest == state => {
            let n = store.get(&manifest, &mut buffer[..MAX])?;
            Release::decode(&buffer[..n])
        }
        _ => Err(Error::Inconsistent),
    }
}

/// Another component's last checkpoint, read and not restored: its manifest, checked as `restore` checks it, and its
/// state's bytes in `state` (their length). None if it has none. Nothing is written, so its owner is never fenced.
pub fn read(store: &mut impl Store, names: &Names, contract: &str, version: u16, schema: u16, buffer: &mut [u8], state: &mut [u8]) -> Result<Option<(Manifest, usize)>, Error> {
    let Restored::Found(m) = restore(store, names, contract, version, schema, buffer)? else { return Ok(None) };
    Ok(Some((m, store.get(&m.state, state)?)))
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
