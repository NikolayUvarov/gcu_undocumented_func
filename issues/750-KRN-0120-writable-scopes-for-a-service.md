# 750-KRN-0120 — Writable `vfs` scopes for a boot service, kept across its restarts

**Type:** `vfs_server`, `init` · **Owner:** kernel session (`vfs_server` since 175) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.4, MC-3.7

## Problem

`vfs_server` lets only the shell's `BADGE_USER` client write; services get read-only clients. The Effector gateway must write its audit trail, delivered files and installed applications, which other programs, the shell and the loader read. It must not write anywhere else on `data/`. A private directory in `system/` (351-KRN-0040) covers its journal, not these. The agent track's request of 2026-10-09 is in [requests-KRN.md](requests-KRN.md).

`vfs.wit` 2.2 already has `scope(dir, writable)`: a client confined to one directory, which may change files there if the caller may. Two parts are missing for a boot service:
- a scope works for the first task that uses it and ends with that task, so a restarted service has none;
- no case shows rename and flush within a scope, and a refusal outside it.

## Plan

1. **`init` makes the scopes.** From its own client with the user's rights on `data/`, `init` opens each directory named in the service's arm, creates it if missing, and asks `scope(dir, true)` for each; the service gets one slot a directory.
2. **A restart gets new ones.** When `init` restarts the service, it asks again; the old scopes ended with the old task.
3. **Inside a scope:** create, write, rename within the scope, flush and delete. A path outside it, and a rename out of it, is refused with the error a missing file gets, so a scope does not tell what lies outside.
4. **No `vfs.wit` change** is expected. If one is needed, it is a minor version.

## Progress (2026-10-20)

Plan steps 1 and 2 are made for one service, `link` (800-KRN-0128): at each start `init` mints a `BADGE_USER` client from `vfs_server`'s keeper, opens `data/link/` (made if missing), asks `scope(dir, true)`, moves the scope into the service's `SLOT_VFS` and drops its own client. The link track's QEMU suite shows a file written and read in the scope and a name outside it refused. A general list of scopes per service in `init`'s arms, and the `isolation` case with a restart, are left.

## Acceptance criteria

- [ ] A new `isolation` case (x86 and aarch64): a test service with a writable scope creates, renames and flushes files under its directory, and is refused everywhere else, a rename out of the scope included.
- [ ] After `init` restarts it, the service writes through its new scopes, and the old ones are refused.

## Progress (2026-10-21)

800-KRN-0128 is done (`main` 8def173). Its last point, a name outside `data/link/` refused by `vfs_server` and not only by `link`, moves here: the `isolation` case above shows it for a scope.

## Progress (2026-10-22)

Made on `claude/youthful-mendel-mf1soy` (84fb0826, with 9127f31e and dbe24fcb):

- **The Effector gateway's delivery scope.** At each start `init` reads the `deliver` root of the owner's `data/effector/policy` (`Policy::deliver`, 750-KRN-0121) and puts a writable scope of it in `effector_gw`'s `SLOT_FILE`; none without the line. The policy's parser refuses a root outside `data/` or in a protected tree.
- **The check** is in the `modes` suite, with `tests/gateway_stub` in the gateway's place, under `deliver data/inbox/`:
  - in the scope a file is written, flushed, renamed and read back;
  - a name and a rename that climb out are refused;
  - restarted by the shell, the gateway gets a new scope, which finds the first one's files, and `vfs_server` ends the first one (`[VFS] SCOPE … ENDED`).

  Passed on x86 and aarch64 in this session's container (QEMU TCG). The criteria name the `isolation` suite; the gateway runs only in a mode, which the `modes` suite boots on both architectures.
- **For EFF:** the gateway's writer names paths relative to the scope's root (`a.txt`, not `data/inbox/a.txt`).

## Related

[750-KRN-0119](750-KRN-0119-boot-images-and-grants-for-effector.md), [750](750-effector-agent.md), `idl/vfs.wit` 2.2 (`scope`), 351-KRN-0040 (a private directory for `keystore`).
