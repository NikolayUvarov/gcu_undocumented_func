# Ready: 800-LNK-0033, each try waits twice as long as the one before, so a machine busy writing its disk ends no put

**From:** LNK · **To:** LANDING · **Written:** 2026-10-31 07:23 UTC · **Concerns:** 800-LNK-0033 (advanced; it closes after the next slot updates over the cable)

**Ready:** `claude/LNK-link` 8ddc98ff9a78671185afbc7885443f6fbbf225f0

## What it brings

On `main` 8b1e00d0, merged.

- **a454c709, `linkproto::Master::exchange`:** each try waits twice as long as the one before, up to 4 s, or the timeout when that is longer.
  - At 460800 bit/s, 8 tries now ride out about 20 s of silence, not 2.3 s.
  - **Why:** `link` on MIND Core is silent while it flushes its USB stick (`PUT_CLOSE`, and each `PUT_DATA` that fills its 16 KiB buffer). Two DEXP slot updates lost a put that way.
  - **Tests:** a new host test (a 6 s silent commit kept, a 60 s one given up on); the dead line's test takes the new waits.
  - `docs/link/mindlink.md` (EN, RU).
- **8ddc98ff:** the issue's record.

**Shared code:** `linkproto` is used by `mindlink` and `effector485` on the host and by `link` on MIND Core. The slave's code is unchanged.

## Tested, on LEG9 (WSL2, Ubuntu 24.04)

- `linkproto` (24) and `tools/mindlink`'s tests, before and after merging `main`.
- **Builds:** `mindlink` and `effector485` for Linux and Windows, and the x86 image.
- **The QEMU `link` suite on x86:** PASS, on a454c709 and again on the tree with `main` merged.
- **On the DEXP,** through the new `effector485` (now the maintainer's daemon): 24 puts of 8.4 MB, all completed, with no lost answer.
- **Not run:** the aarch64 `link` suite on this change. It passed yesterday on f52fca85, which differs only in `linkproto`'s master.
