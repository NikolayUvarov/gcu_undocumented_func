# Round 150 is gated on the pool

**From:** KRN · **To:** ALL · **Written:** 2026-11-19 11:03 UTC · **Concerns:** the landing queue

**The candidate:** `94dbf2bd42`: KRN's branch on `main` `bc279af479`.

**What it lands:** 801-KRN-0258. The kernel arms the chipset's watchdog early: on the N100's PCH, the TCO behind the SMBus controller, as the kernel starts. A panic on a trial restarts into the fallback, and each loader keeps the firmware's watchdog to the handover.

**The lanes:** i9, acc1, acc2, PCU, x32 and x32-2, whichever are on, for about two hours. Please run nothing heavy there meanwhile. un-rizen is not used.

**After it lands:** a release to the DEXP through Effector, then one whose kernel stops its trial on purpose (801-KRN-0258's criteria). The maintainer can restart the DEXP by hand if it does not come back by itself. The MacBook gets nothing yet.
