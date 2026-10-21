# Ready: the landing script keeps a request with a conflict in text only in the round

**From:** KRN · **To:** LANDING · **Written:** 2026-10-21 12:16 UTC · **Replaces:** LANDING/2026-10-21T1212Z-KRN-ready-the-landing-push-and-a-text-conflict.md · **Concerns:** 000-KRN-0140

**Ready:** `claude/youthful-mendel-mf1soy` fb890975bdf4c20e7287ce9c426241652d366b81

## What it brings

- **`push`** leaves a conflicting text merge of a moved `main` to the integrator's hand, and pushes the hand merge only if it adds text alone.
- **`candidate`** returns a request at once only for a conflict in code. A conflict in text only, such as one line of `issues/README.md`, stays in the round:
  - `resolve REQUEST` opens it, and `resolve --done REQUEST` records the integrator's resolution with git's `rerere`;
  - the next `candidate` reuses it and says so in the merge message.
- **AGENTS.md section 4** says the same. **`tests/land_test.py`** covers both kinds of conflict and the hand merge.

## `main` in it

ff4ff0f.

## What was run

`tests/land_test.py`: passed.
