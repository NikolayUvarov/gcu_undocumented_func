// The derivation tree's index (000-KRN-0104; MC-3.5, 3.6, 5.4): each node that has a parent or a child here, with its
// parent, depth, place, children and the tasks that mapped memory through it, so that a revocation visits the nodes it
// removes and no others, in steps of REVOKE_STEP nodes with the lock let go between them. Kept where nodes are made,
// moved and dropped; where it is read, a node's place is checked against the task that should hold it.
use super::*;

// Where a node is: a task's slot, a copy in a task's send, or nowhere but here (a ghost: removed, still with descendants).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum At { Slot { task: usize, pid: u64, index: usize }, Send { task: usize, pid: u64 }, Ghost }

// A node's entry: its children are a list through `first`, `next` and `prev` (ids, 0 for none), newest first; `maps`
// holds two tasks that mapped through it, and `more_maps` says more did. Id 0 marks a free place.
#[derive(Clone, Copy)]
pub(super) struct Entry { id: u64, parent: u64, depth: usize, at: At, first: u64, next: u64, prev: u64, maps: [(usize, u64); 2], more_maps: bool }
impl Entry { const FREE: Self = Self { id: 0, parent: 0, depth: 0, at: At::Ghost, first: 0, next: 0, prev: 0, maps: [(0, 0); 2], more_maps: false }; }

// The entries by id, in open addressing. Its size is a power of two; it grows when full and goes back to the size its
// live entries alone ask for at the reaper's pass, so the kernel heap it takes then depends on what is held, not on
// what was.
pub(super) struct Tree { places: Vec<Entry>, live: usize, sweep: usize }
const PLACES_MIN: usize = 256;
const SWEEP: usize = 256; // places the reaper looks at in one pass

impl Tree {
    pub(super) fn new() -> Self { Self { places: alloc::vec![Entry::FREE; PLACES_MIN], live: 0, sweep: 0 } }
    fn size_for(live: usize) -> usize { let mut size = PLACES_MIN; while live * 10 > size * 7 { size *= 2; } size }
    fn home(&self, id: u64) -> usize { (id.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - self.places.len().trailing_zeros())) as usize }
    fn find(&self, id: u64) -> Option<usize> {
        if id == 0 { return None; }
        let (mask, mut at) = (self.places.len() - 1, self.home(id));
        loop {
            match self.places[at].id { 0 => return None, found if found == id => return Some(at), _ => at = (at + 1) & mask }
        }
    }
    pub(super) fn get(&self, id: u64) -> Option<Entry> { self.find(id).map(|at| self.places[at]) }
    fn get_mut(&mut self, id: u64) -> Option<&mut Entry> { self.find(id).map(move |at| &mut self.places[at]) }
    // Room for `more` entries, growing if need be; false when the kernel heap has none.
    pub(super) fn reserve(&mut self, more: usize) -> bool { let size = Self::size_for(self.live + more); size <= self.places.len() || self.resize(size) }
    fn insert(&mut self, entry: Entry) {
        let (mask, mut at) = (self.places.len() - 1, self.home(entry.id));
        while self.places[at].id != 0 { at = (at + 1) & mask; }
        self.places[at] = entry; self.live += 1;
    }
    fn remove(&mut self, id: u64) {
        let Some(mut hole) = self.find(id) else { return };
        self.places[hole] = Entry::FREE; self.live -= 1;
        // The entries after it that probed past the hole move up, so that no lookup stops short of them.
        let mask = self.places.len() - 1;
        let mut at = (hole + 1) & mask;
        while self.places[at].id != 0 {
            let home = self.home(self.places[at].id);
            if at.wrapping_sub(home) & mask >= at.wrapping_sub(hole) & mask { self.places[hole] = self.places[at]; self.places[at] = Entry::FREE; hole = at; }
            at = (at + 1) & mask;
        }
    }
    fn resize(&mut self, size: usize) -> bool {
        let mut places = Vec::new();
        if places.try_reserve_exact(size).is_err() { return false; }
        places.resize(size, Entry::FREE);
        let old = core::mem::replace(&mut self.places, places);
        self.live = 0;
        for entry in old.into_iter().filter(|e| e.id != 0) { self.insert(entry); }
        true
    }
}

