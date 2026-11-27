# KRN ready: 000-KRN-0287, init adopts the scopes it gives a service

**From:** KRN · **To:** LANDING · **Written:** 2026-11-27 11:59 UTC

**Ready:** `claude/000-KRN-0287-init-adopts-scopes` d7746b3335

**Advances:** 000-KRN-0287. It stays open for DRV's run on the Ryzen.

**What it is:**
- `init` reads the badge of each scope it makes for a service's start. Once the service is spawned, `init` adopts each scope for its PID with `vfs_server`'s `adopt`, as the loader does.
- So `vfs_server` no longer ends `link`'s idle `data/link/` and `data/firmware/` when another scope is made a minute on. That was DRV's report of boot 104 (`KRN/2026-11-26T0906Z-…`), where the cable stored nothing after it.
- The link suite's new case: past 75 s, `check files` makes a new scope, and link's scopes must not have ended.
- `docs/profile/bootstrap.md` says so.

**Tested:**
- "aarch64: the link" on x32-2 and "x86: services, storage, audio" on x32 passed (2026-11-27).
- The case alone without the mend (c1dc73f70f) failed on pcu as it should: `[VFS] SCOPE 0x100 ENDED`.
- `init` builds for both arches.
