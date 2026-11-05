# 311-KRN-0207 — `mind::fs` opens a volume's root again after its service restarts

**Type:** `libmind` (`mind::fs`) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [311](311-storefs-a-file-face.md) · **Constitution:** MC-6.6

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

## Progress

2026-11-04 (07400872):
- **`volumes::again`** (`libmind/src/fs/volumes.rs`): a call that answers `invalid` is asked once more after `forget` drops the kept root. A second `invalid`, one with no kept root, and any other answer come back as they are.
- **`mind::fs`** goes through it in `Dir::open`, `Dir::create`, `File::open_mode`, `remove`, `rename` and `volume`.
  - `Dir::open` and `Dir::create` of a volume's root check the root with a `stat`, so that `ls w:` finds a stale root out where it can still be opened again.
  - An open file's handle is never reopened.
- **Host tests** (`tests/fs_routing_host.rs`, 2 more): the root opened again once; a second `invalid`; no kept root; another answer.
- **QEMU x86** (this session's container, TCG): the shell suite's `storefs_check` passed. After `svc restart storefs`, `ls notes:docs`, `cat notes:docs/a.txt` and `ls notes:` answered, with no restart of the shell. The group's `bench` failed here on TCG's timing (`uptime (empty call)`), as it does in this container. It runs on KVM in the gate.

**Left:**
- aarch64;
- `vfs_server`'s roots: whether `vfs_server` restarts at all, which is not tried yet;
- `check`, whose visit runs once, so it is not asked again.

2026-11-04 (cd29f81d, the branch with `main` at 7ea919e9, on x32-128 through Effector, KVM):
- `x86: keys, shell, tools` and `aarch64: programs, shell and four CPUs` passed.
- On both, `storefs_check` printed "ls and cat again after storefs restarted (311-KRN-0207)".

**Left:** `vfs_server`'s roots and `check`, as above. `vfs_server`'s roots go through the same `again`, but no run restarted `vfs_server`, so that is not tested.

2026-11-04 (59213868), `vfs_server`'s roots:
- **`vfs_server` does restart.** `init` keeps only itself and the shell from a restart (`kept`), so `svc restart vfs_server` stops it with its flush and starts it again.
- **`vfs_restart_check`** runs late in the shell suite, after `storefs_check`.
  - It restarts `vfs_server` and waits for the new instance's `[VFS] THE BOOT VOLUME:`.
  - Then `ls data`, `write`, `cat` and `rm` of `data/after.txt` answer, with no restart of the shell.
  - It comes late because the services' open files on the old instance are not reopened.
- **Run:** passed in QEMU x86 in this session's container (TCG), the shell suite in full. aarch64 is not run yet.

**Left:** aarch64 for `vfs_restart_check`; `check`, as above.

2026-11-05 (43bcf218, x32-128 through Effector, KVM, the two lanes beside each other): `x86: keys, shell, tools` (398 s) and `aarch64: programs, shell and four CPUs` (391 s) passed, each with `storefs_check`'s restart and `vfs_restart_check`.

**Every acceptance criterion is met.** `check` is not asked again: its visit runs once. That is a limit of this change, not a criterion. The issue closes once this lands.

## Related

STO's 311-STO-0032; [311-KRN-0203](../issues-done/311-KRN-0203-a-lent-files-client-and-a-held-storefs-client.done) (the routing).