// A revocation in steps: its caller, its root, the nodes whose children are still to remove (each a child of the one
// below it), what it removed, whether another CPU must leave an affected address space first, and its steps and the
// longest of them, which its caller gets.
pub(super) struct Revocation { slot: usize, pid: u64, root: u64, stack: Vec<u64>, removed: usize, wait: bool, steps: usize, longest_ns: u64 }

pub(super) const REVOKE_STEP: usize = 256; // nodes one step of a revocation visits under the lock

impl Scheduler {
    // Whether node `id` is still where `at` says.
    fn holds_at(&self, id: u64, at: At) -> bool {
        let task = |slot: usize, pid: u64| (slot < self.tasks.len()).then(|| self.tasks[slot].as_ref()).flatten().filter(|t| t.pid == pid);
        match at {
            At::Slot { task: slot, pid, index } => task(slot, pid).is_some_and(|t| index < t.cspace.len() && t.cspace[index].is_some() && t.nodes[index].id == id),
            At::Send { task: slot, pid } => task(slot, pid).is_some_and(|t| t.pending_cap.is_some_and(|p| p.moved_from.is_none() && p.node.id == id)),
            At::Ghost => true,
        }
    }
    fn mapped_by(&self, slot: usize, pid: u64, id: u64) -> bool {
        pid != 0 && slot < self.tasks.len() && self.tasks[slot].as_ref().is_some_and(|t| t.pid == pid && t.heap.made_from(id))
    }
    // Whether anything still derives from an entry's node: a child, or a mapping made through it.
    fn derived(&self, entry: &Entry) -> bool {
        entry.first != 0 || entry.maps.iter().any(|&(slot, pid)| self.mapped_by(slot, pid, entry.id))
            || (entry.more_maps && self.tasks.iter().flatten().any(|t| t.heap.made_from(entry.id)))
    }

    // A child put at the head of its parent's list, and taken out of it.
    fn link(&mut self, parent: u64, id: u64) {
        let first = self.tree.get(parent).map_or(0, |p| p.first);
        if let Some(entry) = self.tree.get_mut(id) { entry.next = first; entry.prev = 0; }
        if let Some(old) = self.tree.get_mut(first) { old.prev = id; }
        if let Some(entry) = self.tree.get_mut(parent) { entry.first = id; }
    }
    fn unlink(&mut self, id: u64) {
        let Some(Entry { parent, next, prev, .. }) = self.tree.get(id) else { return };
        if prev != 0 { if let Some(before) = self.tree.get_mut(prev) { before.next = next; } }
        else if let Some(entry) = self.tree.get_mut(parent).filter(|p| p.first == id) { entry.first = next; }
        if let Some(after) = self.tree.get_mut(next) { after.prev = prev; }
        if let Some(entry) = self.tree.get_mut(id) { entry.next = 0; entry.prev = 0; }
    }

    // Room for the entries of `copies` new children and their sources; false when the kernel heap has none.
    pub(super) fn tree_room(&mut self, copies: usize) -> bool { self.tree.reserve(2 * copies) }
    // Node `node` entered at `at` under its parent's entry (after tree_room); moved, only its place changes.
    pub(super) fn enter(&mut self, node: Node, at: At) {
        if node.id == 0 { return; }
        if let Some(entry) = self.tree.get_mut(node.id) { entry.at = at; return; }
        if !self.tree.reserve(1) { return; }
        let depth = if node.parent == 0 { 0 } else { self.tree.get(node.parent).map_or(1, |p| p.depth + 1) };
        self.tree.insert(Entry { id: node.id, parent: node.parent, depth, at, ..Entry::FREE });
        if node.parent != 0 { self.link(node.parent, node.id); }
    }
    // A copy's source, entered where it is if it had no entry yet (a root's first child).
    pub(super) fn enter_source(&mut self, node: Node, at: At) { if self.tree.get(node.id).is_none() { self.enter(node, at); } }
    // A moved node's new place, if it has an entry.
    pub(super) fn moved(&mut self, id: u64, at: At) { if let Some(entry) = self.tree.get_mut(id) { entry.at = at; } }
    // A task that mapped memory through a child node, kept so that revoking the node unmaps it.
    pub(super) fn mapped(&mut self, node: Node, at: At, slot: usize, pid: u64) {
        if node.parent == 0 { return; }
        self.enter_source(node, at);
        let Some(entry) = self.tree.get_mut(node.id) else { return };
        if entry.maps.contains(&(slot, pid)) { return; }
        match entry.maps.iter().position(|&(_, p)| p == 0) { Some(free) => entry.maps[free] = (slot, pid), None => entry.more_maps = true }
    }

