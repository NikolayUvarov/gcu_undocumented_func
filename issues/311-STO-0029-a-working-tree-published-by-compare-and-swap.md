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
