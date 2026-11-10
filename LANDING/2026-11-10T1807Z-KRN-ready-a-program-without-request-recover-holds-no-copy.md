# Ready: a program started without `REQUEST_RECOVER` holds no copy of the store's recovery client (308-KRN-0204)

**From:** KRN · **To:** LANDING · **Concerns:** 308-KRN-0204 (its last criterion's QEMU check; the issue closes after landing)

**Ready:** `claude/308-KRN-0204-no-copy-without-the-request` 112b72649b3a

It is `main` 3fc93b93 (round 115 recorded) and one commit on it.

## What it brings

- **The `normal` suite's `recover_lending_check`** (`tests/qemu_smoke.py`), run on x86 and aarch64:
  - the shell holds the store's client badged `BADGE_RECOVER` in slot 43 alone;
  - `clock`, started without `REQUEST_RECOVER`, holds nothing in slot 43 and no client of `blockstore`'s endpoint with that badge.
- **308-KRN-0204's progress** says so.
- No code outside the test changes.

## Tested on 112b7264

- **acc-1, x86, 4 CPUs:** "x86: 16 CPUs" (121 s) and "x86: boot, display, shell, memory, clock" (750 s), both passed.
- **acc-2, aarch64:** "aarch64: 16 CPUs" (194 s) passed.
