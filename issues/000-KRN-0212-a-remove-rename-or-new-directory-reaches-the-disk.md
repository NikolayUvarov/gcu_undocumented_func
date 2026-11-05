# 000-KRN-0212 — A remove, a rename or a new directory reaches the disk at once

**Type:** `vfs_server` · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** storage · **Constitution:** MC-6.3, MC-12.3

From LNK's request of 2026-10-24 (`KRN/2026-10-24T1215Z-LNK-a-rename-or-remove-stays-in-vfs-servers-cache.md` on `internal-dev-messages`) and EFF's of the same day and of 2026-11-05 (`KRN/2026-11-05T1400Z-EFF-…`). It went unanswered for 12 days.

## Problem

**What was lost.** On 2026-10-24 the MacBook Pro's `data/effector/effector.conf` was lost:
1. `start.msh` removed each file and moved its new copy in, all on the boot volume.
2. The stick was pulled two hours later from the running machine.
3. The stick held the other two files, but not `effector.conf`.

**Why.** `vfs_server`'s cache (`disk.rs`, 64 sectors) writes back. A changed sector stays there until it is evicted or the volume is flushed.
- **What flushes:** a `Close` of a file opened for writing, `Flush`, and a boot record's write.
- **What does not:** `Remove`, `Rename`, and a directory that `OpenDir` makes. They change only directory entries and the FAT. So they stay in the cache, for hours if nothing else is written. A disk pulled out, or a machine that loses power, keeps only what was evicted by then.

## Plan

1. **A flush after each of them:**
   - `Remove` and `Rename` flush their volume once they are done;
   - `OpenDir` flushes when it made a directory.
   
   A failed flush is the request's error. The change stays in the cache and goes out with the next flush.
2. **No idle timer.** `vfs_server` serves requests and has no timer, so a file written but never closed still waits for its `Close` or a flush. The docs say so.
3. **Docs:** what a pulled disk may lose, in `docs/storage/` and `docs/profile/`.

## Acceptance criteria

- [ ] QEMU, x86 and aarch64 (`vfs` suite): after `mkdir`, `mv` and `rm` on the boot volume, with no `sync` and no file closed since, the host reads them from the image while the machine runs.
- [ ] The docs say what a pulled disk may lose.

## Related

800-LNK-0024 (the owner's settings across a write), 750-EFF-0016, `vfs_server/src/main.rs`, `vfs_server/src/disk.rs`.
