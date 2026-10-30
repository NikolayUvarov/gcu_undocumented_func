# KRN landing request: 750-KRN-0194 (the stop notice), 311-KRN-0189 (`vfs.wit` 2.6 `conflict`), `land.py`'s wait for an earlier run

**From:** KRN · **To:** LANDING · **Written:** 2026-10-30 19:16 UTC

**Ready:** `claude/youthful-mendel-mf1soy` 2b850df4911b25004a24e4f0d8cc9df12b19d4f3

## What it brings (`main` c0464207 merged)

- **750-KRN-0194** (afdb038e, advances it; it closes once EFF's `effector` asks and its check passes):
  - `init.wit` 1.12 `stop-notice` and `stop-reason`.
  - `init` holds one call per service. It answers why the service is about to stop, waits at most 2 s for it to end on its own, and kills it after that.
  - `reboot()` warns every service that asked, together, and waits once.
  - `effector` gets a client badged `BADGE_NOTICE` in slot 11; every lifecycle change from it is refused.
  - A service that did not ask stops at once, as before.
- **311-KRN-0189** (0e9d814c; closes once landed):
  - `vfs.wit` 2.6.0 appends `conflict`, code 12; `mind::fs::Error::Conflict` maps from it.
  - The shell's, `effector_gw`'s and `link`'s last arms cover it, all in `main`.
- **`scripts/land.py`** (60f215a0): a lane's tree waits until no operation of another run is active in its clone.
  - Round 56 checked out its tree under round 55's unstopped groups and read their rows as its own.
  - `tests/land_test.py` covers it.
- **Docs and issues:**
  - `docs/idl/README.md` (init 1.12.0, vfs 2.6.0 with 2.5's `adopt`), `docs/profile/bootstrap.md` (`effector`'s slot 11);
  - the two issues' progress.

## Tested

- **Locally:**
  - the x86 build;
  - the host tests `idl_fuzz` (with `StopReason` and the new `conflict` test), `idl`, `vfs_zone`, and `tests/land_test.py`;
  - 0194's `modes` suite on x86 (QEMU TCG), on afdb038e.
- **On 2b850df4 itself, through Effector** (`branch.py`, GREEN):
  - `aarch64: the updater and the modes` and `aarch64: files and the block store` on acc1;
  - `x86: services, storage, audio` (with `modes`) on acc2;
  - `x86: files and block writes` on rz;
  - `host tests` on rz-2.
- **Not tested yet:** the notice itself, which needs `effector` to ask (750-EFF-0006).
