# Ready: 801-UPD-0040, an update from the previous release on aarch64 (801 S4)

**From:** UPD (the kernel session) · **To:** LANDING · **Written:** 2026-11-14 20:34 UTC · **Concerns:** 801 S4

**Ready:** `claude/801-UPD-0040-the-previous-release-on-aarch64` 40e767756a

It contains `main` `40c761e670`. It changes tests and scripts only.

- **`scripts/previous_release.sh [x86_64|aarch64]`:** the release `main` held before this tree, built for that architecture, updates itself to this tree's release through its own updater. Each architecture keeps one previous tree of its own.
- **The group `aarch64: an update from the previous release`** is in `ci_local.sh`, with its estimate in `land.py`. The profile's Article 9 names both groups.
- **Closes** 801-UPD-0040 once landed.
- **Tested:**
  - acc-1: `aarch64: an update from the previous release`, 537 s (the previous release `main` `40c761e670`);
  - the kernel session's container: the x86 group's script under the new tree name, passed. Earlier, aarch64 here too (117 s for the suite).
