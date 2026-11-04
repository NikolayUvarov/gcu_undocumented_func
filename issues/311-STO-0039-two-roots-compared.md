# 311-STO-0039 — Two roots of a tree compared, and a name's older versions read

**Type:** storage (`libmind` `tree`, `blocks`) · **Owner:** `STO` track · **Priority:** P3 · **Status:** open · **Blocked by:** none · **Main task:** [311](311-storefs-a-file-face.md) · **Constitution:** MC-4.2, MC-4.9, MC-4.13

Split from [311](311-storefs-a-file-face.md) (2026-11-04), its plan's "History: old roots are snapshots; a directory's history and two roots can be compared". The other steps of 311 are done ([311-STO-0032](../issues-done/311-STO-0032-storefs-a-vfs-view-of-a-named-root.done), [311-STO-0037](../issues-done/311-STO-0037-a-scope-of-a-store-volume.done)).

## Problem

A name keeps its last versions (`blocks history`), and each is a whole tree. But `blocks tree` reads only a name's current version, and nothing tells what changed between two versions. Whoever wants to know what a writer changed, or to read a file as it was, has to fetch both trees and compare them by hand.

## Plan

- **`mind::tree::diff`,** in `libmind/src/tree.rs`, with no heap and no system calls:
  - it is given two roots, each a directory or none (the empty tree);
  - it reports each path where they differ: an entry added, removed, or changed (another object, size or kind), with the entry's kind, size and CID on each side;
  - **a subtree with the same CID on both sides is not read:** equal CIDs are equal trees (MC-4.2), so the work follows the change, not the tree;
  - a directory added or removed is one change, its contents not listed; a directory on both sides with another CID is gone into;
  - every node read is checked against its CID and the schema, and a directory's entry count against its parent's entry, as `walk` checks them;
  - the work is bounded by a number of steps (an entry compared, a node read), the depth by `DEPTH`, and the path by a buffer the caller gives.
- **`blocks tree`:**
  - `diff <name>[@<version>] <name>[@<version>]`: each change on a line, then how many;
  - `ls` and `cat` take `<name>@<version>`, so an older version kept in the name's history is read as it was. `@0` is the empty tree. Store names never hold `@`, so the form is unambiguous;
  - a version no longer kept is refused, naming it.
- **Tests:**
  - host (`tests/tree_host.rs`): each kind of change, at the root and below; equal subtrees left unread, counted by the blocks fetched; a directory added and removed whole; a kind changed; a missing, damaged or out-of-shape node refused; the steps, the depth and the path buffer bounded; a model comparison over random trees;
  - QEMU (`store_tree_check`, x86 and aarch64): between versions of `home`, a move and the removals reported, an unchanged name compared with itself reported equal, an older version listed and a file of it read back (SHA-256).
- **Docs:** docs/storage, and the profile's Article 4 rows.

## What it does not give

- **No merge:** a comparison says what changed; nothing applies one tree's changes to another (MC-4.9: a conflict is never merged).
- **No moves:** a file moved is reported removed at one path and added at another, as the trees hold no identity of a path.
- **Only kept versions:** a name keeps its last versions, as the store's retention says; an older one is gone with what only it retained.

## Acceptance criteria

- Host tests as in the Plan, each change of the logic named in the issue making one fail.
- QEMU (x86, aarch64): `store_tree_check`'s comparisons and the older version read back.
- docs/storage and the profile's rows.

## Related

[311](311-storefs-a-file-face.md), [311-STO-0027](../issues-done/311-STO-0027-directories-as-objects.done), [311-STO-0029](../issues-done/311-STO-0029-a-working-tree-published-by-compare-and-swap.done).
