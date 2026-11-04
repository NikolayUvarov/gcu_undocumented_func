# 311-APP-0121 — The shell lends programs a `storefs` client that does not write, and a scope for `REQUEST_FILE`

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (`REQUEST_FILE`'s scope checked; the other two parts wait for KRN) · **Blocked by:** for a program that only reads, and for `REQUEST_FILES`, KRN's answer to `KRN/2026-11-03T…-APP-storefs-for-programs-…`: the loader's plain `storefs` client, and `mind::fs`'s routing (311-KRN-0190's) · **Main task:** [311](311-storefs-a-file-face.md) · **Roadmap:** track B · **Constitution:** MC-3.1, MC-3.4

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

## Progress (2026-11-03)

- **`REQUEST_FILE` on a store volume needs nothing new in the shell.**
  - It makes the directory's scope through `mind::fs`'s `Dir::scope`, which goes to `storefs` for a store volume, and `storefs` serves it since 311-STO-0037.
  - `edit` takes it with `use_scope`.
  - **The `shell` suite's `storefs_check`** now runs `edit notes:docs/b.txt`, saves 10 bytes and reads them back with `cat`. `storefs` logs `SCOPE 0x… FOR notes:/docs (WRITABLE)`, and that scope `ENDED` at the next one's making. It passed on x86 QEMU with 4 CPUs (749de266 with 67347dee).
  - **What it shows about the end:** `storefs` ends an ended task's scope lazily (`end_scopes`), when it next makes one or is asked through one. It does not end it at the task's end.
- **The other two parts meet 311-KRN-0190's code, so they are asked of KRN:**
  - a plain, read-only client for every program, from the loader as for `vfs_server`. The shell's `BADGE_USER` client has no `CAP_BADGE` and cannot mint one;
  - for `REQUEST_FILES`, a copy of the shell's client, with `mind::fs::volumes::route` sending store volumes to `SLOT_STOREFS` and not to the lent client.

## Acceptance criteria

- **The `shell` suite's `storefs_check`:**
  - `find notes:` lists what the shell wrote there;
  - a program given the read-only client is refused a write;
  - [x] `edit notes:x.txt` writes through its scope (`notes:docs/b.txt`, 2026-11-03). That it reaches nothing beside it rests on `storefs`'s scope rules and their host tests; it is not run here.

## Related

[311-KRN-0190](../issues-done/311-KRN-0190-storefs-at-boot-and-its-clients.done), [311-STO-0032](311-STO-0032-storefs-a-vfs-view-of-a-named-root.md), [311-STO-0037](311-STO-0037-a-scope-of-a-store-volume.md), [311-APP-0116](311-APP-0116-the-shell-names-a-vfs-conflict.md).
