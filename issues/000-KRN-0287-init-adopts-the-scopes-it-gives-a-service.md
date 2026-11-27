# 000-KRN-0287 — `init` adopts the scopes it gives a service

**Type:** `init` (no ABI change) · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (made on `claude/000-KRN-0287-init-adopts-scopes`) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-3.1, MC-4.7, MC-12.3

## Problem

DRV reported this, also for BLT and LNK, in `KRN/2026-11-26T0906Z-DRV-…`. It happened on the Ryzen, in boot 104, on release 202611260856, with `main`'s `init` and `vfs_server`.
- At 91.7 s `bt_hci` was ended, and `init` started it again with its scope of `data/bluetooth/` (700-KRN-0280).
- In the same millisecond `vfs_server` ended scopes 0x100 and 0x101. These were `link`'s `data/link/` and `data/firmware/`, made at 2.5 s.
- From then on, the cable's `put` and `list` were refused with `code 3` until `link` was started again.

The reason, in the code:
- A scope's user is set at its first use, or by `adopt`. Only the loader sent `adopt` (000-KRN-0170), for a program's own directory.
- `sweep_scopes` runs whenever a scope is made. It ends every scope that has no user and is older than `SCOPE_UNUSED_MS` (60 s).
- `link` opens `data/link/` only when a storage request comes over the cable.

So any scope made a minute or more after `link` started ended `link`'s two, if no storage request had come yet. A restarted scoped service or a program the loader gives a scope is enough.

## Plan (made)

- **`init` adopts the scopes it makes.** In `init/src/main.rs`, `start` reads each scope's badge with `cap_info` while it still holds the scope. Once the service is spawned, `adopt` names the service's PID to `vfs_server` for each scope, through a client of its own badged `BADGE_READER`. `vfs_server` takes `adopt` only from a scope's maker, and `init` is the maker of these. A scope that could not be adopted is logged: `[INIT] <name>: SCOPE <badge> NOT KEPT FOR PID <pid>`.
- **The QEMU case** is in `tests/qemu_smoke.py`'s link suite, before its first `put`:
  - link's two scopes are found in `vfs_server`'s log;
  - the suite waits until the machine is up 75 s;
  - it runs `check files`, whose own directory the loader gives as a new scope;
  - neither of link's scopes may have ended, and the `put` that follows must work.

  A first form ended `bt_hci` from the shell instead, but in the link suite's machine the shell found no `bt_hci` to end.
- **The record.** `docs/profile/bootstrap.md` says that a service's scopes are adopted for its PID.

## Acceptance criteria

- [x] `init` builds for x86 and aarch64.
- [x] The link suite passes with the new case on both arches (2026-11-27, QEMU): "aarch64: the link" on x32-2, and "x86: services, storage, audio" on x32, which runs the x86 link suite.
- [x] Without the change, the case fails. On `claude/000-KRN-0287-case-without-the-mend` (c1dc73f70f, `main` with the case alone), "aarch64: the link" on pcu failed at its check: `[VFS] SCOPE 0x100 ENDED` once `check`'s scope was made a minute on. That is DRV's bug of boot 104, seen in QEMU.
- [ ] The gate's groups pass with it.
- [ ] On the Ryzen, a restart of a scoped service a minute or more after the boot leaves the cable's `put` working. This is DRV's run.

## Related

- 000-KRN-0170: the loader's `adopt`.
- 750-KRN-0120: the scopes `init` gives its services.
- 700-KRN-0280: `bt_hci`'s scope.
- 801: the cable as the machine's second way in.
