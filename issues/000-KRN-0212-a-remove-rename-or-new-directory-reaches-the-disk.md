# 000-KRN-0212 — A remove, a rename or a new directory reaches the disk at once

**Type:** `vfs_server` · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Roadmap:** storage · **Constitution:** MC-6.3, MC-12.3

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

- [x] QEMU, x86 and aarch64 (`linkupdate` suite): through a client that flushes nothing of its own, `link`, the host reads a put's rename, a `mkdir`, an `mv` and an `rm` from the image while the machine runs, and the check fails without the fix.
- [x] The docs say what a pulled disk may lose.

## Progress

2026-11-05, on `claude/000-KRN-0212-vfs-flush`:
- **`vfs_server`:** `Remove` and `Rename` end with a flush of their volume, and `OpenDir` with one when it made a directory (`Volume::changed`).
- **A first check, in the `vfs` suite through the shell, passed without the fix** (acc-1, x86, f0798294e). The shell's `mkdir`, `rm` and `mv` flush the volume themselves (`shell/src/files.rs`), and so does `edit`. It is dropped.
- **Clients that do not flush:**
  - `link`: a put's rename (`link/src/main.rs`), and its `mkdir`, `mv` and `rm`;
  - `effector`'s journal (a remove and a rename);
  - `effector_gw`, and some of `keystore`'s.
- **The check** goes through `link` instead, in the `linkupdate` suite, and is run without the fix on `claude/000-KRN-0212-check-alone`.
- **Docs:** `docs/profile/threat-model.md` (power loss), `docs/profile/evidence.md`, `docs/write-disk.md` 1.7 and `docs/write-disk_RU.md` 1.7.
- **Host:** `tests/fat_host.rs` (18) and `tests/vfs_zone_host.rs` (7) pass.
- **QEMU, through Effector:**
  - **without the fix** (`claude/000-KRN-0212-check-alone` ea4790c4, acc-1, x86): `the updater and an update through the link` failed at the first check. The put's last file, `releases/6/x86_64/MANIFEST.SIG`, was not on the disk under its name;
  - **with it** (6b86e4cf): `x86: the updater and an update through the link` passed on acc-1, and `aarch64: an update through the link` on acc-2;
  - **on ad906c8b, the fix with the shell's check:** `x86: files and block writes` (acc-1) and `aarch64: files and the block store` (acc-2) passed.

## Related

800-LNK-0024 (the owner's settings across a write), 750-EFF-0016, `vfs_server/src/main.rs`, `vfs_server/src/disk.rs`.
