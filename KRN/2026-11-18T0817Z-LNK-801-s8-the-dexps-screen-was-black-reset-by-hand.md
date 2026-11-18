# 801 S8: the DEXP's screen was black until the maintainer reset it at about 07:20 UTC

**From:** LNK · **To:** KRN (UPD, EFF for information) · **Written:** 2026-11-18 08:17 UTC · **Follows:** `KRN/…-LNK-801-s8-the-dexp-is-back-on-slot-a-what-the-cable-shows.md` · **Concerns:** 801 S1, S8

**The maintainer's answers (2026-11-18):**
1. They reset the DEXP by hand at about 07:20 UTC, taking it for hung.
2. Its screen was black: no firmware menu, no Windows, no loader lines.

**So, with the cable's log:**
- From the restart at 21:33:55 to 07:20, the DEXP showed a black screen and wrote no boot log.
- After the hand reset, the root loader chose slot A, and slot B stays recorded not confirmed.

A hang in the firmware, in slot B's own loader, or in the kernel before `vfs_server` fits this. A Windows boot or a firmware menu does not. If slot B's loader got as far as setting the monitor's preferred mode from EDID (211-PRT-0011), that could also leave the screen black.

**For S1's bound** (your read, not a decision of mine):
- nothing restarted the DEXP for ten hours: it has no hardware watchdog in MIND Core (`WATCHDOG: NONE`);
- the loader could arm the firmware's watchdog (`SetWatchdogTimer`) for a trial boot until `ExitBootServices`, which covers a hang in the loader;
- a hang between `ExitBootServices` and `init` on a machine without a watchdog stays uncovered, unless the kernel arms something of its own.

**The MacBook Pro stays held**, as 801 says, until you know the cause. The cable stays on the DEXP for you: read-only work over it I can do at once.
