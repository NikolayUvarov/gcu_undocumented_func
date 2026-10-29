//! Rights of block store clients (issues 300-STO-0004, 302-STO-0001, 303-STO-0001..0004, 304-STO-0007, 300-STO-0018; Appendix B.6, MC-4.3, MC-4.7, MC-4.11): init
//! mints each client's capability with a badge, and `blockstore` decides every request by it. No system calls:
//! tests/blockstore_host.rs. Badges from `FIRST_DOMAIN_CLIENT` up are the clients `blockstore` mints for memory domains
//! (308-STO-0023), each with the rights of the client that asked for it.

/// May read blocks and names (`get`, `has`, `resolve`, `history`, `snapshot`, `names`). Knowing a CID grants nothing: reading needs this right (MC-4.7).
pub const BADGE_GET: u16 = 1;
/// May store blocks (`put`) and start a collection (`collect`); storing is kept apart from reading (MC-4.11).
pub const BADGE_PUT: u16 = 2;
/// May retain objects: make names point at new roots or remove them (`publish`, `unpublish`, `commit`, MC-4.3: an
/// authorized publication) and pin or unpin objects (MC-4.11). Resolving a name or reading its history is reading.
pub const BADGE_PUBLISH: u16 = 4;
/// The first badge of a domain's client; below it, the badges `init` and the shell give.
pub const FIRST_DOMAIN_CLIENT: u16 = 0x100;

/// A request to the store, as its rights see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation { Put, Get, Has, Stat, Publish, Resolve, Collect, Unpublish, History, Pin, Unpin, Pins, Usage, Commit, Snapshot, Names, DomainCreate, DomainClient, DomainForget, Domains, Scrub, Scrubbed }

/// Whether a client whose capability carries `badge` may make `operation`; bits of later rights are ignored.
pub fn allowed(badge: u16, operation: Operation) -> bool {
    match operation {
        // A collection frees only what nothing retains, as a put that finds no room does. A whole scrub on request reads
        // the medium on the caller's behalf, as a collection does (309-STO-0025).
        Operation::Put | Operation::Collect | Operation::Scrub => badge & BADGE_PUT != 0,
        Operation::Get | Operation::Has | Operation::Resolve | Operation::History | Operation::Snapshot | Operation::Names => badge & BADGE_GET != 0,
        // Names and pins retain objects: retention is its own right, apart from reading and storing (MC-4.11).
        // Making and forgetting a memory domain is retention too (308-STO-0023).
        Operation::Publish | Operation::Unpublish | Operation::Commit | Operation::Pin | Operation::Unpin
            | Operation::DomainCreate | Operation::DomainForget => badge & BADGE_PUBLISH != 0,
        // A domain's client carries the rights of the client that asked for it.
        Operation::Stat | Operation::Pins | Operation::Usage | Operation::DomainClient | Operation::Domains | Operation::Scrubbed => badge & (BADGE_GET | BADGE_PUT | BADGE_PUBLISH) != 0,
    }
}

/// A durability class (its number in idl/blockstore.wit's `durability`) as the log and the tools write it (307-STO-0019).
pub fn class(durability: u8) -> &'static str { ["VOLATILE", "FLUSHED", "COPIED"].get(durability as usize).copied().unwrap_or("?") }
