# 311-STO-0032 — `storefs`: files and directories of a named root, through `vfs.wit`

**Type:** storage (a new service `storefs`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** [311-KRN-0190](311-KRN-0190-storefs-at-boot-and-its-clients.md) (`storefs` at boot and its clients), the kernel track's, from the request of 2026-10-30; [311-KRN-0189](../issues-done/311-KRN-0189-a-conflict-error-in-vfs-wit.done) (`vfs.wit` 2.6 `conflict`) is done · **Main task:** [311](311-storefs-a-file-face.md) · **Constitution:** MC-4.1, MC-4.7, MC-4.9, MC-11.10, MC-12.4

Split from [311](311-storefs-a-file-face.md) (2026-10-30), its third step, after [311-STO-0029](../issues-done/311-STO-0029-a-working-tree-published-by-compare-and-swap.done).

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

- **Built in a worktree of the storage session,** for the session to review and merge: first the logic alone, host-tested; then, on the kernel track's answer, the service binary and its build entry. No `idl/*.wit` or `init` change: those are 311-KRN-0189 (`conflict` in `vfs.wit` 2.6) and 311-KRN-0190 (`init` starts `storefs` and lends its clients).
- **`storefs/src/fs.rs`,** with no heap and no system calls, takes the store as traits: `dag::Blocks`, `worktree::Names`, and its own `Roots` (a name's current version and root), so `blocks`'s client of `Names` is unchanged. Its state is 786 KiB, all zeros, so the service keeps it in `.bss` (checked on a `no_std` build against `libmind`).
- **A tree for each `root`:** each `root` opens a working tree of its own at the name's current version, and the handles opened from it share it. Two clients, or two roots of one client, are two writers.
- **A tree follows its name** (the storage session's review, 2026-10-30, as `mind::fs` keeps its roots for good): before each call on a tree that holds no unpublished data, the name's current version is read, and if it moved the tree moves to it. Its open files are read again at their paths; a handle whose entry is gone, or is no longer of its kind, is closed, as `vfs_server` closes the handles of what is removed. A tree holding unpublished data stays where it is, so a writer's own reads do not change under it. Between calls a tree holds no unpublished edit: what a call put in it is published by that call or dropped.
- **Handles** are a table's slots, as `vfs_server`'s are: each keeps its client's PID and badge, its tree, its path from the tree's root, and whether it may write. Another client's handle is `invalid`. A root may write if the service's predicate says so of the badge and the volume (an argument of `root`); everything opened from a handle may write only if it may, and a file only if opened with `write`.
- **A file written** is held as the chunks of 16 KiB it changed, from a pool of 16 shared by every file (256 KiB). A chunk is read from the file's object when first written, unless the write covers it. A truncation lets the chunks past the end go and zeroes the rest of the last; the object is no longer read past the new size. A write that needs more chunks than are free takes none (`Full`), until a flush lets them go.
- **A file is at most 64 MiB** (`FILE_MAX`, the storage session's review): a write or a truncation that would leave it larger is refused (`no-space`), nothing changed, so storing a file reads and puts 4096 chunks at most. A larger file the store holds is read whole, and may be cut down to the bound.
- **Storing a file** puts its chunks and the object's bytes it did not change through one `dag::Builder`, kept with the state: `Builder::reset` (new, with a test in tests/dag_host.rs) starts each object. The object goes into the tree in place of the old one by **`worktree::replace`** (new, with tests in tests/worktree_host.rs and among its random edits): one edit, so a refusal leaves the old file.
- **What publishes** (changed on the storage session's review, 2026-10-30, as `mind::fs`'s `mkdir`, `remove` and `rename` never flush): **each change of a directory is one publication, made by its own call**: `open-dir` with `create` (all the directories it makes), `open` that makes or empties a file, `remove` and `rename`. **Only a file's data waits** for the `flush` or `close` of a handle that may write, or the `flush` of a directory handle that may; a directory's flush stores every file written below it and publishes them once. A flush through a handle that may not write publishes nothing and answers `ok`; a tree with nothing changed is not published again (MC-4.9: the commit points are these calls).
- **After a conflict** nothing is published, and the call that would have published is told. The tree drops what it did not publish and moves to the name's current version, never merging: a file whose written data was dropped answers that conflict (`io`) to every later call, `close` included, and `close` still closes its handle, as `vfs_server`'s close does. The writer makes its change again from the current version, through the same root: no client keeps conflicting. What a refused publication stored is retained by nothing, so a collection after its lease (60 s) frees it.
- **A file the working tree cannot hold** for `DIRS` or `BYTES` at a flush is put after the files before it are published, so that flush publishes twice. Three files 31 directories down in three branches need 94 held directories; the host test flushes them so.
- **The bounds:** `TREES` = 4, `HANDLES` = 64, `FILES` = 32, `CHUNKS` = 16, `FILE_MAX` = 64 MiB, `PATH` = 512 bytes of a handle's path from its root (two names of 255 bytes), `PAGE` = 16 entries a `list`. Each is refused with nothing changed; a call's directories are dropped if one of them is refused.
- **Each client's share** (the storage session's review): a client, by PID and badge, holds at most half of each table: `TREES_EACH` = 2, `HANDLES_EACH` = 32, `FILES_EACH` = 16, `CHUNKS_EACH` = 8 (128 KiB). Past its share a request is refused as past the table (`handles`, or `Full` for chunks), nothing changed, so a second client always finds its own share free. A tree, its files and their chunks count for the client that opened its root.
- **Volumes:** 1 to 16 bytes of `A-Z a-z 0-9 . _ -`, the case ignored as `vfs_server` ignores it, so the store's name is `fs/` and the volume in lower case. Another name is `not-found`.
- **Errors are the logic's own** (`fs::Error`), with `wire()` to `vfs.wit`'s (2.5 when built, 2.6 since 311-KRN-0189, below):
  - each one `vfs.wit` has is itself; the tables' bounds are `handles`, and `Full`, `TooLarge` and a store with no room are `no-space`;
  - **`Conflict` is `io`** until `vfs.wit` 2.6 has `conflict`: nothing of the write was stored under the name, as for a write that did not reach a medium, so no client takes its data for stored. `invalid` or `denied` would blame the request or its rights, and a retry of an `io` flush is refused again with nothing published;
  - the store's other errors (a block missing, corrupt or out of shape) are `io`.
- **Sizes and offsets are `u64`** in the logic; `vfs.wit`'s are `u32`, and `FILE_MAX` keeps every file written within them.
- **The way in** (the kernel track's answer, 2026-10-30, way (b)): `mind::fs` asks `vfs_server` first, and `storefs` for a volume `vfs_server` answers `not-found`. So `fs::root` refuses the boot disk's empty name, `ram`, `log` and `models` (`fs::VFS_VOLUMES`, any case) as `not-found`; any other volume `v` is `fs/<v>`. The conflict's wire value is set in one place, `fs::Error::wire`, to become 2.6's `conflict` with 311-KRN-0189.
- **The service, `storefs/src/main.rs`:** the logic behind `vfs.wit` 2.5's generated server code, as `vfs_server` dispatches it; each request's PID and badge are its client. The state (786 KiB) and the buffers are statics. Pending with 311-KRN-0190: the store's client, looked for in `SLOT_BLOCKSTORE` as `tally` and `blocks` look for theirs, and the badge rule, the user's badge (`mind::fs::BADGE_USER`) writing every volume and every other badge reading. `check`, `scope`, `format`, `fill` and `adopt` are `denied`: they are `vfs_server`'s. `volume` gives the label `MIND STORE`, FAT type 0, no room, and the chunk as the cluster. When a table is full, the handles of ended clients are closed unpublished (`fs::forget`), as `vfs_server` frees them. It logs one line, `[STOREFS] READY …`, which names a missing store client. Built into the x86 and aarch64 images as `storefs.elf` (`02_build.sh` after `blocks`; `scripts/build_aarch64.sh` takes that list), on this session's container; the x86 QEMU `store` suite passes with it in the image, where nothing starts it.
- **Not in the logic:** `check`, `scope`, `format`, `fill` and `adopt`; `modified` stamps, which a tree does not keep.
- **Host tests** (tests/storefs_host.rs, the store's logic on a simulated medium): each call and its refusals, each change of a directory one version; a file flushed and read back by SHA-256, and 400 random writes, truncations and flushes against a model; a directory's flush; a reader's open root, file and directory following a writer; two writers, the second told of the conflict and writing again through the same root; a change of a directory refused by a conflict on its own call; a flush stopped at each of its 9 writes, made or torn, leaving the old root or the new; the rights; each bound and each client's share, and a second client getting its share; a root that is not a directory; the error mapping against the generated `vfs.wit` 2.5. Each of 22 changes of the logic made a test fail: among them a conflict ignored, handles open to every client, a tree that does not follow, or follows with unpublished data, a stale file that answers, a removal, move or made directory that waits, `FILE_MAX` and each share left out.

## Progress (2026-10-30)

- **Merged by the session** (3feb85d9, 17007ec8, 9af247ac). The kernel track chose way (b) on 2026-10-30: `mind::fs` asks `vfs_server` first, and `storefs` for a volume `vfs_server` answers `not-found`.
- **Tested:**
  - `scripts/host_tests.sh` whole (`storefs` 12) on eea5d440;
  - both images built with `storefs.elf`, and `store`, `storefaults` and `vfs` still pass on x86 and aarch64, on `un-rizen_2a8b108d5593` through Effector.
- **Off the images** since 2026-10-30 (the landing round of 16:17 returned it): on a disk, `storefs.elf` was listed as a program the user runs (`wm`'s menu, `list -l`). It stays out of `02_build.sh` until 311-KRN-0190 makes it a boot image.
- **`vfs.wit` 2.6** (311-KRN-0189, round 57, `main` ad3f64ff): a conflict is now `conflict` on the wire, not `io`, so a client reads the file again and writes once more. `tests/storefs_host.rs` checks the mapping against the generated 2.6 enum. Merged with `main` as a3d171b7, and taken up on this branch at once, because the check of 2.5 failed on the merge.
- **Not run:** nothing starts the service until 311-KRN-0190. The QEMU criteria wait for it, and for APP's lending to programs.

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

[311](311-storefs-a-file-face.md), [311-STO-0029](../issues-done/311-STO-0029-a-working-tree-published-by-compare-and-swap.done), `vfs_server` (the kernel session's), 000-KRN-0170 (the programs' data directories).
