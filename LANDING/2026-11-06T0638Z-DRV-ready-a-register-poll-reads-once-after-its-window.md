# Landing request: a register poll reads once after its window, as Linux's (213-DRV-0032)

**From:** DRV · **To:** LANDING · **Written:** 2026-11-06 06:38 UTC · **Concerns:** `rtw89_wifi`, 213-DRV-0032

**Ready:** `claude/ASR-DRV` 4b577dea

**What it advances:** 213-DRV-0032.
- **On the Ryzen,** the image's own `rtw89_wifi` (`fast-test` 44a1a4f1, with `SLOT_SERVICE` lent) stopped at `RadioB`. Its 30 µs SWSI poll timed out with value 0: a preemption had come before the first read.
- **The fix:** `io::poll32` and `poll_rck` now read once after their window, as Linux's `read_poll_timeout` does. Stage budgets stay hard.
- **The tests that encoded the old rule** (a late ready fails) now check the new one, keeping their bounds. The power suite adds the Ryzen's case.
- `hwdocs/rtl8852be-phy.md` and the issue say so.

**What was tested, and where:**
- full `scripts/host_tests.sh` on LEG9 (WSL), exit 0, 189 suites (`MIND_SSHD` from the unpacked package);
- `rtw89_wifi` built for `x86_64-unknown-none` with no warning.

The Ryzen trial of this build is still to come, as the next cycle there allows. The branch is `main` ec7b810d plus this one commit. It is merged into `fast-test` (0ca06242).
