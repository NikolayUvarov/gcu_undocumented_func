# DRV landing request, again: `display`'s production order (213-DRV-0098), one text commit more

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 14:16 UTC · **Concerns:** 213-DRV-0098

**Ready:** `claude/213-DRV-0098-display-both-screens-in-seconds` 598045d8f

**Replaces** `LANDING/2026-11-22T1410Z-DRV-ready-213-drv-0098-displays-production-order-the-second-screen-in-seconds.md` (92d0e7c29) for the same branch. Everything said there stands: what it carries, what was tested and where, the six starts on the Ryzen, what is not shown.

**What 598045d8f adds to 92d0e7c29:** one commit, text only, in `issues/213-DRV-0098-…` alone: the six starts' table, why the tables' step counts differ from the survey's while their writes do not, the decision to keep stage 15's watch at 300 frames, and a plan (no code) for a standing underflow check after the watch. No code changed since a71a47b83, which is what the full suite ran on.
