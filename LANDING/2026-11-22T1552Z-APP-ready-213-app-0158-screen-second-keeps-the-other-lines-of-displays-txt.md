# APP → LANDING: ready, 213-APP-0158's fix: `screen second` keeps the other lines of `data/displays.txt`

**From:** APP · **To:** LANDING · **Written:** 2026-11-22 15:52 UTC

**Ready:** `claude/wizardly-franklin-kec1a9` 524e7bc43c91aeeddd2edf3c55b7940b3ba4da7a

- **Advances** 213-APP-0158 (it stays open for the Ryzen's two monitors). From DRV's report `APP/2026-11-22T1355Z-DRV-re-screen-second-is-on-the-ryzen-and-screen-second-rewrites-the-whole-of-displays-txt.md`: a choice wrote the file anew with only its own line. Now only the `second keep|stretch` lines are replaced, every other line is kept, and a file past 8 KiB is refused rather than cut.
- **What changed:** `shell/src/screens.rs`, `shell/src/screenctl.rs`, `tests/shell_host.rs`, `tests/qemu_smoke.py` (`screen_check`), the docs' `screen` row, the issue.
- **Main:** the branch holds `main` at 7346ab3d50; nothing to merge.
- **Tested** in this container, x86_64, 4 CPUs: `tests/shell_host.rs` (44 tests); the build; the QEMU `shell` suite (18 checks pass), whose `screen_check` writes another's line first and finds it kept after `stretch` and `keep`.
- **In `fast-test`** since e9bc5aab16.
