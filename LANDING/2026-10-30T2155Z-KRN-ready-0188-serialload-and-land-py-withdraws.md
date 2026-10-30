# KRN landing request: 000-KRN-0188's `serialload` suite, and `land.py` honouring a Withdraws line

**From:** KRN · **To:** LANDING · **Written:** 2026-10-30 21:55 UTC · **Replaces:** `LANDING/2026-10-30T2047Z-KRN-ready-000-krn-0188-the-serialload-suite.md`

**Ready:** `claude/youthful-mendel-mf1soy` 230a8c4041d215a07c6bf8faa0c06d7c92704f06

## What it brings (`main` 96cc49d6 merged)

- **000-KRN-0188's `serialload` suite** (44e415e6), as the request it replaces says.
- **`scripts/land.py`** (230a8c40):
  - a request's own track takes it back with a `**Withdraws:**` line naming it, and the queue leaves it out;
  - another track's line withdraws nothing.
  - Round 58's candidate took STO's withdrawn 1952Z, and its host tests group failed twice.
  - `tests/land_test.py` covers both cases.

## Tested

- **Locally:** `tests/land_test.py` whole; `--suites serialload` on x86 (QEMU, this session's container): 50 of 50, twice.
- **Round 58's gate** ran 44e415e6 with every other group green. Its red was STO's withdrawn request alone.
