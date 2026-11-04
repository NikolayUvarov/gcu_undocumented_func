//! Copies of chosen names on a second medium (309-STO-0040; MC-4.4, 4.8, 5.1). The copy is a whole store of its own on
//! that medium: its own superblock, volume key, index, names, pins and buffers. A name has a copy when the copy holds it
//! with a root, so the choice lives where the copy does and nothing in the store's layout changes. Each change of such a
//! name is made on the store first; then every block under its new root the copy lacks is put there, read from the
//! store and checked against its CID; then the copy's name is moved to the root by its own compare-and-swap. A name
//! whose copy did not take a step is behind, and is brought in step between requests. A block one medium finds damaged
//! is repaired from the other, on a read and by the scrub. Only the volume's names are copied: a memory domain's records
//! are sealed under the domain's key, which the copy does not hold. Nothing here depends on the kernel: the host tests
//! build it with store.rs.
use crate::cid::Cid;
use crate::dag;
use crate::store::{valid_name, Device, Error, Scrubbed, Store, Update, BLOCK_MAX, COMMIT_MAX, NAME_MAX, VOLUME};

/// Names the list of those behind holds; past them, every name of the copy is compared again once the list is empty.
pub const BEHIND: usize = 64;
/// Damaged blocks one scrub step repairs at most; the next pass finds the others again.
pub const REPAIRS: usize = 32;

/// The store's own medium, or its copy's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Medium { Store, Copy }
impl Medium {
    fn at(self) -> usize { self as usize }
    fn other(self) -> Self { match self { Medium::Store => Medium::Copy, Medium::Copy => Medium::Store } }
}

/// The step of following a change that the copy did not take: its blocks put there, or its name moved there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step { Blocks, Name }

