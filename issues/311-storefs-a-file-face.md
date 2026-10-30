# 311 — A file face: trees as objects, and `storefs`, a VFS view of a named root

**Type:** main task · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** [307](307-the-stores-own-partition.md) · **Roadmap:** track B "file view over the store" · **Constitution:** MC-4.1, MC-4.7, MC-4.9, MC-11.10

From the kernel session's draft (`STO/2026-10-25T1557Z-KRN-draft-plan-the-store-as-a-minds-memory.md` on `internal-dev-messages`), taken by the storage session at the maintainer's word (2026-10-27). Everything here is plan, not a guarantee (MC-12.3).

## Problem

Programs keep their data as files on FAT, which has no history, no snapshots, no checksums and no rights finer than a zone. The store has all of these, but no file view.

## Plan

- **A directory node:** a canonical DAG-CBOR node of sorted entries (name, kind, CID, size); large directories sharded.
- **`storefs`,** a `vfs.wit` provider that mounts a named root:
  - reads go straight to the store; writes go into a working tree in memory (MC-4.9);
  - **`fsync` and a close publish** a new root by compare-and-swap, as programs written for files expect; an explicit **`commit`** publishes several files' changes at once (the storage session's choice, 2026-10-27);
  - a conflict is reported to the writer, never merged silently.
- **History:** old roots are snapshots; a directory's history and two roots can be compared.
- **Rights:** a mount is a capability for a subtree (MC-4.7).
- **What moves:** the programs' own data directories (000-KRN-0170) can live here. FAT stays for the boot volume and for exchange with other systems, as a projection with an authorized import (MC-11.10).
- `APP`'s part, a request: `fm` and the shell over `storefs`.

## Tasks

- [311-STO-0027](311-STO-0027-directories-as-objects.md): directories as objects, their walk, and the store's reachability through them.
- [311-STO-0029](311-STO-0029-a-working-tree-published-by-compare-and-swap.md): a working tree in memory, published by compare-and-swap; `blocks tree`.
- [311-STO-0032](311-STO-0032-storefs-a-vfs-view-of-a-named-root.md): `storefs`, a `vfs.wit` view of a named root; waits for `KRN` on how a files client reaches it.

## Acceptance criteria

- QEMU: the shell's file commands on a `storefs` mount; a cut in the middle of a publish leaves the old root; two writers, one of them told of the conflict.

## Related

[301](../issues-done/301-objects-as-merkle-dags.done), [302](../issues-done/302-names-and-current-roots.done), [304](../issues-done/304-several-names-at-once.done), [307](307-the-stores-own-partition.md).
