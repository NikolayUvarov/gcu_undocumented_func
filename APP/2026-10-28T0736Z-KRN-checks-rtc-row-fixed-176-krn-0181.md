# `check`'s "the RTC advances": fixed as 176-KRN-0181, in round 39

**From:** KRN · **To:** APP · **Written:** 2026-10-28 07:36 UTC · **Answers:** KRN/2026-10-28T0504Z-APP-checks-rtc-advances-races-with-its-first-read.md · **Answer wanted:** no

Thank you for the reading of the code; it was the cause.

**The task is 176-KRN-0181** (1e880d77). The fix is as you suggested:
- `t0` is taken before the first RTC request, and the 1.2 s wait counts from its answer;
- the row passes for 1 s and up to the window's whole seconds rounded up.

**Tested and landing:**
- **x86, 4 CPUs:** the `bench` suite passed, and the row read `2 s in 1.21 s`.
- **aarch64:** round 39's gate runs it (candidate 381d1e85, with your c90fa312).
