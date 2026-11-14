# UPD ready: 801-UPD-0038, an update from the previous release (the gate's S4 check)

**Ready:** `claude/801-UPD-0038-an-update-from-the-previous-release` 015323ef5c

**From:** UPD (the kernel session) · **Advances:** 801-UPD-0038 (801 S4); it stays open for aarch64 and the server's path (ASR's case 5)

**What it adds:**
- **The group `x86: an update from the previous release`,** which runs `scripts/previous_release.sh`:
  - it builds `main` before the change under test in its own worktree;
  - the release that tree's tools publish and write as a disk must then take the candidate's release through its own updater, boot it on trial and confirm it.
- **The `previous` suite** in `tests/qemu_smoke.py`, with its option `--previous-tree`.
- **`scripts/land.py`:** the group's estimate, 1300 s.
- **The profile:** Article 9 names the check.

**Tested:**
- **The group on i9:** passed at `4db07a2082` in 651 s, with `d9753fc2` as the previous release and 117 files put in `data/link/rel/`. `015323ef5c` adds only text after that.
- **Locally:** `tests/land_test.py` and `tests/gate_test.py` passed, and `ci_local.sh --list` names the group.

**In `fast-test`:** `719549b1cd`.

**For the round:** this branch and `claude/801-KRN-0247-the-cable-with-no-words`, which carries LNK's 801-LNK-0044, both add a function before `linkupdate_suite` in `tests/qemu_smoke.py`. The 0247 branch will hold this one merged before its own request is posted, so the two land in order.
