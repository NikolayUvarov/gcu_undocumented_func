# 801-UPD-0038 — An update from the previous release: no release lands that the one before it cannot update to

**Type:** update (tests, the gate) · **Owner:** `UPD` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S4) · **Roadmap:** stage V · **Constitution:** MC-9.3, MC-9.4, MC-12.1

## Problem

801 S4: «Не переходный релиз. А если обновление не может подключиться к сети и/или серверу - то это обновление не рабочее». The gate's check updates the previous round's release to the new one, and names a change that stops it.

**What the bench showed:** the MacBook Pro's updater (`main` 20fba2c5) could not take `main` past 351-UPD-0026, the new signature lines. Only a disk rewrite got it there.

**Why nothing caught it:** every suite built both releases of an update from the one tree under test, so a format the previous updater cannot read never showed.

## Plan

- **`scripts/previous_release.sh`**, the CI group `x86: an update from the previous release`:
  - It takes `main` before this tree's change: the merge base with `origin/main`, or for `main` itself the commit before.
  - It builds that in a worktree of its own (`$TMPDIR/mind-previous-<commit>`, kept while the base is the same) with that tree's `./02_build.sh`.
  - Then it runs the `previous` suite.
- **The `previous` suite** (`tests/qemu_smoke.py --previous-tree`):
  - **Release 5, by the previous tree's own tools:** signed (`sign_manifest.py --version 5`), staged and published (`release.py`), and written as a maintainer writes a disk (`make_usb_image.py --slots`: slot A confirmed, its channel and key file kept). Its `update.txt` names the cable's directory with `automatic apply`.
  - **Release 6, by this tree's tools:** this tree's build, signed as version 6 and published by this tree's release tool into the same server.
  - **The cable's way in:** release 6 is put into the disk's `data/link/rel/` as the cable puts it.
  - **The check:** the previous release's own updater must take release 6 into slot B, boot it on trial and confirm it. Slot B then holds release 6's manifest.
- **In the gate:** the group is one of `scripts/ci_local.sh --list`, so every round's candidate is updated from `main` before the round. A change that stops it fails the gate there, with the group's log naming the refusal.
- **Not yet:**
  - aarch64;
  - the update through the Effector server (ASR's 801-ASR-0014, case 5);
  - a release that cannot connect, which is S1's (801-KRN-0242).

## Acceptance criteria

1. **On x86,** the group passes on a branch whose base is `main`: the previous tree is built, and its updater takes this tree's release and confirms it.
2. **The group is in the gate's list,** with its time estimated in `scripts/land.py`.
3. **The profile** (Article 9) names the check as evidence, and what it does not cover.

## Related

[801](801-the-bench-stays-manageable.md), [801-ASR-0014](801-ASR-0014-the-stages-conditions-as-checks.md), [801-UPD-0039](801-UPD-0039-the-updater-always-reads-the-cables-directory.md), 351-UPD-0026, 351-KRN-0141.
