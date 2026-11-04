# 309-STO-0040 — Copies of chosen names on a second medium, and a damaged block repaired from the other

**Type:** storage (`blockstore`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** for the QEMU criteria, `init` granting `blockstore` a second store disk (`requests-KRN.md`, "A second store disk for the block store's copy", 2026-11-04) · **Main task:** [309](309-copies-scrub-and-repair.md) · **Constitution:** MC-4.4, MC-4.8, MC-5.1

Split from [309](309-copies-scrub-and-repair.md) (2026-11-04), its plan's "Copies". The scrub ([309-STO-0025](../issues-done/309-STO-0025-scrub.done)) and a copy in a file ([309-STO-0031](../issues-done/309-STO-0031-a-copy-of-named-roots-in-a-file.done)) are done. A store that cannot be mounted, rebuilt from its copy, is left to a later task of 309.

Everything here is plan, not a guarantee (MC-12.3).

## Problem

- **One copy on one medium.** The store keeps every block once. The scrub finds a damaged block, but nothing repairs it unless the owner exported a copy to a file before ([309-STO-0031](../issues-done/309-STO-0031-a-copy-of-named-roots-in-a-file.done)) and imports it by hand.
- **A lost medium loses the store.**
- **`copied` is never reached.** `blockstore.wit` has had the class since 2.0 (307-STO-0019), "not reached yet" (MC-4.4, Appendix B.3).

## Plan

- **Two media, two stores.**
  - `init` gives `blockstore` a second block client, the copy's medium (the request to `KRN`, below).
  - Each medium is a whole store of its own: its own superblock and its own volume key, kept by `keystore` under its own id where `init` lends that client.
  - The first medium is the store; the second is its copy. The copy holds only the names chosen for it, and what they retain.
- **Which names have two copies.**
  - A name has a copy when the copy's medium holds it, so the choice lives where the copy does. Nothing in the store's layout changes.
  - `copy(name, on)` (`blockstore.wit` 2.5, a minor version), with the publish right:
    - on: the name's current version is copied, its blocks and then the name;
    - off: the name is removed from the copy, and the copy's next collection frees what only it retained.
  - **Only the volume's names.** A memory domain's records are sealed under the domain's key, which the copy does not hold, so a domain's name is refused (`invalid`).
- **Each change of a copied name is made on the copy too.** This covers `publish`, `unpublish` and `commit`:
  1. every block under the new root that the copy lacks is put there, and the copy flushed;
  2. the store's name record is written and flushed;
  3. the copy's name is moved to the same root by its own compare-and-swap, and flushed.

  **The reply says `copied` only after step 3.** If the copy did not take a step, the change stands at `flushed` and says so, and the name is behind on its copy (logged). A commit's copied names change in one commit on the copy.
- **The work follows the change.** Where both roots are trees, the old root on the copy and the new one are compared (`tree::diff`, 311-STO-0039), and only what differs is walked for blocks the copy lacks. Any other root is walked whole, each block checked with `has` on the copy before it is read.
- **A name behind on its copy** is copied again between requests, as the scrub runs (MC-5.1: a budget per step). At a mount, each name on the copy is compared with the store's:
  - the same root: in step;
  - another root: behind;
  - **a name the store does not hold is left as it is,** and logged. A store that came back blank never empties its copy. Only an `unpublish` or a `copy(name, off)` removes a name from the copy.
- **Repair.**
  - **A read:** a block the store finds damaged is read from the copy, checked against its CID as every read is, and put back in the store. The reply is the block, and the log says `REPAIRED … FROM THE COPY`.
  - **The scrub** scrubs both media, a step at a time. A damaged record whose block the index names is repaired from the other medium, if that medium holds it.
  - **After repairs,** a collection frees the damaged records, as an import does (309-STO-0031).
  - **A block only one medium holds** is repaired by nothing: one damaged in a name without a copy is reported, as now.
- **`copy-stat()`** (2.5): whether a copy is mounted, its medium's size and use, its blocks and names, the names behind, and the blocks repaired from each medium since the mount.
- **Independence stated** (MC-4.8):
  - two VirtIO disks are two devices to the system, but QEMU may hold both as files on one host disk, which the guest cannot tell;
  - two partitions of one disk are not independent copies ([307](307-the-stores-own-partition.md)), and the store will say so when it is given one.
- **`blocks`:**
  - `copies <name> on|off`;
  - `copy stat`;
  - `publish`, `remove` and `commit` print the class each reply gives, as now.

## The request to `KRN`

`init` is the kernel track's, so the copy's medium is a request in `requests-KRN.md`, "A second store disk for the block store's copy" (2026-11-04):
- in QEMU, the next VirtIO disk after the store's that is blank or starts with the store's superblock;
- granted to `blockstore` in slot 4, with the write badge, and never to `vfs_server`;
- logged as the store's disk is;
- on real machines it waits for [307-KRN-0177](307-KRN-0177-the-stores-own-partition-classified-and-granted.md)'s partitions.

## What it does not give

- **No rebuild yet.** A store that cannot be mounted is not rebuilt from its copy here; a later task of 309 does that, with a tool that says what was lost.
- **No pairing check.** A medium does not record which store it is the copy of. A copy from another machine's store would have its names reported as not in this store and left as they are; a name both hold would be moved to this store's root. `init` gives a copy only in QEMU until 307's partitions.
- **The copy's versions are its own.** A copied name's root is the same on both media; its version numbers may differ.
- **Pins are not copied.** A pin retains on the medium it was taken on.
- **Memory:** the copy's index, names and buffers are its own, within the store's memory quota. A copy whose index does not fit is refused, and logged.

## Acceptance criteria

- **Host** (`tests/blockstore_host.rs`, the store's logic on simulated media):
  - a copied name's blocks and name reach the copy, and the reply is `copied`; a name without a copy's is `flushed`;
  - a copy that fails a step leaves the change at `flushed` and the name behind, caught up later;
  - a block damaged in either medium repaired from the other, on a read and by the scrub; a block only one medium holds reported, not repaired;
  - a mount with a name only the copy holds leaves it there;
  - a domain's name refused;
  - the work follows the change, counted by the blocks read;
  - the random model covers copies (309's criterion).
- **QEMU** (x86, aarch64), once `init` grants the copy: the `store` suite on two disks:
  - a name copied, and its publish replying `COPIED`;
  - a block damaged on one disk by the suite repaired from the other, on a read and by a scrub;
  - the next scrub clean.
- **Docs:** docs/storage, `blockstore.wit` 2.5 and docs/idl, and the profile's Article 4 rows.

## Related

[309](309-copies-scrub-and-repair.md), [309-STO-0025](../issues-done/309-STO-0025-scrub.done), [309-STO-0031](../issues-done/309-STO-0031-a-copy-of-named-roots-in-a-file.done), [307](307-the-stores-own-partition.md), [307-STO-0019](../issues-done/307-STO-0019-how-far-a-write-reached.done), [311-STO-0039](../issues-done/311-STO-0039-two-roots-compared.done).
