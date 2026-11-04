# 311-KRN-0207 — `mind::fs` opens a volume's root again after its service restarts

**Type:** `libmind` (`mind::fs`) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [311](311-storefs-a-file-face.md) · **Constitution:** MC-6.6

**Asked by:** STO, 2026-11-04 (`requests-KRN.md`, taken out of it with this issue), from a run by hand on x86 while building 311-STO-0032's QEMU checks.

## Problem

**What STO saw.** `svc restart storefs` was run while the shell had used the store volume `w:`.
- The writer's next write was `invalid`, and nothing of its data was published. That is what the store promises.
- The shell's own `ls` and `cat` of the volume then answered `INVALID PATH OR REQUEST`, and starting a program with a file there answered `NOT A PROGRAM FILE`.
- It stayed so until the shell was restarted.

**The cause.** `mind::fs` keeps the root handle it opened for each volume, and the new instance knows none of the old one's handles. `init` restarts a boot service that ends, so a `storefs` that fails once leaves every program that used a store volume without it. A client must recover on its own terms (MC-6.6). `vfs_server`'s roots may do the same; that was not tried.

## Plan

1. **A kept root that answers `invalid`** is opened again once, and the call is asked once more. This covers a call on the root itself, and a path resolved from it.
2. **A handle of a file that was open is not reopened.** The program is told, as now: its position and its writes belong to the old instance.
3. **Both kinds of volume.** The same for `vfs_server`'s roots, checked by restarting it in QEMU if that is allowed. `vfs_server` may not restart at all; then that is recorded.

## Acceptance criteria

- **Host tests** (`tests/fs_routing_host.rs` or a file of its own):
  - a root that answers `invalid` is reopened once and the call succeeds;
  - a second `invalid` is returned as it is;
  - an open file's handle is never reopened.
- **QEMU, x86 and aarch64** (the `store` or `shell` suite): after `svc restart storefs`, the shell's `ls` of a store volume answers again without a restart of the shell.

## Related

STO's 311-STO-0032; [311-KRN-0203](../issues-done/311-KRN-0203-a-lent-files-client-and-a-held-storefs-client.done) (the routing).
