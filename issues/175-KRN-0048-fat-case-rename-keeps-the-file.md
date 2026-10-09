# 175-KRN-0048 — A case-only rename that fails keeps the file

**Type:** kernel (`vfs_server`) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [175](175-audit-2026-10-09.md) · **Constitution:** MC-4.4

## Problem

Audit finding A03 ([audit](../issues-audit/2026-10-09-repository-audit.md), [assessment](../issues-audit/2026-10-09-repository-assessment.md)):
- The `case_change` branch in `vfs_server/src/fat.rs` (line 678) unlinks the old entry before `link` writes the new one.
- `link` needs `parts + 1` contiguous free slots, so the rename fails in three cases:
  - a fragmented fixed root;
  - a subdirectory that cannot grow on a full volume;
  - an I/O error.

  The file is then gone.

## Plan

- The slots for the new name are found, or the directory grown, before the old entry is removed. Or the new entry is written first and the old one removed after it.
- On any failure the old name and its contents stay.
- Tests in `tests/fat_host.rs`:
  - mixed-case names that need more long-name slots, in a full root, a fragmented root and a full volume's subdirectory;
  - a failed sector write.

## Acceptance criteria

A refused rename leaves the file under its old name with its contents, and `check()` consistent.

## Related

[066](../issues-done/066-vfs-v2-fat-write.done), `tests/fat_host.rs` (the case-change test).
