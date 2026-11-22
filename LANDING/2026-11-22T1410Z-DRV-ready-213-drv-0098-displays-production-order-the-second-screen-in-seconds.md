# DRV landing request: `display`'s production order: the second screen in seconds (213-DRV-0098)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 14:10 UTC · **Concerns:** 213-DRV-0098 (advanced: both criteria by the evidence below; it stays open for the maintainer's look); 213-DRV-0099 (a proposal in its issue, text only, for the maintainer's decision)

**Ready:** `claude/213-DRV-0098-display-both-screens-in-seconds` 92d0e7c29

**After** `LANDING/2026-11-22T1202Z-DRV-ready-213-drv-0095-display-reads-the-monitors-edid-and-sets-its-own-mode-in-hdmi.md` (1e148b81f, in your current candidate): this branch grew from 0095's and holds it. It has `main` 1e9d8beb5 inside; a trial merge with today's `main` (4d71860ac) and with your candidate 8a7e1f933 is clean.

## What it carries

No ABI change, no IDL change. Code in `display/` and its host suites only, `docs/profile/bootstrap.md`'s `display` row, and the two issues.

- **The production order, behind `SURVEY_ORDER`** (committed false): steps 1 to 4 make the survey's writes and checks in the same order, without its pauses and its line for every write.
  - Step 1 reads only what steps 2 to 4 take (143 of the 305 registers).
  - One line a stage with its time; a part's detail is held and logged only if the part fails.
  - Step 3's times by part, and the time from `display`'s start to `offer_screen`.
  - Every status and frame wait is by its condition with its own limit, as before. The one wait that had no condition, the `DIG_START` pulse, is now an explicit hold of 10 us in either order (Display Core holds it 1 us; the survey's log line held it about 21 ms).
- **`SURVEY_ORDER` true** gives the order of boots 70 to 81 exactly, for bring-up.
- **The committed switches are all off** (`BATCHES_ALLOWED` 0, `STAGES_ALLOWED` 0, the rest false): a machine that boots this `display` maps, reads and writes nothing.
- **Three things put right on the way:** a comment that claimed step 3 touches no register step 1 has not read (false in the survey too); a stale step 2 line (`THE MODE STAYS 1920x1080 …`) logged before the EDID's mode was lit; `bootstrap.md`'s `display` row (it said seven batches, eight stages, BAR 0 never read).

## Tested

- **On LEG9,** on a71a47b83 (92d0e7c29 adds only `issues/` text to it):
  - `02_build.sh`: success;
  - the full `scripts/host_tests.sh` with `MIND_SSHD`: exit 0; 265 Rust result lines, 6643 passed, 0 failed, 22 ignored; the Python and shell tests OK;
  - the display suites with and without the Ryzen's VBIOS image (not in the repository): 95 cases. The new ones run steps 3 and 4 on the model in both orders and compare the writes, the offers and `screen_gone`, in order: at 1080p DVI, 1080p HDMI and the EDID's 3440x1440; for 16 stages and for builds of 1, 2, 5, 8, 9, 10, 13 and 15; left in place and undone; and on seven failure paths.
- **On the Ryzen, today** (that machine, its KQ34C144C on HDMI-A-1, releases 202611221335 of a bench tree with a71a47b83 and the switches raised; MC-12.1). Six starts in a row, each from Linux, each whole log read from the stick:

| Boot | Offer after `display`'s start | `[COMPOSITOR] SECOND SCREEN …` at | Stage 15's 300 frames | Tables 12 / 4 / 76: steps/writes |
|---|---|---|---|---|
| 84 | 6163.2 ms | 9.597 s | clean, longest gap 130 us at frame 27 | 321/33, 33/5, 2276/23 |
| 85 | 6167.1 ms | 9.599 s | clean, 129 us at 32 | 321/33, 33/5, 2276/23 |
| 86 | 6166.3 ms | 9.664 s | clean, 129 us at 28 | 321/33, 33/5, 2276/23 |
| 87 | 6170.7 ms | 9.546 s | clean, 130 us at 32 | 321/33, 33/5, 2289/23 |
| 88 | 6165.6 ms | 9.638 s | clean, 130 us at 28 | 321/33, 33/5, 2263/23 |
| 89 | 6183.8 ms | 9.606 s | clean, 130 us at 32 | 321/33, 33/5, 2289/23 |

  - In each: 16 stages `DONE; THE FIRMWARE'S PIPE UNCHANGED (30 REGISTERS)`, no line of a failure, the short path of table 12. The survey's boots offered the screen at about 137 s.
  - The tables' writes are the model's and the survey's (33, 5, 23). Their step counts are not (308 and 300 in the survey's boot 81): unpaced, the tables' own status loops poll more often before their statuses show; the count moves by a few between starts for the same reason.
  - 5.0 s of the 6.2 s is stage 15's watch, kept at 300 frames.
- **Not shown:**
  - **the maintainer's look**: all of the above is by the log and the registers. He saw the survey's mirror at 1920x1080 this morning, nothing of the monitor's own mode or of this order yet;
  - the firmware's own screen (DP-2) by eye through the unpaced stages (by its 30 registers it did not change);
  - a start not through Linux (a cold PHY, table 12's long path: 213-DRV-0097), and any failure path on hardware;
  - another monitor, mode or machine (MC-12.9).
- QEMU has no such engine (`display` is not started there); the aarch64 groups do not build it.

It is in `fast-test` (aadbd8882; 213-DRV-0096's head on top of it, 9f3a608cd, is not requested yet).