    // A node gone from its place for good: a ghost while anything derives from it, else forgotten, and then its parent
    // if that kept its entry only for its children.
    pub(super) fn forget(&mut self, id: u64) {
        let Some(entry) = self.tree.get(id) else { return };
        if self.derived(&entry) { self.tree.get_mut(id).unwrap().at = At::Ghost; return; }
        self.unlink(id); self.tree.remove(id);
        self.let_go_of(entry.parent);
    }
    // A root's entry, or a ghost, with nothing left under it.
    fn let_go_of(&mut self, id: u64) {
        let Some(entry) = self.tree.get(id) else { return };
        if self.derived(&entry) { return; }
        if entry.at == At::Ghost { self.forget(id); } else if entry.parent == 0 { self.tree.remove(id); }
    }
    // A copy in a send that will not arrive.
    pub(super) fn forget_send(&mut self, pending: Option<Pending>) {
        if let Some(p) = pending.filter(|p| p.moved_from.is_none()) { self.forget(p.node.id); }
    }

    // Whether a copy, mint or transfer of node `id` is refused: a revocation in progress covers it (ERR_REVOKING), or a
    // copy would be more than DERIVE_DEPTH_MAX derivations from its root (ERR_LIMIT).
    pub(super) fn derive_refused(&self, id: u64, copy: bool) -> Option<usize> {
        let depth = self.tree.get(id).map_or(0, |e| e.depth);
        if !self.revoking.is_empty() {
            let mut at = id;
            for _ in 0..=DERIVE_DEPTH_MAX {
                if self.revoking.contains(&at) { return Some(ERR_REVOKING); }
                match self.tree.get(at) { Some(e) if e.parent != 0 => at = e.parent, _ => break }
            }
        }
        (copy && depth + 1 > DERIVE_DEPTH_MAX).then_some(ERR_LIMIT)
    }

    // CAP_REVOKE of node `root` held in `slot`: its first step now, and whether that was the last; from here on a copy,
    // mint or transfer of anything under the root is refused until the last step.
    pub(super) unsafe fn begin_revoke(&mut self, slot: usize, root: u64, cpu: usize) -> (Revocation, bool) {
        let pid = self.tasks[slot].as_ref().unwrap().pid;
        self.revoking.push(root);
        let mut job = Revocation { slot, pid, root, stack: alloc::vec![root], removed: 0, wait: false, steps: 0, longest_ns: 0 };
        let done = self.revoke_step(&mut job, cpu);
        (job, done)
    }

