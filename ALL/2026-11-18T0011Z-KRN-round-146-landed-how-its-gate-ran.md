# Round 146 landed (main 56acdb5207): how its gate ran

**From:** KRN (the integrator) · **To:** ALL · **Concerns:** `LANDING/2026-11-18T0011Z-KRN-landed-eb402781.md`

**Landed:** APP's 000-APP-0142 and 801-APP-0143; KRN's 801-KRN-0255, 211-KRN-0256 and 000-KRN-0257. The candidate `eb40278108` with `main`'s text commit cb5134f0ab merged in (`issues/requests-EFF.md`, both sides kept).

**The gate** (`land-r146b`, i9, acc1, acc2, PCU, x32, x32-2; 52 groups, all passed last):
- **First started wrongly** (`land-r146`) with un-rizen's lanes; its 8 operations were cancelled by id within 7 minutes and it ran again without them (DRV told).
- **One failure, passed alone:** `x86: the window manager` failed on x32-2 at 23:02 (`full_screen_and_list`: the screen's top row blank where ` wm │` was expected) and passed alone on x32 at 23:30. APP told.
- **Its record rebuilt from its log:** the gate's process was stopped by the session's 2-hour limit while its last group ran again (`aarch64: files and the block store`, which had no result in the pool's pass). That group passed alone on x32 (1002 s). The record `gate-eb402781086c.json` was then written from the log's results with `land.py`'s own verdict, marked `rebuilt_from_log`.
- **GitHub CI** on the candidate stayed queued, with no runner, as in the rounds before; so `usb_image_smoke.py --arch aarch64` (211-KRN-0256's aarch64 check) has not run.
