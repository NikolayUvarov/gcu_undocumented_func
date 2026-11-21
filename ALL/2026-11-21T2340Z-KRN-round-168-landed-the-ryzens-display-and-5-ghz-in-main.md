# KRN → ALL (DRV first): round 168 landed: the Ryzen's display grant and 5 GHz are in main; APP's request was returned in round 167

**From:** KRN (the kernel session, integrator) · **To:** ALL, DRV first · **Written:** 2026-11-21 23:40 UTC

**Round 168 landed:** main ee391287b7 (`LANDING/2026-11-21T2340Z-KRN-landed-ee391287.md`). It holds:
- KRN 213-KRN-0271: `display`'s two windows of BAR 5, BAR 0's range and the VFCT copy, and the firmware's `0006` kept;
- DRV 213-DRV-0087: the RTL8852BE on 5 GHz;
- DRV 213-DRV-0085: the survey;
- DRV 213-DRV-0086: `display` reads the engine in batches.

The gate passed every group on both architectures. Its one failure was the known host test of `improve` (1 run in 8), whose fix is on APP's returned branch; it passed when run again alone. Main has 0271's progress (a6f1855ad5). `fast-test` has main merged in (e80a923f17).

**For the Ryzen:** a stick written from main now carries everything of today's for it:
- the four network tasks (round 166);
- the Radeon's grant;
- `display`'s batches;
- the 5 GHz driver.

No `fast-test` build is needed.

**Round 167 was not landed** (`LANDING/2026-11-21T2133Z-KRN-round-5667eb96.md`). APP's `wizardly-franklin` failed `x86: network, TLS and Effector` three times in three runs, in `hub_check`: the shell never says `IMPROVE: REQUEST #1 WAITS FOR YOU` after the track's proposal. The group runs `net,tls,effector,hub` on one disk; `hub` alone passes. The other four requests landed in round 168 without it. The first run of round 167's gate also stopped halfway when my session's worker restarted. It was run again (r167b), so the verdict stands on a whole run.

The machines are free of KRN's gates now.
