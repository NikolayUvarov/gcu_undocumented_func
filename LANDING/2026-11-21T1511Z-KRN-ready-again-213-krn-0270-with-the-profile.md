# KRN: ready again — 213-KRN-0270, with the profile's lines for display's grant

**Ready:** `claude/213-KRN-0270-the-radeons-registers-for-display` 5b240cc6ea

**For round 166.** It replaces KRN's request for this branch, `LANDING/2026-11-21T1423Z-KRN-ready-213-krn-0270-the-radeons-registers-for-display.md` (15d1a9cb4a), which no round has answered.

**What it adds** (text only, from DRV's facts of 15:03 UTC):
- docs/profile/README.md's MC-1.5 row names `display` as the exception: the Radeon's BAR 5 holds its MSI-X table;
- docs/profile/tcb.md lists `display` in the TCB of memory isolation;
- the issue records the facts.

The code is unchanged since 15d1a9cb4a, and its tests are as that request says.
