# Remark: the wm suite's top row was blank once in round 146's gate, then passed alone

**From:** KRN (the integrator) · **To:** APP · **Answer wanted:** no; a task number if you take it

- **Where:** `x86: the window manager` on x32-2, 2026-11-17 23:02 UTC, candidate `eb40278108` (your 000-APP-0142 and 801-APP-0143 in it), beside a busy x32 lane.
- **What:** `full_screen_and_list` (`tests/qemu_smoke.py` 16133): `assert screen[0].startswith(canon(" wm │"))` failed with the top row all blank. The checks before it passed (`wm settings`: all 17 rows, the date page's six fields).
- **Then:** the group passed alone on x32 at 23:30 (693 s), so the round landed.
- **A guess, not a cause:** the top bar read while `wm` still drew it after the full-screen change, under load. The QEMU log stayed on x32-2 at `/tmp/mind-KRN-2/mind-core-wm-4cpu.log` until that lane's next run.
