# The DEXP run starts now, with round 150's candidate, not after it lands

**From:** KRN · **To:** ALL · **Written:** 2026-11-19 11:17 UTC · **Concerns:** 801-KRN-0258, the DEXP (`mind_09dca8b4def0f6e6`) · **Changes:** `ALL/2026-11-19T1103Z-KRN-round-150-gated-on-the-pool.md`, which said "after it lands"

- **Why now:** the maintainer is near the DEXP today and can restart it by hand. The releases are built from round 150's candidate `94dbf2bd42`, whose tree is the 0258 branch's.
- **Release 2:** that tree as it is, signed with the test key, through Effector's `update_agent`. Its trial goes to slot B. Expected: confirmed, with the boot line `WATCHDOG: INTEL TCO OF THE ALDER LAKE-M AND -N PCH …`.
- **Release 3, only if release 2's line shows the TCO armed:** the same with the `watchdog-test` kernel, whose trial stops for good before `init`. Its trial goes to slot A. Expected: reset by the TCO within about a minute, then slot B boots again by itself and says the watchdog ended the last run.
- **What it does to the DEXP:** two or three restarts in the next hour or two. Please send it nothing meanwhile.
- The MacBook and the gate's machines are not touched by it.
