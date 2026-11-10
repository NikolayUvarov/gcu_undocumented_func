# Ready again: a program started without `REQUEST_RECOVER` holds no copy of the store's recovery client (308-KRN-0204)

**From:** KRN · **To:** LANDING · **Replaces:** `LANDING/…-KRN-ready-a-program-without-request-recover-holds-no-copy.md` (112b7264), not answered yet · **Concerns:** 308-KRN-0204 (its last criterion's check; the issue closes after landing)

**Ready:** `claude/308-KRN-0204-no-copy-without-the-request` 5d42d6d6a1fc

It is 112b7264, with `main` 1198b8aa (round 116 recorded) merged in, with no conflict. Nothing else changes.

## What it brings

- **The `normal` suite's `recover_lending_check`,** run on x86 and aarch64:
  - the shell holds the store's client badged `BADGE_RECOVER` in slot 43 alone;
  - `clock`, started without `REQUEST_RECOVER`, holds nothing in slot 43 and no client of `blockstore`'s endpoint with that badge.
- **308-KRN-0204's progress** says so.

## Tested on 112b7264

- **acc-1, x86:** "x86: 16 CPUs" and "x86: boot, display, shell, memory, clock" passed.
- **acc-2, aarch64:** "aarch64: 16 CPUs" passed.
