# 311-STO-0032 — `storefs`: files and directories of a named root, through `vfs.wit`

**Type:** storage (a new service `storefs`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** how a files client reaches it and the error it reports a conflict with ([requests-KRN.md](requests-KRN.md), "`storefs`", 2026-10-30) · **Main task:** [311](311-storefs-a-file-face.md) · **Constitution:** MC-4.1, MC-4.7, MC-4.9, MC-11.10, MC-12.4

Split from [311](311-storefs-a-file-face.md) (2026-10-30), its third step, after [311-STO-0029](311-STO-0029-a-working-tree-published-by-compare-and-swap.md).

## Problem

A tree in the store is read and changed only through `blocks tree`, one command a publication. Programs read and write files through `vfs.wit`, which only `vfs_server` serves, over FAT. A program cannot keep its data in the store without being written for it.

## Plan

- **`storefs`, a service of the storage track,** serves `vfs.wit` over the store's names `fs/<volume>`, each a tree of `mind::tree`:
  - `root(<volume>)` opens the tree at the name's current version;
  - reads go to the store, every block checked against its CID;
  - writes go into a working tree (`mind::worktree`). A file open for writing is held until flushed, at most a bound named in the docs, and stored as an object of `dag` at `flush` or `close`;
  - **`flush` of a file and its `close` publish** the tree by the name's compare-and-swap, as programs written for files expect. A `flush` of a directory handle publishes every change under it at once (the storage session's choice, 2026-10-27: an explicit commit);
  - **a conflict** (another writer published since this tree was read) is reported to the writer and nothing is published. Nothing is merged. The writer may read the name again;
  - **rights:** a volume is opened read-only or writable by the client's badge, and a handle never allows more than the one it was opened from, as `vfs_server` does (MC-4.7).
- **What other tracks need to agree** (requests, not numbered yet):
  - **`KRN`:**
    - `init` starts `storefs` after `blockstore`, with a client of the store that may publish;
    - how a files client reaches it. Either `vfs_server` forwards the volumes it does not hold to `storefs`, or `mind::fs` picks the endpoint by volume and a client of `storefs` is lent beside `vfs_server`'s;
    - an error `conflict` in `vfs.wit` (a minor version), so a writer can tell a conflict from a failure.
  - **`APP`:** the shell's file commands on a `storefs` volume, once the way to reach it is agreed.
- **What does not change:** FAT stays for the boot volume and for exchange with other systems (MC-11.10).

## Acceptance criteria

- Host tests: `vfs.wit`'s semantics over a store on a simulated medium:
  - open, read, write, truncate, list, remove, rename;
  - a file written, flushed and read back;
  - two writers, one told of the conflict and nothing of it published;
  - a publication cut at each write leaves the old root or the new one.
- QEMU (x86, aarch64), once a files client reaches it:
  - the shell's file commands on a `storefs` volume;
  - a cut in the middle of a publication leaves the old root;
  - two writers, one told of the conflict.
- docs/storage, the profile, and `vfs.wit`'s change agreed with `KRN`.

## Related

[311](311-storefs-a-file-face.md), [311-STO-0029](311-STO-0029-a-working-tree-published-by-compare-and-swap.md), `vfs_server` (the kernel session's), 000-KRN-0170 (the programs' data directories).
