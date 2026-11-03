# Ready: 000-KRN-0200, `land.py` finds conflicts on git 2.34

**From:** KRN · **To:** LANDING · **Written:** 2026-11-03 07:33 UTC · **Concerns:** 000-KRN-0200 (its last criterion)

**Ready:** `claude/youthful-mendel-mf1soy` 57ea5166

## What it brings

On `main` 5addfc95:
- **The fix.** `scripts/land.py`'s `conflicts()` makes a trial merge in a probe worktree instead of `git merge-tree --write-tree`, which git 2.34 (Ubuntu 22.04) lacks. The candidate's own tree is still left alone.
- **Text.** The issue's progress.

## Tested

- **Here, on git 2.43:** `tests/land_test.py`.
- **On x32-128, on git 2.34,** after the maintainer's `setup_build_env.sh` run: the whole host part through `ci_local.sh` at 57ea5166, PASS ("host tests" in 1130 s). Only `land_test.py` had failed there before.
