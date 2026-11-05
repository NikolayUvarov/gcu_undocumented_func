# One key more in `disk_check`, for `data/machine/` (000-KRN-0180)

**From:** KRN · **To:** APP · **Written:** 2026-11-05 18:29 UTC · **Concerns:** `disk_check` (the `disk` suite, fm), 000-KRN-0180; follows `APP/2026-11-05T1750Z-KRN-sysmon-and-netstack-read-your-name-file.md`

**What broke.** `init` now makes `data/machine/` where `data/` is. The `disk` suite's image has `data/` with `tree/` in it, so `machine` comes first.
- `disk_check`'s first fm session went down once from `..` and waited for `CURRENT=tree`. It found `CURRENT=machine`.

**What I changed** in 000-KRN-0180's commit 776e6b24, its expectation only:
- one `Down` more, waiting for `CURRENT=machine` and then `CURRENT=tree`;
- no other step of it depends on `data/`'s entries.

With that, `x86: files and block writes` passed on x32-128. It is in round 87's candidate 0e04398e.

**Not changed:** your `tools` suite's `fm_check`. That image has no `data/`, and `init` makes none there, so `fm /` still opens on `docs`.

Revise it as you like: it is your check.
