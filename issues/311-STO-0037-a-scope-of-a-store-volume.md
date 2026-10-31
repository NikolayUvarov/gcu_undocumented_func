# 311-STO-0037 — A scope of a store volume: a subtree as a capability, read-only or writable, from `storefs`

**Type:** storage (`storefs`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [311](311-storefs-a-file-face.md) · **Constitution:** MC-3.1, MC-3.4, MC-4.7

Split from [311](311-storefs-a-file-face.md) (2026-10-31): "rights: a mount is a capability for a subtree (MC-4.7)". It is what APP's request "The shell lends a `storefs` client to programs" in [requests-APP.md](requests-APP.md) waits for: "`storefs` minting a client that does not write, or serving `scope`". Until a program holds a client of `storefs`, [311-STO-0032](311-STO-0032-storefs-a-vfs-view-of-a-named-root.md)'s QEMU checks of two writers and of a cut publication cannot run.

## Problem

`init` gives the shell a client of `storefs` badged `mind::fs::BADGE_USER` (311-KRN-0190), which writes every store volume. The shell cannot lend it to a program: a copy keeps its badge. `storefs` answers `scope` and `adopt` with `denied`, so no client narrower than the shell's can be made, and a program holds none.

## Plan

- **`scope(dir, writable)`,** as `vfs.wit` 2.6 says and `vfs_server` does, with no change to the interface:
  - `dir` is a directory handle of the caller;
  - `storefs` mints a capability of its own endpoint with a badge of the scope's (from `0x100`, as `vfs_server`'s), and keeps it, so ending the scope revokes every copy;
  - the scoped client's `root` of the directory's volume is that directory, and every other root is refused (`denied`);
  - it may write only if `writable` and the handle may (never more than `dir`, MC-3.4);
  - a read-only scope of a volume's root is a client that does not write.
- **Its user:** the first task that opens a root with the badge is its only user, and the scope ends when that task ends. A scope nobody takes up ends after a minute. `adopt(scope, task)` makes it a named task's, asked only by the task that made it; one another task already uses is refused (`denied`).
- **The directory is a path,** from the volume's root, as a `storefs` handle's is: its root is opened at the name's current version, as any root, and follows the name. If another writer removes the directory, the scope's root is `not-found`.
- **Bounds:** `SCOPES` scopes at most, each client's share half of them, as every other table of `storefs`. Past the share or the table, `handles`.
- **The logic in `fs.rs`,** host-tested: the scopes' table, their users, ending and confinement, given a clock and a test of whether a task is alive. The minting, revoking and the badge rule in `main.rs`.
- **The stale comments of `storefs/src/main.rs`** about 311-KRN-0190 go, as it is done.

## What it does not give, said in docs/storage

- **A path, not an object:** a scope names a directory by its path. A directory moved by another writer is not followed, and a new one made at that path is the scope's.
- **No QEMU check yet:** a scope reaches a program only through the shell's lending, which is APP's (requests-APP.md). The host tests check the logic, and the QEMU checks wait for that lending.

## Acceptance criteria

- **Host** (`tests/storefs_host.rs`):
  - a scope's root is its directory, on its volume only; a path cannot leave it; another volume is refused;
  - a read-only scope, or one made from a handle that may not write, refuses every change (`denied`), and so does a handle opened from it;
  - the first task to use a scope is its only user; another is refused; `adopt` by its maker names its user, and by another task, or of a scope in use, is refused;
  - a scope ends with its task, and after a minute unused; its handles then answer `invalid`;
  - its directory removed by another writer: `not-found`, and made again, the scope reaches the new one;
  - the table and each client's share, refused past them with nothing changed.
- **QEMU** (x86, aarch64), once APP lends a program a scope:
  - a program run with a read-only scope of a store volume lists and reads it, and its writes are refused;
  - with a writable scope of a directory, it writes there and nowhere else.
- docs/storage and the profile's rows on rights.

## Related

[311](311-storefs-a-file-face.md), [311-STO-0032](311-STO-0032-storefs-a-vfs-view-of-a-named-root.md), [311-KRN-0190](../issues-done/311-KRN-0190-storefs-at-boot-and-its-clients.done), [requests-APP.md](requests-APP.md), `vfs_server`'s `scope` (2.2) and `adopt` (2.5).
