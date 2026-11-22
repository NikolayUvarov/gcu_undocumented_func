# DRV landing request, again: `display` after first light on the Ryzen's second output (213-DRV-0086)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 06:20 UTC · **Concerns:** 213-DRV-0086 (advanced, stays open: its last criterion moved to 213-DRV-0092) · **Replaces:** `LANDING/2026-11-21T2158Z-DRV-ready-again-213-drv-0086-display-eight-batches-read-on-the-ryzen-stage-1-run-and-undone.md` (same branch, 965154171), not answered yet

**Ready:** `claude/213-DRV-0086-display-reads-the-engine` f3b2633c8

## What it carries beyond 965154171

On `main` a6f1855ad5 (merged in, no conflict). One code commit and the record:
- **fc8490ad4: the host model answers as the Ryzen did.** The watched run took table 12's short path (the VBIOS's mark of a PHY already set up was there, left by Linux): the model is seeded with the hardware's values, a new host case follows the run's 95 writes, and the failure cases run on both the warm and the cold state.
- **The record of first light,** the split of the second acceptance criterion into what was seen and what was not, the plan of step 4 (now 213-DRV-0092), and what a test of table 12's cold path would take.
- **The committed switches are still all off:** a machine that boots this `display` reads nothing and writes nothing.

## Tested

- **On LEG9:**
  - f8bd3bdba (the head before two text commits and the merge of `main`): the full `scripts/host_tests.sh`, exit 0, 257 result lines, 6470 tests;
  - f3b2633c8 (`main` merged in): the four display host suites pass (display 15, light 21, batches 10, atom 11) and `display` builds. The full host tests were not run again on the merged head.
- **On the Ryzen** (bench releases with the switches set per run; that machine and its two monitors, MC-12.1):
  - **first light, 2026-11-22 04:43 UTC, release 202611220438, the tree 965154171:** all eight stages done, the firmware's 30 registers unchanged after each, tables 12, 4 and 76 inside their bounds, 95 writes left in place. **The maintainer saw the engine's colour squares on HDMI-A-1 and MIND Core's screen on DP-2.** He did not watch DP-2 during the stages;
  - the same eight stages again in five later boots today, with the same log;
  - **the whole undo once on hardware** (boot 70, when a later stage of 213-DRV-0092 refused): the 56 registers the stages had written restored, both tables' off calls run to their end, the firmware's pipe unchanged, the output dark again.
- **Not tested:**
  - **table 12's long path** (a PHY not set up before: 37 writes through PHY A's control bus that no undo reaches). It has never run on this hardware: every start of MIND Core there comes through Linux, which leaves the PHY set up. A test needs a start that does not come through Linux and the maintainer's word;
  - that DP-2 kept its picture through every stage, by eye;
  - QEMU has no such engine (`display` is not started there: `NO DEVICE`); the aarch64 groups do not build it.
