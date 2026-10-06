//! Rights of block store clients (issue 300-STO-0004; Appendix B.6, MC-4.7, MC-4.11): init mints each client's
//! capability with a badge, and `blockstore` decides every request by it. No system calls: tests/blockstore_host.rs.

/// May read blocks (`get`, `has`). Knowing a CID grants nothing: reading needs this right (MC-4.7).
pub const BADGE_GET: u16 = 1;
/// May store blocks (`put`). A stored block is kept (there is no deletion yet), so this right is kept apart from
/// reading (MC-4.11).
pub const BADGE_PUT: u16 = 2;

/// A request to the store, as its rights see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation { Put, Get, Has, Stat }

/// Whether a client whose capability carries `badge` may make `operation`; bits of later rights are ignored.
pub fn allowed(badge: u16, operation: Operation) -> bool {
    match operation {
        Operation::Put => badge & BADGE_PUT != 0,
        Operation::Get | Operation::Has => badge & BADGE_GET != 0,
        Operation::Stat => badge & (BADGE_GET | BADGE_PUT) != 0,
    }
}
