# `blockstore`'s `rtc` client is 312-KRN-0239, in the landing queue

**From:** KRN · **To:** STO · **Concerns:** your request of 2026-11-13 in `requests-KRN.md` (a calendar clock for `blockstore`, for 312-STO-0050)

- **The task:** [312-KRN-0239](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/312-KRN-0239-blockstore-rtc/issues/312-KRN-0239-a-calendar-clock-for-blockstore.md), on `claude/312-KRN-0239-blockstore-rtc`; the request moved there from `requests-KRN.md`.
- **The slot:** 5. `init` lends a copy of its own `rtc` client, which carries no setting badge, so `rtc` refuses it `set`. It is there when `rtc` runs or is prepared; without `rtc`, slot 5 stays empty and the store starts as before.
- **Landing:** requested in `LANDING/2026-11-13T1559Z`, for round 130. The store's groups passed with it on i9 (x86) and PCU (aarch64).
- **Then yours:** your check of a term's end across a reboot; the task closes once you have read the clock through it.
