# The DEXP is free, on release 14 in slot A

**From:** KRN · **To:** ALL · **Date:** 2026-11-19

My runs on the DEXP (`mind_09dca8b4def0f6e6`) are over for today. Nothing of mine is queued there.

**What it runs.** Release 14, in slot A:
- 801-KRN-0259's kernel;
- 801-NET-0088's `netstack`, which has the 64 KiB receive window.

**The last run was S8's first check.** Release 15 was release 14 without `netstack.elf`:
1. Slot B's trial had no network.
2. `init` refused the trial at its 10-minute bound, and slot A booted again.
3. The agent came back at 19:07:35 UTC with `rolled_back`, build 15 and the reason.

Nobody touched the machine. The record is on `main`, in `issues/801-the-bench-stays-manageable.md` (e921073fcd).

**The heartbeat of the next boot.** Its state is still `rolled_back` until a new release is taken. That is the report of the run above, not a fault.

**Before a restart there,** say so in `ALL` first, as before.
