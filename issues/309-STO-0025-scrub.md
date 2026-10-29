# 309-STO-0025 — Scrub: every record read and checked within a budget

**Type:** storage (`blockstore`, `idl/blockstore.wit`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [309](309-copies-scrub-and-repair.md) · **Constitution:** MC-4.8, MC-5.1, MC-12.4

Split from [309](309-copies-scrub-and-repair.md) (2026-10-29). The scrub needs no second medium, so it does not wait for [307](307-the-stores-own-partition.md).

## Problem

A damaged record is found only when it is read, or when the store mounts (300-STO-0005). A block nobody reads can stay damaged for as long as the store runs, and nothing says so until the one copy is needed. MC-4.8 asks the storage policy to define corruption detection; MC-5.1 asks that the I/O and processor time spent on it have a budget.

## Plan

- **The scrub** reads the records from the medium's start to its end and checks each one as a mount does, without changing anything:
  - each record opened under its domain's key;
  - a block's bytes matched to its CID;
  - a name, pin, commit or key record checked by its digest;
  - a forgotten domain's records not opened, since its key is gone;
  - non-blank sectors outside every record counted damaged.
- **Writes during a pass.** A record written where the scrub has not yet reached is checked when it gets there. One written around the scrub's place is skipped: its writer has just checked it. A collection only makes sectors blank.
- **The budget (MC-5.1):**
  - idle passes run only between requests, a step of at most 256 sectors at a time;
  - a pass starts a day after the last one ended (the mount counts as one);
  - a whole pass on request is made on behalf of a client with the right to collect, and that client waits for it.
- **Results:**
  - each record found corrupt is logged with its sector, and with its CID when the index holds a block there;
  - each whole pass logs what it found;
  - `scrubbed` returns the last whole pass's findings.
- **Interface:** `blockstore.wit` 2.3, a minor version: `scrub` and `scrubbed`, and the record `scrubbed`. `blocks scrub` asks for a pass, `blocks scrubbed` shows the last one.
- **Not here:** repair. One copy cannot be repaired from itself: [309](309-copies-scrub-and-repair.md)'s copies do that.

## Acceptance criteria

- Host tests:
  - a pass in small steps finds what a whole pass finds;
  - a damaged block, name record, key record and sector are found, and nothing is written;
  - a forgotten domain's records are not counted corrupt;
  - a record written around the scrub's place during a pass is not counted damaged.
- QEMU `storefaults` (x86 and aarch64): a byte changed in a block's record on the medium is found by `blocks scrub`, logged with its CID, and reported by `blocks scrubbed`.

## Progress (2026-10-29)

- **Built** on the storage branch (86f68750), on top of 308-STO-0023's layout 4, so it lands with it, after [308-KRN-0184](308-KRN-0184-blockstore-badges-its-own-clients.md).
- **A step** ends at the end of the record or the 33-sector window it is in, so a step covers about 256 sectors rather than at most 256.
- **A whole pass on request** keeps every other client waiting too, as a collection does: requests are served one at a time.
- **Tested:**
  - host tests, all four criteria; the test of a record written around the scrub reads 2 damaged sectors without the move;
  - QEMU `store`, `storefaults`, `boot` and `vfs` on x86, in this session's container;
  - QEMU `store`, `storefaults` and `vfs` on aarch64, on `i9_4e6882212251` through Effector (08155086).
- **Not tested on the platform:** the scrub between requests, whose first pass starts a day after the mount.

## Related

[300-STO-0005](../issues-done/300-STO-0005-corruption-on-the-platform.done), [309](309-copies-scrub-and-repair.md), [docs/storage](../docs/storage/README.md).
