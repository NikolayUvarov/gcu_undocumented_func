# 311-STO-0029 — A working tree published by compare-and-swap: edits in memory, a new root under a name

**Type:** storage (`libmind` `worktree`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — ([311-STO-0027](../issues-done/311-STO-0027-directories-as-objects.done), directory nodes, done) · **Main task:** [311](311-storefs-a-file-face.md) · **Constitution:** MC-4.9, MC-4.10, MC-4.13, MC-12.4

Split from [311](311-storefs-a-file-face.md) (2026-10-30), its second step: what `storefs` will do on a write, before there is a `storefs`.

## Problem

After [311-STO-0027](../issues-done/311-STO-0027-directories-as-objects.done) a directory is an object, but nothing makes one from edits. A writer must not change a published root in place (MC-4.9). It changes a working copy and publishes a new root, and must learn when another writer published first instead of overwriting that writer's work.

## Plan

- **`mind::worktree` (`libmind/src/worktree.rs`)**, with no heap and no system calls. Its bounds are fixed and named, and an edit past them is refused (`Full`) until the writer publishes:
  - **A working tree** starts from a published root (or none, an empty tree) and the name's version it was read at.
  - **Edits:**
    - put a file (an object already in the store, with its size) at a path;
    - make a directory;
    - remove an entry, a directory only when empty;
    - rename within the tree.

    Each takes a path of `/`-separated names, checked as `mind::tree` checks names. A missing parent, a file where a directory belongs, or a name taken is refused.
  - **Reads** see the edits: lookup and list of a path, with the unchanged parts read from the store.
  - **Write-out** puts the changed directories, deepest first, and returns the new root. Unchanged subtrees keep their CIDs.
  - **Publish** is the name's compare-and-swap (`blockstore` `publish` with the version read):
    - if another writer published since, nothing is published and the writer is told both versions (a conflict, never a silent merge);
    - the blocks already put stay unreferenced until the next collection.
- **`blocks tree`** (the store's shell tool), each command one publish:
  - `blocks tree ls <name> [path]`, `blocks tree cat <name> <path> <file>`;
  - `blocks tree put <name> <file> <path>`, `blocks tree mkdir <name> <path>`, `blocks tree rm <name> <path>`, `blocks tree mv <name> <from> <to>`;
  - `[expected version]` as the last word to publish against a version given, which is how a test plays a second writer.
- **docs/storage:** the working tree, its bounds, and what a conflict leaves.

## Decisions while building (2026-10-30)

- **Built in a worktree of the storage session,** as 311-STO-0027 was; for the session to review and merge.
- **What is held:** each directory an edit changed, with every directory above it, as its canonical node, in one area of bytes with its name. A held directory's entry in its parent keeps its current entry count; only its CID waits for the write-out. So every bound is checked at the edit, and a write-out never grows a node.
- **The bounds:** `DIRS` = 64 directories and `BYTES` = 64 KiB, about 85 KiB with the read buffer: two paths of `tree::DEPTH` at once, or four full directories. The tree is all zeros when new (CIDs kept in their binary form, zeros for none), so `blocks` keeps it in `.bss`; with `Option<Cid>` it put 90 KiB into the program's file.
- **Errors are the working tree's own** (`worktree::Error`): `Name`, `NotFound`, `NotDirectory`, `Exists`, `NotEmpty`, `Inside`, `Full`, `TooLarge`, `Conflict { read, current }`, and `Store(dag::Error)` for the store and the blocks read. `Full` is its own bounds, which a publication lifts; `TooLarge` is the tree's (a node over a block, `tree::DEPTH`), which it does not.
- **A refused edit changes nothing:** the directories it began to hold are let go; the nodes it would change are encoded first and checked against the bounds, then made, those that shrink first.
- **A put does not replace a file:** a name taken is refused for every edit, as the plan says. A new version of a file is a removal and a put in one tree, one publication.
- **A put reads the object's root** (`dag::size`): an object not stored is `Store(NotFound)`, a size not its own `Store(Shape)`, a directory `Store(Format)`. The store's walk would refuse each at the publication; here the edit is refused.
- **Paths:** names separated by `/`, a leading `/` allowed, the root the empty path; each name as `tree::valid_name` (now public, and `Entry::valid` uses it).
- **A move checks the depth of where it goes, not the height of what it moves;** a tree too deep is refused by the store's walk at the publication (`invalid`), and nothing is published. The height of an unread subtree would need a walk of it.
- **The publication's store is a trait,** `worktree::Names`: `publish` (a new version, or none for a conflict) and `version` (the current one, from `snapshot`, so a removed name gives its removal's). The current version in a conflict is read after the refusal and may be newer than the one that refused. Published, the tree starts again from the new root and version; refused, it stays as it was.
- **`tree`:** `encode_each` encodes entries given one by one (`encode` uses it), and `Dir::bytes` gives a node's bytes; both crate-visible.
- **`blocks tree`:** a version given as the last word is read from the name's history (a version it keeps; 0, the empty tree), so the second writer edits the tree it read. A conflict also names the root not published, written out again from the unchanged tree (its nodes are stored already), so the QEMU suite sees it freed by a collection.
- **Host tests:** besides the plan's cases, 2400 random edits against a model of the tree. The stopped publication is cut at the medium: each write in turn not made, or made in its first sector alone.
- **The QEMU check waits out the leases** (62 s by the guest's clock, as the collection check before it does), so the collection frees something and the files read after it are evidence.

## Progress (2026-10-30)

- **Built** in a worktree by an agent of the storage session and merged. The host tests and the QEMU checks pass:
  - on this session's container (x86, TCG), by the agent;
  - on `un-rizen_2a8b108d5593` through Effector (x86 and aarch64, TCG), on 254370a1;
  - `scripts/host_tests.sh` whole, on 3fe6d984.
- **The profile** (README, evidence; x86 and aarch64) and docs/storage say so.
- **Closes when landed.**

## Acceptance criteria

- Host tests (`tests/worktree_host.rs`, through the block store's own logic on a simulated medium):
  - every edit, its refusals, and reads that see it;
  - the bounds refused with `Full` and nothing changed;
  - a write-out's root decodes and walks (`mind::tree::walk`), and unchanged subtrees keep their CIDs;
  - two writers from one version: the first publishes, the second is told of the conflict and publishes nothing;
  - a store stopped at each call of a publish in turn leaves the old root or the new one, never a mix.
- QEMU `store` suite (x86, aarch64):
  - `blocks tree` makes a tree of two directories and three files under a name, lists it, and reads a file back (SHA-256 checked);
  - the name's history holds each root;
  - a publish against an older version is refused as a conflict, and the name is unchanged;
  - after `blocks collect` every file is still read.
- docs/storage and the profile.

## Related

[311](311-storefs-a-file-face.md), [311-STO-0027](../issues-done/311-STO-0027-directories-as-objects.done), [302](../issues-done/302-names-and-current-roots.done), [304](../issues-done/304-several-names-at-once.done).
