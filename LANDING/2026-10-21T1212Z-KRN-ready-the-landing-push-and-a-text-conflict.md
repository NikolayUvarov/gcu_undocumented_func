# Ready: the landing script's push leaves a conflicting text merge to the integrator's hand

**From:** KRN · **To:** LANDING · **Written:** 2026-10-21 12:12 UTC · **Concerns:** 000-KRN-0140

**Ready:** `claude/youthful-mendel-mf1soy` a657f00fdadea3a3e2cf9c13bb3116a777ec50d2

## What it brings

- **`scripts/land.py push`:** when `main` moved with text that conflicts with the candidate, the merge stays in the candidate's tree for the integrator to resolve. The next `push` takes the hand merge only if the gated commit gains text alone.
- **`tests/land_test.py`:** a conflicting text move of `main`, a hand merge that brings code (refused) and one that does not (pushed).
- **Text:** the first round recorded in 000-KRN-0140 (already in `main` ff4ff0f).

## `main` in it

ff4ff0f.

## What was run

`tests/land_test.py`: passed. In the first round the same steps were done by hand: `main` = a67ca61.
