//! The badges of `nnd`'s clients (262-APP-0112, docs/nn/runtime.md section 1): each start of a program that asks for
//! `nnd` (REQUEST_NN) gets a copy of the shell's client badged for that start alone, the lowest free badge first. The
//! shell keeps the copy; once the program has ended it revokes the copy (every copy made from it goes too), asks `nnd`
//! to retire the badge, and frees the badge only when `nnd` has answered. No system calls: tests/shell_host.rs.

/// How many programs may hold `nnd`'s client at once: the shell keeps a capability for each.
pub const CLIENTS: usize = 32;

/// The shell's own error for a start refused because every badge is held or still owed a retirement; it never crosses
/// an endpoint.
pub const NO_BADGE: usize = usize::MAX - 0x1000;

/// Where a badge is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    /// Taken for a start that has not committed yet.
    Starting,
    /// `pid` holds it; `copy` is the shell's handle of the minted client, which it revokes at the end.
    Lent { pid: u64, copy: usize },
    /// Its program ended, or its start failed: `nnd` has not retired it yet.
    Owed,
}

/// `ready`: `nnd` has answered the shell's `retire-all`, so no badge an earlier shell lent holds anything there; no badge
/// is lent before.
pub struct Badges { held: [Option<Held>; CLIENTS], pub ready: bool }

impl Default for Badges { fn default() -> Self { Self::new() } }

impl Badges {
    pub const fn new() -> Self { Self { held: [None; CLIENTS], ready: false } }

    fn at(&mut self, badge: u16) -> Option<&mut Option<Held>> { (badge as usize).checked_sub(1).and_then(|i| self.held.get_mut(i)) }

    /// The lowest free badge (1 to `CLIENTS`), taken for a start; None before `ready`, or when every one is held or owed.
    pub fn take(&mut self) -> Option<u16> {
        if !self.ready { return None; }
        let i = self.held.iter().position(Option::is_none)?;
        self.held[i] = Some(Held::Starting);
        Some(i as u16 + 1)
    }

    /// The start committed: `pid` holds the client the shell keeps in `copy`.
    pub fn lent(&mut self, badge: u16, pid: u64, copy: usize) { if let Some(h) = self.at(badge) { *h = Some(Held::Lent { pid, copy }); } }

    /// The start failed: the badge waits for its retirement, as anything may have reached `nnd` under it.
    pub fn owe(&mut self, badge: u16) { if let Some(h) = self.at(badge) { *h = Some(Held::Owed); } }

    /// Each lent badge whose program `alive` says has ended goes to `ended` with the copy to revoke, and is owed from now.
    pub fn ended(&mut self, alive: &mut dyn FnMut(u64) -> bool, ended: &mut dyn FnMut(u16, u64, usize)) {
        for (i, h) in self.held.iter_mut().enumerate() {
            if let Some(Held::Lent { pid, copy }) = *h {
                if !alive(pid) { *h = Some(Held::Owed); ended(i as u16 + 1, pid, copy); }
            }
        }
    }

    /// The badges owed a retirement, lowest first.
    pub fn owed(&self) -> impl Iterator<Item = u16> + '_ {
        self.held.iter().enumerate().filter(|(_, h)| **h == Some(Held::Owed)).map(|(i, _)| i as u16 + 1)
    }

    /// `nnd` has retired `badge`: the next start may take it.
    pub fn retired(&mut self, badge: u16) { if let Some(h) = self.at(badge) { if *h == Some(Held::Owed) { *h = None; } } }

    /// Where `badge` is.
    pub fn state(&self, badge: u16) -> Option<Held> { (badge as usize).checked_sub(1).and_then(|i| self.held.get(i)).copied().flatten() }
}
