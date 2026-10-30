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

## Decisions while building (2026-10-30)

- **Built in a worktree of the storage session,** for the session to review and merge: the logic alone, host-tested. No service binary, build script, `idl/*.wit` or `init` changes: they wait on the kernel track.
- **`storefs/src/fs.rs`,** with no heap and no system calls, takes the store as traits: `dag::Blocks`, `worktree::Names`, and its own `Roots` (a name's current version and root), so `blocks`'s client of `Names` is unchanged. Its state is 786 KiB, all zeros, so the service keeps it in `.bss` (checked on a `no_std` build against `libmind`).
- **A tree for each `root`:** each `root` opens a working tree of its own at the name's current version. The handles opened from it share it, and it stays at that version until it publishes. Two clients, or two roots of one client, are two writers.
- **Handles** are a table's slots, as `vfs_server`'s are: each keeps its client's PID and badge, its tree, its path from the tree's root, and whether it may write. Another client's handle is `invalid`. A root may write if the service's predicate says so of the badge and the volume (an argument of `root`); everything opened from a handle may write only if it may, and a file only if opened with `write`.
- **A file written** is held as the chunks of 16 KiB it changed, from a pool of 16 shared by every file (256 KiB). A chunk is read from the file's object when first written, unless the write covers it. A truncation lets the chunks past the end go and zeroes the rest of the last; the object is no longer read past the new size. A write that needs more chunks than are free takes none (`Full`), until a flush lets them go.
- **Storing a file** puts its chunks and the object's bytes it did not change through one `dag::Builder`, kept with the state: `Builder::reset` (new, with a test in tests/dag_host.rs) starts each object. The object goes into the tree in place of the old one by **`worktree::replace`** (new, with tests in tests/worktree_host.rs and among its random edits): one edit, so a refusal leaves the old file.
- **What publishes:** the `flush` or `close` of a file handle that may write, and the `flush` of a directory handle that may. A directory's flush stores every file written below it, then publishes the tree once: a publication is always of the whole tree, so it also takes the tree's other edits. A file made or emptied by `open`, a directory made, a removal and a move are edits that wait for one of these; the tree's last handle closed first drops them (MC-4.9: the commit points are the flushes). A flush through a handle that may not write publishes nothing and answers `ok`. A tree with nothing changed is not published again.
- **After a conflict** nothing is published. The tree keeps its changes, never merged, and its handles read them; it stays at the version it read, so each later flush of it is refused the same way. A `close` refused so closes its handle all the same, as `vfs_server`'s close does. The writer opens a new `root` to read the new version. What the refused tree stored is retained by nothing, so a collection after its lease (60 s) frees it, and a read of it after that fails.
- **A file the working tree cannot hold** for `DIRS` or `BYTES` at its flush is put after a publication of the edits before it, so that flush publishes twice. Otherwise a tree that took 64 directories in edits could never store a file in a directory it does not hold.
- **The bounds:** `TREES` = 4, `HANDLES` = 64, `FILES` = 32, `CHUNKS` = 16, `PATH` = 512 bytes of a handle's path from its root (two names of 255 bytes), `PAGE` = 16 entries a `list`. Each is refused with nothing changed; a path of directories made by `open-dir` is taken out again if one of them is refused.
- **Volumes:** 1 to 16 bytes of `A-Z a-z 0-9 . _ -`, the case ignored as `vfs_server` ignores it, so the store's name is `fs/` and the volume in lower case. Another name is `not-found`.
- **Errors are the logic's own** (`fs::Error`), with `wire()` to `vfs.wit` 2.5's:
  - each one `vfs.wit` has is itself; the tables' bounds are `handles`, and `Full`, `TooLarge` and a store with no room are `no-space`;
  - **`Conflict` is `io`** until `vfs.wit` 2.6 has `conflict`: nothing of the write was stored under the name, as for a write that did not reach a medium, so no client takes its data for stored. `invalid` or `denied` would blame the request or its rights, and a retry of an `io` flush is refused again with nothing published;
  - the store's other errors (a block missing, corrupt or out of shape) are `io`.
- **Sizes and offsets are `u64`** in the logic; `vfs.wit`'s are `u32`, and checking them is the service's.
- **Not in the logic:** `check`, `scope`, `format`, `fill` and `adopt`; `modified` stamps, which a tree does not keep.
- **Host tests** (tests/storefs_host.rs, the store's logic on a simulated medium): each call and its refusals; a file flushed and read back by SHA-256, and 400 random writes, truncations and flushes against a model; a directory's flush; two writers; a flush stopped at each of its 8 writes, made or torn, leaving the old root or the new; the rights; each bound; a root that is not a directory; the error mapping against the generated `vfs.wit` 2.5. A flush that ignored a conflict, and handles open to every client, each made a test fail, and so did six other changes of the logic.

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
