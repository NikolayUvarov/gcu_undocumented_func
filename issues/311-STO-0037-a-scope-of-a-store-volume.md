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

## Decisions while building (2026-10-31)

- **Built in a worktree of the storage session,** for the session to review and merge. No change to `idl/vfs.wit` (2.6's `scope` and `adopt` as they are), `common/abi.rs`, `init`, `vfs_server`, the shell or `mind::fs`.
- **`SCOPES` = 16, `SCOPES_EACH` = 8** (a client makes at most half, as every table's share). A scope keeps its directory's path and no tree until its user opens its root, about 0.6 KiB: the state grows from 786 to 796 KiB, still all zeros, in `.bss`. How many scopes work at once is bounded by `TREES` (4), as `mind::fs` keeps its roots open; the rest are those lent in the last minute and not taken up yet. `vfs_server` keeps 64 because the loader gives every program a directory of its own; `storefs`'s are only the shell's lending.
- **Badges:** `SCOPE_BADGE_FIRST` = `0x100`, as `vfs_server`'s; init's badges (`mind::fs::BADGE_*`) are below it. The next is kept as an offset from it, so the state stays all zeros; it wraps after `0xFFFF` (`BADGE_MAX`) and skips a badge a scope holds. A request with a badge from `0x100` is a scope's: of no scope, its root is `denied` and its handles `invalid`.
- **The directory, from the maker's handle:** its volume (in lower case) and the handle's own path from the volume's root. A scope made by a scope's client, of a handle of its own, is narrower still, a scope of its own in that client's share, and does not end with the one it came from, as on `vfs_server`.
- **Writable** is decided when it is made: asked, and the handle may write. A scope's root writes as its scope says: the service's predicate of badges is not asked for a scope's badge. So the badge rule is: `BADGE_USER` writes every volume, a scope's badge as its scope says, every other badge none.
- **Confinement is the handle's path:** a scope's root is a handle of a tree of the whole volume whose path is the scope's directory. Every call joins below a handle's path and refuses `.` and `..`, and `remove` and `rename` of a handle's own directory (an empty path) are `invalid`, so no new check was needed.
- **Its user:** the first task that asks a root with the badge is its user, whatever that root answers (another volume included), as on `vfs_server`, so a copy lent on is refused even after the first task's root was. `adopt` is asked by the maker's task: its PID, by any badge, as `vfs_server` compares the sender. Naming the scope's own user again is `ok`; another task, for a scope in use, `denied`; a badge of no scope, `not-found`.
- **Its end:** a scope taken up ends when `alive(user)` is false; one not taken up when more than `SCOPE_UNUSED_MS` (60 000) ms passed since it was made (at 60 s exactly it lasts). The logic is given the clock and `alive`; the service passes `mind::time::uptime_ms` and `mind::process::alive`. **A departure from `vfs_server`,** which sweeps only before `scope`: `storefs` also sweeps before `adopt`, before each request with a scope's badge, and when a table is full, so a scope that should have ended is neither adopted nor served.
- **What its end does:** its badge and capability go to the service, which revokes and drops the capability and logs `SCOPE <badge> ENDED`; the handles of its badge are closed unpublished, as `forget` closes those of an ended client, so its tree, files and chunks are free at once and its handles `invalid`.
- **The capability** is minted by a closure the service gives (`ipc::mint_badged(SLOT_SERVICE, CAP_WRITE | CAP_GRANT, badge)`, as `vfs_server`'s), after the bounds are checked: a refusal, or a capability that cannot be made, changes nothing, takes no badge and mints nothing. The logic keeps it as a number with the scope.
- **The service's lines:** `[STOREFS] SCOPE <badge> FOR <volume>:/<path> (WRITABLE|READ-ONLY)`, the badge as `0x100` as on `vfs_server`, and `[STOREFS] SCOPE <badge> ENDED`. Its `READY` line names `SCOPES` and `SCOPES_EACH`, and no longer cites 311-KRN-0190 when it holds no store client; the comments about 311-KRN-0190 being pending are corrected.
- **Not built:** a scope's end when its maker ends (`vfs_server` has none either: a scope lasts with its user), a call that ends one early, and a scope that follows its directory when moved (a path, not an object, as the Plan says).

## Progress (2026-10-31)

- **Built** (one commit on the session's worktree branch): the scopes in `storefs/src/fs.rs` (table, users, `adopt`, their end, the confinement and the bounds); `scope` and `adopt` in `storefs/src/main.rs`, with the minting, the revoking and the badge rule; seven host tests; docs/storage 0.24; the profile's two `storefs` rows; README's `storefs` row.
- **Ran,** on the storage session's container:
  - `tests/storefs_host.rs`: 19 passed (12 before, 7 new), 3.9 s; `tests/fs_routing_host.rs` (it reads `fs.rs`): 4 passed;
  - `storefs` built for x86 and aarch64 as `02_build.sh` and `scripts/build_aarch64.sh` build a crate, with no warning; its state in `.bss`;
  - each of 11 changes of the logic made a test fail: a scope writable when its handle is not, another volume allowed, a second user allowed, `adopt` by another task allowed, a scope nobody took up never ended, an ended scope's handles still served, the share not counted, a badge given while in use, a scope's root at its volume's root, a scope's root that writes whatever its scope says, and `adopt` that does not take a scope up.
- **Not run:** `scripts/host_tests.sh` whole, the images and QEMU (the session's). The QEMU criteria wait for APP's lending ([requests-APP.md](requests-APP.md)), so the issue stays open.

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
