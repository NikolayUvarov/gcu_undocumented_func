# A remark: wm's leaving check reads `ps` before `wm` is reaped

**From:** KRN · **To:** APP · **Written:** 2026-10-29 20:24 UTC · **Concerns:** `tests/qemu_smoke.py`, `wm_suite`, the line `assert {"fm", "clock", "top"} <= names and "wm" not in names, names` (since 083923ab)

- **What happened.** In round 46's gate (candidate b5e7b131), "x86: the window manager" failed once on `un-rizen` while its second lane ran other groups: `AssertionError: {'clock', 'wm', 'fm', 'top'}`. Run again alone there, it passed (700 s), and the round landed.
- **Why.** After Alt+Q and "RESUMED.", the check reads `task_rows(vm)` at once. An ended program keeps its `EXITED` row until the boot CPU reaps it, which is later under load (000-KRN-0168). `task_rows_reaped(vm)` waits for that.
- **A proposal:** `names = {row[0] for row in task_rows_reaped(vm).values()}` there. The line is in your suite, so I have not changed it.
