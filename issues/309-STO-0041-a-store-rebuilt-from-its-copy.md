# 309-STO-0041 — A store rebuilt from its copy, and what it lost said

**Type:** storage (`blockstore`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — (built on [309-STO-0040](309-STO-0040-copies-of-chosen-names-on-a-second-medium.md), on the track's branch; it lands after or with it) · **Main task:** [309](309-copies-scrub-and-repair.md) · **Constitution:** MC-4.4, MC-4.8, MC-5.1, MC-6.8

Split from [309](309-copies-scrub-and-repair.md) (2026-11-04), its plan's "Damage beyond a block": a store that cannot be mounted, rebuilt from its copy, with a tool that checks and says what it lost. The copies themselves are [309-STO-0040](309-STO-0040-copies-of-chosen-names-on-a-second-medium.md).

Everything here is plan, not a guarantee (MC-12.3).

## Problem

- **A lost medium leaves its names on the copy only.** A store whose medium was replaced by a blank one, or came back blank, mounts empty. Its copy keeps the names that had one, at a mount logged `ONLY ON THE COPY … LEFT AS IT IS`, but nothing brings them back into the store.
- **Nothing says what was lost.** No call lists the copy's names. Whether each one is whole on the copy is found only by trying to read it.
- **A medium the store cannot mount stays as it is** (305-STO-0008). Its copy is not mounted beside it (`NO COPY: …`), so even what the copy holds cannot be read.

## Plan

- **The copy's names listed:** `copy-names(after)` (`blockstore.wit` 2.6, a minor version), with the read right.
  - It returns up to 16 of the copy's names after `after`, in byte order.
  - Each comes with the copy's version, the root, and how it stands against the store:
    - `in-step`: the same root in the store;
    - `behind`: another root in the store, or a removal there;
    - `only-on-copy`: a name the store does not hold at all.
  - It is refused through a domain's client (`rights`), as `copy` is.
- **A name brought back:** `rebuild(name, check)` (2.6), with the publish right, never through a domain. It takes only a name `only-on-copy`, at the copy's current version. Any other name is refused with `conflict`: a name the store holds is never overwritten, and a removal is never undone.
  - **The walk:** the copy's root is walked over the copy, within the walk's own steps. Each block the store lacks is read from the copy, checked against its CID, and then put in the store. With `check`, nothing is put: the reply counts the blocks the walk visits and those the store lacks.
  - **The publication:** the store's name is published at the copy's root by compare-and-swap from version 0. The store's version is charged to the owner the copy's version records.
    - The name is then in step with its copy. It keeps its copy, writes nothing more there, and the reply is `copied`.
  - **A name the copy holds incomplete:** a walk that meets a block the copy lacks, or holds damaged, stops. The reply is `incomplete` or `corrupt`, with the CID it stopped at, and nothing is published. The blocks already put are not retained, so the next collection frees them. The name stays on the copy as it is.
  - **The reply:** the name's version in the store, the blocks visited and put, how far the write reached, or the error and the CID.
- **The log:** each name rebuilt is logged `REBUILT <name> VERSION <v> ROOT <cid> FROM THE COPY, <n> BLOCKS PUT`, and each refused `NOT REBUILT <name>: <error> AT <cid>`.
- **The tool:** `blocks rebuild [--check]`.
  - It lists the copy's names with `copy-names`. For each `only-on-copy` name it calls `rebuild(name, true)`, then, without `--check`, `rebuild(name, false)` for each name found whole.
  - It prints a line per name:
    - `WHOLE <name> VERSION <v> ROOT <cid>: <b> BLOCKS, <m> TO PUT`;
    - `REBUILT <name> VERSION <v> ROOT <cid> (<class>): <m> BLOCKS PUT`;
    - `LOST <name> VERSION <v> ROOT <cid>: <error> AT <cid>`;
    - `IN STEP <name>`;
    - `BEHIND <name>`.
  - It ends with `CHECKED` or `REBUILT <n> NAMES ON THE COPY: <w> WHOLE, <l> LOST, <s> IN STEP, <b> BEHIND`.
- **A medium the store cannot mount:** the copy is mounted beside it, read-only, so that `copy-names` and `rebuild(name, true)` say what the copy would bring back. `rebuild(name, false)` waits for a store that mounts: it is refused with `device` while the store is not mounted. The store still never formats a medium it cannot mount (305-STO-0008). Making it blank is a person's act: on QEMU, a blank disk put in its place.

## What it does not give

- **What the copy never held:**
  - a name that had no copy;
  - the store's history, older than each name's current version on the copy;
  - pins;
  - a change the copy was behind on when the store was lost.

  The tool cannot tell these from what never existed, and it says so in its output.
- **No older version of the copy** is tried when its current one is incomplete.
- **No erasing of a medium the store cannot mount.**
- **No pairing check** ([309-STO-0040](309-STO-0040-copies-of-chosen-names-on-a-second-medium.md)): a copy from another store would be rebuilt into this one.

## Acceptance criteria

- **Host** (`tests/blockstore_host.rs`):
  - a store whose medium is replaced by a blank one rebuilt from its copy, names, roots and every block, found again after a remount of both;
  - `check` putting nothing, and counting what the rebuild then puts;
  - a name the copy holds incomplete or damaged reported with the CID and not published, and its blocks freed by the next collection;
  - a name in step, behind, or held by the store refused with `conflict`;
  - a domain's client refused;
  - a medium that cannot be mounted: the copy's names listed, `check` answered, and the rebuild refused with `device`;
  - `copy-names` paging over more than 16 names;
  - the random model of both media covering rebuilds after a store's medium was lost.
- **QEMU** (x86, aarch64): the `store` suite, after 309-STO-0040's copy check.
  - Two names are copied, and a chunk of one is damaged on the copy.
  - The store's disk is replaced by a blank image, and the machine boots.
  - `blocks rebuild --check` reports one name whole and one lost; `blocks rebuild` brings the whole one back, read by its pattern.
  - After a reboot, both disks are found as the store's and the name is still there.
- **Docs:** docs/storage, `blockstore.wit` 2.6 and docs/idl, and the profile's Article 4 rows.

## Related

[309](309-copies-scrub-and-repair.md), [309-STO-0040](309-STO-0040-copies-of-chosen-names-on-a-second-medium.md), [309-STO-0031](../issues-done/309-STO-0031-a-copy-of-named-roots-in-a-file.done), [305](../issues-done/305-recovery-without-the-store.done), [307](307-the-stores-own-partition.md).