/// What the copy tells the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event<'n> {
    /// A name's version copied: its version on the copy, and the blocks put there.
    Copied { name: &'n [u8], version: u64, blocks: u32 },
    /// A name removed on the copy, at that version there.
    Removed { name: &'n [u8], version: u64 },
    /// A name behind on the copy: the step that did not take, and why.
    Behind { name: &'n [u8], step: Step, error: Error },
    /// A name the copy did not hold, not copied: the step that did not take, and why.
    NotCopied { name: &'n [u8], step: Step, error: Error },
    /// A name only the copy holds, left as it is.
    OnlyOnCopy { name: &'n [u8], version: u64, root: Cid },
    /// A damaged block of `into` stored again there from the other medium.
    Repaired { cid: Cid, into: Medium },
    /// A damaged block of `on` the other medium did not give back whole, or that could not be stored again.
    NotRepaired { cid: Cid, on: Medium, error: Error },
    /// The collection after repairs on `on`: the blocks it freed, or why it did not run.
    Collected { on: Medium, freed: Result<u32, Error> },
}

/// What the copy holds: its medium's sectors and those used, its blocks, the names it holds a root of, the names behind
/// on it, and the blocks repaired since the mount, of the store from the copy and of the copy from the store.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats { pub sectors: u64, pub used: u64, pub blocks: u32, pub names: u32, pub behind: u32, pub from_copy: u32, pub from_store: u32 }

// A name, held by value.
#[derive(Clone, Copy)]
struct Name { bytes: [u8; NAME_MAX], len: u8 }
impl Name {
    fn new(name: &[u8]) -> Self { let mut n = Name { bytes: [0; NAME_MAX], len: name.len() as u8 }; n.bytes[..name.len()].copy_from_slice(name); n }
    fn get(&self) -> &[u8] { &self.bytes[..self.len as usize] }
}

// The names behind on the copy, oldest first; one that does not fit asks for every name to be compared again.
struct Behind { names: [Name; BEHIND], count: usize, rescan: bool }
impl Behind {
    fn push(&mut self, name: &[u8]) {
        if self.names[..self.count].iter().any(|n| n.get() == name) { return; }
        if self.count == BEHIND { self.rescan = true; return; }
        self.names[self.count] = Name::new(name);
        self.count += 1;
    }
    fn remove(&mut self, name: &[u8]) {
        let Some(k) = self.names[..self.count].iter().position(|n| n.get() == name) else { return };
        self.names.copy_within(k + 1..self.count, k);
        self.count -= 1;
    }
    fn pop(&mut self) -> Option<Name> {
        if self.count == 0 { return None; }
        let first = self.names[0];
        self.remove(first.get());
        Some(first)
    }
}

// What following a name on the copy did.
enum Followed { InStep, Copied { version: u64, blocks: u32 }, Removed { version: u64 }, OnlyOnCopy { version: u64, root: Cid } }

/// The copy on the second medium, beside the store it copies.
pub struct Copies<'a, C: Device> {
    copy: Store<'a, C>,
    // A block on its way from one medium to the other.
    block: &'a mut [u8; BLOCK_MAX],
    behind: Behind,
    // Blocks repaired since the mount, and whether a collection is to free the damaged records: of each medium.
    repaired: [u32; 2],
    collect: [bool; 2],
    // The medium the scrub reads next: the store's pass, then the copy's.
    scrubbing: Medium,
}

/// Where the copy's log goes.
pub type Log<'l> = &'l mut dyn FnMut(Event);

// Runs `f` with `main` in the volume's domain, then in the one it was in: only the volume's names are copied.
fn in_volume<'s, D: Device, T>(main: &mut Store<'s, D>, f: impl FnOnce(&mut Store<'s, D>) -> T) -> T {
    let domain = main.domain();
    let _ = main.set_domain(VOLUME);
    let done = f(main);
    let _ = main.set_domain(domain);
    done
}

impl<'a, C: Device> Copies<'a, C> {
    /// The copy mounted on its medium, beside `main`: each name it holds with a root is compared with the store's. The
    /// same root is in step; another root, or a removal in the store, is behind; a name the store does not hold at all
    /// is left as it is, and logged, so a store that came back blank never empties its copy. If the copy's mount found
    /// records corrupt, every name is behind, so its blocks are looked at again, and the copy's next pass ends with a
    /// collection that frees the damaged records.
    pub fn new<D: Device>(copy: Store<'a, C>, block: &'a mut [u8; BLOCK_MAX], main: &mut Store<D>, log: Log) -> Self {
        let behind = Behind { names: [Name { bytes: [0; NAME_MAX], len: 0 }; BEHIND], count: 0, rescan: false };
        let suspect = copy.stats().corrupt > 0;
        let mut copies = Copies { copy, block, behind, repaired: [0; 2], collect: [false, suspect], scrubbing: Medium::Store };
        in_volume(main, |main| copies.compare(main, &mut *log));
        copies
    }

    // Compares every name the copy holds with a root with the store's (main in the volume's domain).
    fn compare<D: Device>(&mut self, main: &Store<D>, log: Log) {
        self.behind.rescan = false;
        let suspect = self.copy.stats().corrupt > 0;
        let Ok(names) = self.copy.names(b"", b"") else { return };
        for (name, _, _) in names {
            match need(&self.copy, main, name) {
                Ok(Need::OnlyOnCopy { version, root }) => log(Event::OnlyOnCopy { name, version, root }),
                Ok(Need::Copy { root, at: Some(at), .. }) if root == at && !suspect => {}
                _ => self.behind.push(name),
            }
        }
    }

    /// The time (ns, monotonic) of the next requests, for the copy's leases.
    pub fn set_time(&mut self, now: u64) { self.copy.set_time(now); }

    /// Whether the volume's `name` has a copy: the copy holds it with a root.
    pub fn holds(&self, name: &[u8]) -> bool { self.copy.snapshot(name).is_ok_and(|(_, root)| root.is_some()) }

    /// How many names are behind on the copy, as the list holds them.
    pub fn behind(&self) -> usize { self.behind.count + self.behind.rescan as usize }

    pub fn stats(&self) -> Stats {
        let s = self.copy.stats();
        let names = self.copy.names(b"", b"").map_or(0, |n| n.count() as u32);
        Stats { sectors: s.sectors, used: s.used, blocks: s.blocks, names, behind: self.behind() as u32, from_copy: self.repaired[0], from_store: self.repaired[1] }
    }

    /// The copy's own store: what it holds, and its medium.
    #[cfg(test)]
    pub fn copy_store(&mut self) -> &mut Store<'a, C> { &mut self.copy }

    // Whether a change of `name` in main's domain is followed on the copy.
    fn follows<D: Device>(&self, main: &Store<D>, name: &[u8]) -> bool { main.domain() == VOLUME && self.holds(name) }

    /// Publishes as `Store::publish`; then, if the name has a copy, follows it there. Returns the store's version, and
    /// whether the copy took every step (`copied`); a copy that did not leaves the name behind.
    pub fn publish<D: Device>(&mut self, main: &mut Store<D>, name: &[u8], expected: u64, root: &Cid, owner: u16, log: Log) -> Result<(u64, bool), Error> {
        let version = main.publish(name, expected, root, owner)?;
        Ok((version, self.follows(main, name) && self.follow_logged(main, name, log)))
    }

    /// Removes as `Store::unpublish`; then, if the name has a copy, removes it there, so it has none after.
    pub fn unpublish<D: Device>(&mut self, main: &mut Store<D>, name: &[u8], expected: u64, owner: u16, log: Log) -> Result<(u64, bool), Error> {
        let version = main.unpublish(name, expected, owner)?;
        Ok((version, self.follows(main, name) && self.follow_logged(main, name, log)))
    }

    /// Commits as `Store::commit`; then the names of it that have a copy change in one commit on the copy, after their
    /// blocks. `copied` only if every name of the commit has a copy and the copy took every step; else its copied names
    /// are behind, or were followed while the others reached the store alone.
    pub fn commit<D: Device>(&mut self, main: &mut Store<D>, updates: &[Update], owner: u16, log: Log) -> Result<([u64; COMMIT_MAX], bool), Error> {
        let versions = main.commit(updates, owner)?;
        let mut follows = [false; COMMIT_MAX];
        for (f, u) in follows.iter_mut().zip(updates) { *f = self.follows(main, u.name); }
        let n = updates.len();
        if !follows[..n].iter().any(|&f| f) { return Ok((versions, false)); }
        let mut done = [(0u64, 0u32, false); COMMIT_MAX];
        let followed = self.commit_on_copy(main, updates, &follows[..n], owner, &mut done);
        for (k, u) in updates.iter().enumerate().filter(|(k, _)| follows[*k]) {
            match followed {
                Ok(()) => {
                    self.behind.remove(u.name);
                    let (version, blocks, changed) = done[k];
                    if !changed && blocks == 0 { continue; }
                    if u.root.is_some() { log(Event::Copied { name: u.name, version, blocks }) } else { log(Event::Removed { name: u.name, version }) }
                }
                Err((step, error)) => { self.behind.push(u.name); log(Event::Behind { name: u.name, step, error }); }
            }
        }
        Ok((versions, followed.is_ok() && follows[..n].iter().all(|&f| f)))
    }

    // The blocks of each followed change put on the copy, then one commit there of those not in step: for each, its
    // version on the copy, the blocks put and whether it changed.
    fn commit_on_copy<D: Device>(&mut self, main: &mut Store<D>, updates: &[Update], follows: &[bool], owner: u16, done: &mut [(u64, u32, bool); COMMIT_MAX]) -> Result<(), (Step, Error)> {
        for (k, u) in updates.iter().enumerate().filter(|(k, _)| follows[*k]) {
            if let Some(root) = u.root { done[k].1 = self.blocks(main, &root).map_err(|e| (Step::Blocks, e))?; }
        }
        let mut changes = [Update { name: &[], expected: 0, root: None }; COMMIT_MAX];
        let mut which = [0usize; COMMIT_MAX];
        let mut m = 0;
        for (k, u) in updates.iter().enumerate().filter(|(k, _)| follows[*k]) {
            let (version, root) = self.copy.snapshot(u.name).map_err(|e| (Step::Name, e))?;
            if root == u.root { done[k].0 = version; continue; }
            changes[m] = Update { name: u.name, expected: version, root: u.root };
            which[m] = k;
            m += 1;
        }
        if m == 0 { return Ok(()); }
        let versions = self.copy.commit(&changes[..m], owner).map_err(|e| (Step::Name, e))?;
        for j in 0..m { (done[which[j]].0, done[which[j]].2) = (versions[j], true); }
        Ok(())
    }

    /// Turns the copy of the volume's `name` on or off. On: the name's current version is copied, its blocks then the
    /// name, and Ok(true) once the copy holds it at the store's root; a name the copy held already but that did not take a
    /// step is behind, Ok(false); one it did not hold, the copy's refusal. Off: the copy's own removal of the name, whose
    /// next collection there frees what only it retained, Ok(false). Errors: invalid (a name of a memory domain, whose
    /// records the copy could not open, or a name no store takes); not-found (on: the store holds no live version; off:
    /// the copy holds no root of it).
    pub fn copy<D: Device>(&mut self, main: &mut Store<D>, name: &[u8], on: bool, owner: u16, log: Log) -> Result<bool, Error> {
        if !valid_name(name) || main.domain() != VOLUME { return Err(Error::Invalid); }
        if !on {
            let (version, root) = self.copy.snapshot(name)?;
            if root.is_none() { return Err(Error::NotFound); }
            let version = self.copy.unpublish(name, version, owner)?;
            self.behind.remove(name);
            log(Event::Removed { name, version });
            return Ok(false);
        }
        if main.snapshot(name)?.1.is_none() { return Err(Error::NotFound); }
        let held = self.holds(name);
        match self.follow(main, name) {
            Ok(done) => { self.behind.remove(name); tell(done, name, log); Ok(true) }
            Err((step, error)) if held => { self.behind.push(name); log(Event::Behind { name, step, error }); Ok(false) }
            Err((step, error)) => { log(Event::NotCopied { name, step, error }); Err(error) }
        }
    }

    // Follows `name` on the copy and logs what it did; a step not taken puts it behind. Whether it is in step.
    fn follow_logged<D: Device>(&mut self, main: &mut Store<D>, name: &[u8], log: Log) -> bool {
        match self.follow(main, name) {
            Ok(done) => { self.behind.remove(name); tell(done, name, log); true }
            Err((step, error)) => { self.behind.push(name); log(Event::Behind { name, step, error }); false }
        }
    }

    // Brings the copy of `name` to the store's current version (main in the volume's domain): the blocks under its root
    // the copy lacks put there, then the copy's name moved to the root from the copy's own version; or removed there if
    // the store removed it. A name the store does not hold at all is left as it is.
    fn follow<D: Device>(&mut self, main: &mut Store<D>, name: &[u8]) -> Result<Followed, (Step, Error)> {
        match need(&self.copy, main, name).map_err(|e| (Step::Name, e))? {
            Need::OnlyOnCopy { version, root } => Ok(Followed::OnlyOnCopy { version, root }),
            Need::Nothing => Ok(Followed::InStep),
            Need::Remove { held } => self.copy.unpublish(name, held, owner_of(main, name)).map(|version| Followed::Removed { version }).map_err(|e| (Step::Name, e)),
            // At the same root too the blocks are looked at: the copy's mount may have found one damaged.
            Need::Copy { root, held, at } => {
                let blocks = self.blocks(main, &root).map_err(|e| (Step::Blocks, e))?;
                if at == Some(root) { return Ok(if blocks == 0 { Followed::InStep } else { Followed::Copied { version: held, blocks } }); }
                let version = self.copy.publish(name, held, &root, owner_of(main, name)).map_err(|e| (Step::Name, e))?;
                Ok(Followed::Copied { version, blocks })
            }
        }
    }

    // Puts on the copy every block under `root` it lacks, each read from the store and checked against its CID, the
    // store's walk visiting each; returns how many.
    fn blocks<D: Device>(&mut self, main: &mut Store<D>, root: &Cid) -> Result<u32, Error> {
        let (copy, block) = (&mut self.copy, &mut *self.block);
        let (mut put, mut failed) = (0u32, None);
        let walked = main.walk(root, |store, cid| {
            if copy.has(cid) { return Ok(()); }
            let stored = store.get(cid, &mut block[..]).and_then(|len| copy.put(cid.codec(), &block[..len]));
            match stored {
                Ok(_) => { put += 1; Ok(()) }
                Err(e) => { failed = Some(e); Err(dag::Error::Store) }
            }
        });
        match (failed, walked) { (Some(e), _) | (None, Err(e)) => Err(e), (None, Ok(_)) => Ok(put) }
    }

    /// The block `cid` names, as `Store::get` gives it; in the volume's domain, one the store finds damaged, or lacks
    /// while the copy holds it, is read from the copy, checked against its CID, put back in the store, and returned. One
    /// the copy finds damaged too is not returned.
    pub fn get<D: Device>(&mut self, main: &mut Store<D>, cid: &Cid, out: &mut [u8], log: Log) -> Result<usize, Error> {
        let got = main.get(cid, out);
        if !matches!(got, Err(Error::Corrupt | Error::NotFound)) || main.domain() != VOLUME || !self.copy.has(cid) { return got; }
        let len = match self.copy.get(cid, out) {
            Ok(len) => len,
            Err(error) => { log(Event::NotRepaired { cid: *cid, on: Medium::Store, error }); return got; }
        };
        match main.put(cid.codec(), &out[..len]) {
            Ok(_) => self.repaired_into(Medium::Store, cid, log),
            Err(error) => log(Event::NotRepaired { cid: *cid, on: Medium::Store, error }),
        }
        Ok(len)
    }

    fn repaired_into(&mut self, into: Medium, cid: &Cid, log: Log) {
        self.repaired[into.at()] += 1;
        self.collect[into.at()] = true;
        log(Event::Repaired { cid: *cid, into });
    }

    // Repairs the block `cid` found damaged on `into` from the other medium, if that medium gives it back whole (main in
    // the volume's domain). The damaged copy leaves the index only once the whole one is in hand, and is stored again.
    fn repair<D: Device>(&mut self, main: &mut Store<D>, cid: &Cid, into: Medium, log: Log) {
        let block = &mut *self.block;
        // Both indexes name it: the store's in the volume's domain (a domain's block has no copy), and the other's.
        if !main.has(cid) || !self.copy.has(cid) { return log(Event::NotRepaired { cid: *cid, on: into, error: Error::NotFound }); }
        let source = match into { Medium::Store => self.copy.get(cid, &mut block[..]), Medium::Copy => main.get(cid, &mut block[..]) };
        let len = match source { Ok(len) => len, Err(error) => return log(Event::NotRepaired { cid: *cid, on: into, error }) };
        let stored = match into {
            Medium::Store => main.check_block(cid).and_then(|whole| if whole { Ok(false) } else { main.put(cid.codec(), &block[..len]).map(|_| true) }),
            Medium::Copy => self.copy.check_block(cid).and_then(|whole| if whole { Ok(false) } else { self.copy.put(cid.codec(), &block[..len]).map(|_| true) }),
        };
        match stored {
            Ok(true) => self.repaired_into(into, cid, log),
            // It reads back whole now: nothing to repair.
            Ok(false) => {}
            Err(error) => log(Event::NotRepaired { cid: *cid, on: into, error }),
        }
    }

    /// Brings the oldest name behind in step, `main` in any domain: Some(true) once it is, Some(false) if the copy did not
    /// take a step (the name goes to the end of the list), None if no name is behind. A list that overflowed has every
    /// name of the copy compared again first.
    pub fn catch_up<D: Device>(&mut self, main: &mut Store<D>, log: Log) -> Option<bool> {
        if self.behind.count == 0 && self.behind.rescan { in_volume(main, |main| self.compare(main, &mut *log)); }
        let name = self.behind.pop()?;
        Some(in_volume(main, |main| self.follow_logged(main, name.get(), &mut *log)))
    }

    /// Whether a scrub's pass is under way on either medium, or the copy's is due after the store's.
    pub fn scrubbing<D: Device>(&self, main: &Store<D>) -> bool { main.scrubbing() || self.copy.scrubbing() || self.scrubbing == Medium::Copy }

    /// Drops the passes under way: the next scrub starts at the store's first record.
    pub fn restart_scrub<D: Device>(&mut self, main: &mut Store<D>) {
        main.restart_scrub();
        self.copy.restart_scrub();
        self.scrubbing = Medium::Store;
    }

    /// Scrubs about `budget` sectors (309-STO-0025), the store's pass then the copy's; `found` hears of each record of
    /// either that does not open or is not whole. Each damaged record whose block the index names is repaired from the
    /// other medium if that one gives it back whole; a step repairs REPAIRS at most. Once a pass ends after repairs on
    /// its medium, a collection there frees the damaged records. Returns the medium and findings of a pass that ended.
    pub fn scrub<D: Device>(&mut self, main: &mut Store<D>, budget: u64, found: &mut dyn FnMut(Medium, u64, Option<Cid>), log: Log) -> Result<Option<(Medium, Scrubbed)>, (Medium, Error)> {
        let on = self.scrubbing;
        let (mut damaged, mut n) = ([None::<Cid>; REPAIRS], 0);
        let mut note = |lba: u64, cid: Option<Cid>| {
            found(on, lba, cid);
            if let Some(cid) = cid.filter(|_| n < REPAIRS) { damaged[n] = Some(cid); n += 1; }
        };
        let stepped = match on { Medium::Store => main.scrub(budget, &mut note), Medium::Copy => self.copy.scrub(budget, &mut note) };
        in_volume(main, |main| for cid in damaged[..n].iter().flatten() { self.repair(main, cid, on, &mut *log) });
        let Some(findings) = stepped.map_err(|e| (on, e))? else { return Ok(None) };
        self.scrubbing = on.other();
        if self.collect[on.at()] {
            let freed = match on { Medium::Store => main.collect(), Medium::Copy => self.copy.collect() }.map(|c| c.blocks);
            // One that could not run (a name lacks a block) is asked for again after the next pass.
            self.collect[on.at()] = freed.is_err();
            log(Event::Collected { on, freed });
        }
        Ok(Some((on, findings)))
    }

    /// A whole pass of each medium now, from its first record, in steps of `budget` sectors as `scrub` takes them: what
    /// the store's pass found, and what the copy's found or why it stopped.
    pub fn scrub_whole<D: Device>(&mut self, main: &mut Store<D>, budget: u64, found: &mut dyn FnMut(Medium, u64, Option<Cid>), log: Log) -> Result<(Scrubbed, Result<Scrubbed, Error>), Error> {
        self.restart_scrub(main);
        let mut mine = Scrubbed::default();
        loop {
            match self.scrub(main, budget.max(1), found, log) {
                Ok(Some((Medium::Store, f))) => mine = f,
                Ok(Some((Medium::Copy, f))) => return Ok((mine, Ok(f))),
                Ok(None) => {}
                Err((on, e)) => {
                    self.restart_scrub(main);
                    return if on == Medium::Store { Err(e) } else { Ok((mine, Err(e))) };
                }
            }
        }
    }
}

// What bringing the copy of `name` in step with the store's current version needs (main in the volume's domain): nothing
// (neither holds a root of it), nothing either for a name the store does not hold at all, its removal there, or its
// root's blocks and then, unless the copy is at that root, its name; with the copy's version and root of it.
enum Need { Nothing, OnlyOnCopy { version: u64, root: Cid }, Remove { held: u64 }, Copy { root: Cid, held: u64, at: Option<Cid> } }
fn need<C: Device, D: Device>(copy: &Store<C>, main: &Store<D>, name: &[u8]) -> Result<Need, Error> {
    let (version, root) = main.snapshot(name)?;
    let (held, at) = copy.snapshot(name)?;
    Ok(match (root, at) {
        (None, Some(at)) if version == 0 => Need::OnlyOnCopy { version: held, root: at },
        (None, None) => Need::Nothing,
        (None, Some(_)) => Need::Remove { held },
        (Some(root), at) => Need::Copy { root, held, at },
    })
}

// The badge the store's latest version of `name` was written by: the copy's version is charged to the same owner.
fn owner_of<D: Device>(main: &Store<D>, name: &[u8]) -> u16 { main.history(name).ok().and_then(|v| v.first()).map_or(0, |v| v.owner) }

// Tells the log what following a name did.
fn tell(done: Followed, name: &[u8], log: Log) {
    match done {
        Followed::InStep => {}
        Followed::Copied { version, blocks } => log(Event::Copied { name, version, blocks }),
        Followed::Removed { version } => log(Event::Removed { name, version }),
        Followed::OnlyOnCopy { version, root } => log(Event::OnlyOnCopy { name, version, root }),
    }
}
