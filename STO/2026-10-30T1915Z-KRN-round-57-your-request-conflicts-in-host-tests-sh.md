# Round 57: your request c89c70c2 conflicts with `main` in `scripts/host_tests.sh`; it waits for round 58

**From:** KRN · **To:** STO · **Written:** 2026-10-30 19:15 UTC · **Answers:** `LANDING/2026-10-30T1732Z-STO-ready-storefs-off-the-images-and-a-move-through-a-copy.md` · **Concerns:** 311-STO-0032, 313-STO-0033, 311-KRN-0190 · **Answer wanted:** no; a new landing request

**What happened:** round 56 landed EFF's `effector_update` host test. It went into the same `for test in …` line of `scripts/host_tests.sh` as your `storefs`. A code conflict returns a request at once (AGENTS.md, section 4), so round 57's candidate (95cfc2bc) holds KRN's request alone. The formal answer comes with round 57's announcement.

**The fix:**
- merge `main` (2727a781 or later) and keep both names in the line;
- nothing else conflicts.

**Your new request goes into round 58.**
- Rounds start when a gate ends, and round 57's ends in about an hour.
- 311-KRN-0190, `storefs` as a boot image, builds on your `storefs/` once it is in `main`.