    // One step: at most REVOKE_STEP nodes, depth first, newest child first; true once nothing is left under the root.
    pub(super) unsafe fn revoke_step(&mut self, job: &mut Revocation, cpu: usize) -> bool {
        let started = crate::clock::now_ns();
        let mut budget = REVOKE_STEP;
        while budget > 0 {
            let Some(&top) = job.stack.last() else { break };
            let kid = self.tree.get(top).map_or(0, |e| e.first);
            if kid == 0 {
                // All under `top` removed: its entry goes (the root's only if it kept it for its children).
                job.stack.pop();
                if top == job.root { self.let_go_of(top); } else { self.tree.remove(top); }
                continue;
            }
            budget -= 1;
            self.unlink(kid);
            let entry = self.tree.get(kid).unwrap();
            if self.holds_at(kid, entry.at) {
                match entry.at {
                    At::Slot { task, index, .. } => { Self::clear(self.tasks[task].as_mut().unwrap(), index); job.removed += 1; }
                    At::Send { task, .. } => { self.tasks[task].as_mut().unwrap().pending_cap = None; job.removed += 1; }
                    At::Ghost => {}
                }
            }
            for (slot, pid) in entry.maps { if pid != 0 { self.unmap_revoked(slot, pid, kid, cpu, &mut job.wait); } }
            if entry.more_maps {
                for slot in 1..self.tasks.len() {
                    if let Some(pid) = self.tasks[slot].as_ref().filter(|t| t.heap.made_from(kid)).map(|t| t.pid) { self.unmap_revoked(slot, pid, kid, cpu, &mut job.wait); }
                }
            }
            let gone = self.tree.get_mut(kid).unwrap(); gone.at = At::Ghost; gone.maps = [(0, 0); 2]; gone.more_maps = false; // its children next
            job.stack.push(kid);
        }
        job.steps += 1; job.longest_ns = job.longest_ns.max(crate::clock::now_ns().saturating_sub(started));
        job.stack.is_empty()
    }

    // A mapping made through a revoked node goes; a space live on another CPU keeps its page tables until that CPU
    // switched away, and the revocation waits for it (MC-3.6). A lease's place becomes the borrower's own page (000-KRN-0152).
    unsafe fn unmap_revoked(&mut self, slot: usize, pid: u64, id: u64, cpu: usize, wait: &mut bool) {
        let current = self.current;
        let Some(task) = (slot < self.tasks.len()).then(|| self.tasks[slot].as_mut()).flatten().filter(|t| t.pid == pid) else { return };
        let remote = task.cpu != cpu && current[task.cpu] == slot;
        let (changed, made) = task.heap.revoke(&mut task.space, &[id], !remote, task.state != State::Exited);
        if changed && remote { self.flush[task.cpu] = true; *wait = true; cpu::wake(task.cpu); }
        if made > 0 { self.walk_payers(slot, |task| task.memory_tree += made * HEAP_PAGE_SIZE); }
    }

    // The end of a revocation: its root no longer refuses copies, and its caller gets how many capabilities went, the
    // steps and the longest, at once or, while another CPU may still use a removed mapping, once that CPU switched away.
    // The frame to resume.
    pub(super) unsafe fn end_revoke(&mut self, job: Revocation, sp: usize, cpu: usize) -> usize {
        if let Some(at) = self.revoking.iter().position(|&r| r == job.root) { self.revoking.swap_remove(at); }
        if self.revoking.is_empty() { self.revoking = Vec::new(); } // an empty list holds no heap memory
        let Revocation { slot, pid, removed, wait, steps, longest_ns, .. } = job;
        if !self.tasks[slot].as_ref().is_some_and(|t| t.pid == pid && t.state != State::Exited) { return self.select(sp, cpu); }
        let mailbox = self.mailbox(slot);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*mailbox).arg2), steps);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*mailbox).msg[0]), longest_ns as usize);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*mailbox).result), removed);
        // A CPU that switched address space between two steps already left the removed mappings: only a mark still set
        // is waited for (select wakes the waiters when the last goes).
        if wait && self.flush.iter().any(|&f| f) { self.tasks[slot].as_mut().unwrap().state = State::BlockedFlush; return self.select(sp, cpu); }
        sp
    }

    // The reaper's pass: ghosts nothing derives from any more (a mapping made through one ends unseen) are forgotten, a
    // few places at a time, and the index goes back to the size its live entries ask for.
    pub(super) fn sweep_tree(&mut self) {
        let size = self.tree.places.len();
        let (from, to) = (self.tree.sweep.min(size), (self.tree.sweep + SWEEP).min(size));
        let ghosts: Vec<u64> = self.tree.places[from..to].iter().filter(|e| e.id != 0 && e.at == At::Ghost).map(|e| e.id).collect();
        self.tree.sweep = if to == size { 0 } else { to };
        for id in ghosts { self.let_go_of(id); }
        if self.revoking.is_empty() { let want = Tree::size_for(self.tree.live); if want < self.tree.places.len() { self.tree.resize(want); } }
    }
}
