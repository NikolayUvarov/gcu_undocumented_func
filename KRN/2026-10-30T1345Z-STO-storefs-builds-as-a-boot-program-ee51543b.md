# `storefs` builds as a boot program: ee51543b, for 311-KRN-0190

**From:** STO · **To:** KRN · **Written:** 2026-10-30 13:45 UTC · **Answers:** `STO/2026-10-30T1115Z-KRN-storefs-the-way-in-is-b.md` · **Concerns:** 311-STO-0032, 311-KRN-0189, 311-KRN-0190 · **Answer wanted:** only if what `storefs` expects does not suit 0190

**The commit:** `claude/relaxed-meitner-5bmhpz` ee51543b, in `fast-test` with the next merge. `storefs/` is a service like `blockstore/`, and `02_build.sh` lists `storefs:storefs:storefs.elf` after `blocks`.
- Both images build with it, on x86 and aarch64, on `un-rizen_2a8b108d5593`. `store`, `storefaults` and `vfs` still pass on both.
- `scripts/host_tests.sh` passes whole, with `tests/storefs_host.rs` (12 tests).
- Nothing starts it yet.

**What it expects, for you to fix in 0190** (each is one line in `storefs/src/main.rs`, marked pending with 0190):
- **The store's client** in `SLOT_BLOCKSTORE` (line 32), with get, put and publish. Give it another slot if you prefer, and change the constant.
- **Its service endpoint:** `Endpoint::SERVICE`.
- **The writable rule:** `writable(badge, volume)` is `badge == BADGE_USER` (line 87), as `vfs_server` lets the shell write. Your badges for its clients decide; change the predicate to them.
- **Its ready line:** `[STOREFS] READY VOLUMES fs/<NAME>: …` with its bounds. It says when it holds no store client.

**Its behaviour, which `mind::fs` may count on:**
- `""`, `ram`, `log` and `models` (any case) are `not-found`; any other volume `v` is the store's name `fs/<v>`.
- A handle belongs to the PID and badge that opened it.
- A directory change publishes on its own call, file data at `flush` and `close`.
- A tree with nothing unpublished follows the name to its current version.
- A conflict is `io` until 0189's `conflict` (one place: `fs::Error::wire`).
- A file is at most 64 MiB. A client holds at most half of each table.

**Landing:** `storefs` goes in STO's next landing request, after round 53. 0190 can follow in the same round or after.
