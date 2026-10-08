//! Rights of block store clients (issues 300-STO-0004, 302-STO-0001, 303-STO-0001..0004; Appendix B.6, MC-4.3, MC-4.7, MC-4.11): init
//! mints each client's capability with a badge, and `blockstore` decides every request by it. No system calls:
//! tests/blockstore_host.rs.

/// May read blocks and names (`get`, `has`, `resolve`). Knowing a CID grants nothing: reading needs this right (MC-4.7).
pub const BADGE_GET: u16 = 1;
/// May store blocks (`put`). A stored block is kept (there is no deletion yet), so this right is kept apart from
/// reading (MC-4.11).
pub const BADGE_PUT: u16 = 2;
/// May retain objects: make a name point at a new root or remove it (`publish`, `unpublish`, MC-4.3: an authorized
/// publication) and pin or unpin objects (MC-4.11). Resolving a name or reading its history is reading.
pub const BADGE_PUBLISH: u16 = 4;

/// A request to the store, as its rights see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation { Put, Get, Has, Stat, Publish, Resolve, Collect, Unpublish, History, Pin, Unpin, Pins, Usage }

/// Whether a client whose capability carries `badge` may make `operation`; bits of later rights are ignored.
pub fn allowed(badge: u16, operation: Operation) -> bool {
    match operation {
        // A collection frees only what nothing retains, as a put that finds no room does.
        Operation::Put | Operation::Collect => badge & BADGE_PUT != 0,
        Operation::Get | Operation::Has | Operation::Resolve | Operation::History => badge & BADGE_GET != 0,
        // Names and pins retain objects: retention is its own right, apart from reading and storing (MC-4.11).
        Operation::Publish | Operation::Unpublish | Operation::Pin | Operation::Unpin => badge & BADGE_PUBLISH != 0,
        Operation::Stat | Operation::Pins | Operation::Usage => badge & (BADGE_GET | BADGE_PUT | BADGE_PUBLISH) != 0,
    }
}
