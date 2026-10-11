# 211-KRN-0044 — The boot log names the branch and commit it was built from

**Type:** kernel (init and the build) · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress (made; the MacBook Pro's run left) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-9.5, MC-12.1

## Problem

The maintainer tests builds of `fast-test` on the MacBook Pro one after another, and every fresh image starts its logs at `LOG:boot0001.log` again. On 2026-10-09 the maintainer asked that the boot log name the commit and the branch it was built from, to tell the logs apart.

- **The commit.** The verified manifest already names it (`commit …`), but the log shows only the manifest's digest (`[INIT] LAUNCH: MANIFEST …`).
- **The branch.** Nothing records it.

## Plan

- **The build.** `02_build.sh` (and through it the aarch64 build) exports `MIND_BUILD_BRANCH` (`git rev-parse --abbrev-ref HEAD`) and `MIND_BUILD_COMMIT` (12 hex digits, with `+CHANGES` when tracked files differ from the commit).
- **init's first line.** init embeds them (`option_env!`, so cargo rebuilds it when they change) and logs `[INIT] BUILD: BRANCH <branch>, COMMIT <commit>`, before the launch record.
- **What does not change.** The signed manifest keeps its format, because the branch is not an input of the build (that is `UPD`'s format).
  - A detached checkout says `HEAD`.
  - `scripts/reproducible.sh` builds both copies from detached worktrees of one commit, so they stay equal.

## Acceptance criteria

- The `normal` suite finds `[INIT] BUILD: BRANCH …, COMMIT …` with a 12-digit commit in init's log.
- The MacBook Pro's `LOG:bootNNNN.log` shows the line with `fast-test` and the commit built.

## Related

[211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md) (the boot logs), [350-UPD-0004](350-UPD-0004-launch-record.md) (the launch record).
