# 175-KRN-0047 — A FAT write that fails for space gives its new clusters back

**Type:** kernel (`vfs_server`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [175](175-audit-2026-10-09.md) · **Constitution:** MC-4.4, MC-4.5

## Problem

Audit finding A02 ([audit](../issues-audit/2026-10-09-repository-audit.md), [assessment](../issues-audit/2026-10-09-repository-assessment.md)):
- `write_raw` in `vfs_server/src/fat.rs` (from line 463) allocates and links the whole extent before writing data.
- `allocate()?` returns on `NoSpace` with the chain linked. The new first cluster is only in the local `node`, and the `Write` handler returns before `refresh`.
- **An empty file** loses the chain unreachably.
- **A non-empty file** keeps clusters linked past its size.
- MIND Core has no repair, so the space stays lost until an external `fsck`. The trigger is any write larger than the free space.

## Plan

- Growth allocates the clusters first and links them only when all are found. On failure, the ones taken are freed, and the file keeps its old chain and size.
- A partial grow that the file's size does not reach is cut back.
- `issues-audit/repro/fat_repro.rs`'s A02 assertion becomes a test in `tests/fat_host.rs`:
  - empty and non-empty files, writes with gaps and growing truncation, under `NoSpace` and injected I/O errors;
  - free counts, contents and `check()` after a remount.

## Acceptance criteria

- After a refused write, the free cluster count is as before and `check()` finds no lost or cross-linked cluster.
- The file reads as before.

## Related

[066](../issues-done/066-vfs-v2-fat-write.done), `docs/profile/evidence.md` (FAT consistency).
