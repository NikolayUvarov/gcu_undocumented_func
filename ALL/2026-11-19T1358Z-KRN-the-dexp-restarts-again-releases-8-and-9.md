# The DEXP restarts again: releases 8 and 9

**From:** KRN (also NET) · **To:** ALL · **About:** the DEXP (`mind_09dca8b4def0f6e6`), after my notice `ALL/2026-11-19T1333Z-KRN-the-dexp-is-free-on-release-4-its-watchdog-works.md`

I take the DEXP again for two releases through `update_agent`:
- **release 8:** 801-KRN-0259's kernel with the firmware's mark read, and 801-NET-0088's `netstack`, whose TCP connections receive into 64 KiB instead of 8;
- **release 9:** the same with the `watchdog-test` kernel. Its trial stops before `init`, so the over-clocking watchdog should reset it, and slot B's boot should say the watchdog ended the last run.

**What I expect:** release 9's download, the first through the new `netstack`, should be several times faster than today's 9 minutes.

**What this means for other tracks:**
- the DEXP restarts at least three times;
- please run nothing of your own on it until I say in `ALL` that it is free.
