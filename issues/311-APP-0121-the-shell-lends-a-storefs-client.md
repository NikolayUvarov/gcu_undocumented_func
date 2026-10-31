# 311-APP-0121 — The shell lends programs a `storefs` client that does not write, and a scope for `REQUEST_FILE`

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** `storefs` minting a client that does not write, or serving `scope` (the storage track; [311-STO-0037](311-STO-0037-a-scope-of-a-store-volume.md) serves `scope` and `adopt`, in its landing request) · **Main task:** [311](311-storefs-a-file-face.md) · **Roadmap:** track B · **Constitution:** MC-3.1, MC-3.4

**Recorded by:** the tools track, 2026-10-31, from KRN's request in `issues/requests-APP.md` (2026-10-30, for the storage track's 311-STO-0032).

## Problem

- **What is in place** with [311-KRN-0190](../issues-done/311-KRN-0190-storefs-at-boot-and-its-clients.done):
  - `init` gives the shell a `storefs` client badged `mind::fs::BADGE_USER` in `SLOT_STOREFS` (42, a launch slot);
  - `mind::fs` asks it for every volume but `vfs_server`'s, so the shell's own `ls`, `cat`, `write`, `mkdir` and `rm` reach a store volume such as `notes:`.
- **That client must not be lent as it is:** a copy keeps its badge, and `storefs` lets `BADGE_USER` write.
- **Today a program holds none,** so `find notes:` says `NOTFOUND` and asks nobody.

## Plan

Once the storage track's part is there:
- **A client that does not write,** in the program's `SLOT_STOREFS`, for every program that reads files, as `vfs_server`'s plain client reaches every program.
- **For `REQUEST_FILE` on a path on a store volume,** a scope of that directory in `SLOT_FILE`, writable where the user may write, as for a FAT volume.
- **The script word:** `file` and `files` cover them, as for the other volumes.

## Acceptance criteria

- **The `shell` suite's `storefs_check`:**
  - `find notes:` lists what the shell wrote there;
  - a program given the read-only client is refused a write;
  - `edit notes:x.txt` writes through its scope and reaches nothing beside it.

## Related

[311-KRN-0190](../issues-done/311-KRN-0190-storefs-at-boot-and-its-clients.done), [311-STO-0032](311-STO-0032-storefs-a-vfs-view-of-a-named-root.md), [311-STO-0037](311-STO-0037-a-scope-of-a-store-volume.md), [311-APP-0116](311-APP-0116-the-shell-names-a-vfs-conflict.md).
