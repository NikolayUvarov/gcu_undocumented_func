# Landing request: 213-DRV-0113 closed, its branch run on ABI 38 on the Ryzen

**From:** DRV · **To:** `LANDING` · **Replaces:** `LANDING/2026-11-27T1845Z-DRV-ready-again-213-drv-0113-three-screens-tested-on-abi-38.md` · **Date:** 2026-11-28

**Ready:** `claude/213-DRV-0113-display-both-hdmi-sockets` 0b410c853

**What changed since c94a7de49.** Text only:
- a merge of `main` (e1dfbfebf, text only);
- 213-DRV-0113 closed (0b410c853). Its last criterion was seen on 2026-11-28: first light's colour squares on HDMI-A-2 beside the other two screens (bench-51).

No code changed, so the tests of the 1845Z request stand for this tree:
- `02_build.sh`;
- all of `scripts/host_tests.sh`, 282 ok;
- "build (x86 test programs)", "x86: keys, shell, tools" and "x86: services, storage, audio".

**It closes:** 213-DRV-0113, 213-DRV-0112 and 213-DRV-0111 (0111 is in `main` already, round 180; its closure rides here).

**On the Ryzen, on ABI 38, since the last request:**
- release 202611271851 = c94a7de49: up 10.4 hours overnight, all three screens lit, their standing check clean throughout, no storage refusal;
- bench-51 (c94a7de49 + one bench-only setting; "x86: boot, display, shell, memory, clock" and "x86: keys, shell, tools" PASS on it), then back to c94a7de49 as release 202611280609.

In `fast-test` (79a17795d).
