# 300-STO-0004 — Rights to the block store by badge: storing apart from reading

**Type:** service (storage) · **Owner:** `STO` track · **Priority:** P2 · **Status:** in progress · **Blocked by:** — ([300-KRN-0024](../issues-done/300-KRN-0024-read-only-blockstore-client.done): a program that asks only to read, `REQUEST_BLOCKSTORE_READ`, holds a client badged get alone; done) · **Roadmap:** track B · **Constitution:** MC-4.7, MC-4.11, MC-3.3, Appendix B.6

Part of main task [300](300-checksummed-block-store.md).

## Problem

Any holder of a `blockstore` client could put and get. MC-4.7 says that a hash does not permit reading, so knowing a CID must not be enough to read a block. MC-4.11 separates the right of access from the obligation to retain. A stored block is kept, because there is no deletion yet, so the right to store is a different right from the right to read.

## Plan

- **The rule** in `libmind/src/blockstore.rs`, with no system calls so the host tests build it:
  - `BADGE_GET` allows `get` and `has`;
  - `BADGE_PUT` allows `put`;
  - either allows `stat`;
  - a client without either may do nothing, and bits this version does not know grant nothing.
- **The service** checks every request against the badge of the caller's capability. A refusal is answered `rights` and logged with the caller's PID and badge.
- `idl/blockstore.wit` 1.0: `stat` returns `result<stats, error>`, so it can be refused. The interface has not run anywhere yet, so this is still its first version.
- **The kernel track** mints the clients with these badges (added to [300-KRN-0001](../issues-done/300-KRN-0001-blockstore-at-boot.done) (done)).

## Acceptance criteria

- The rule is host-tested for every badge, unknown bits included.
- The service refuses and logs every request its badge does not allow.
- **Blocked:** in the QEMU suite, a client with only `BADGE_GET` is refused a put, and one with only `BADGE_PUT` is refused a get.

## Progress (2026-10-06)

- **Done:**
  - the rule (`mind::blockstore`: `BADGE_GET`, `BADGE_PUT`, `allowed`);
  - the service's checks and log line `[BLOCKSTORE] REFUSED <OPERATION> FOR PID <pid> (BADGE <badge>)`;
  - `stat` as a result;
  - the host test `rights_come_from_the_badge`.
- **Waiting:** clients with badges (the kernel track), then the QEMU case.

## Related

[docs/storage](../docs/storage/README.md); `libmind/src/gpio.rs` (the same pattern for pins); Constitution Article 4.

## Progress (2026-10-07)

- On the platform the shell's client carries badge 7 (get, put, publish), and the `store` suite uses it through `blocks` ([300-STO-0003](../issues-done/300-STO-0003-blocks-tool-and-store-suite.done)).
- A refusal can only be seen with a client that lacks a right. The shell cannot make one, because a badge is set once and kept by children. A request to the kernel track asks for one.
