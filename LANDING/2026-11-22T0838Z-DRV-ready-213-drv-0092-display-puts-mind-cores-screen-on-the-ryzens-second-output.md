# DRV landing request: `display` puts MIND Core's screen on the Ryzen's second output (213-DRV-0092)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 08:38 UTC · **Concerns:** 213-DRV-0092 (advanced, stays open: one criterion); 213-DRV-0100 (closed with no change); 213-DRV-0095 to 0099 (written down, not started)

**Ready:** `claude/213-DRV-0092-display-mind-cores-picture` 3e68b77b9

**After** `LANDING/2026-11-22T0620Z-DRV-ready-again-213-drv-0086-display-first-light-on-the-ryzens-second-output.md` (f3b2633c8): this branch grew from 0086's and holds its code. It needs 213-KRN-0272, which is in `main` since round 169.

## What it carries

On `main` 88d611c1da (merged in, no conflict; it merges with today's `main` without one as well). No ABI change, no IDL change of ours.

- **Stages 9 to 16 of `display`,** each with its undo, on top of first light's eight:
  - 9 reads the firmware's own pipe as a reference and measures the reference clock;
  - 10 draws a picture of its own into the range of BAR 0 behind the firmware's screen (slot 5);
  - 11 to 14 set up the second pipe behind the engine's test pattern (clocks, DPP, HUBP, MPC);
  - 15 unblanks it and takes the pattern off, with a watch of 300 frames on both pipes: an underflow on the firmware's pipe puts the stage back at once;
  - 16 offers the range to the compositor through `display.wit` 1.3 (`offer_screen`; `screen_gone` first in its undo).
- **Two batches more of step 1** (the firmware's pipe, the MPC tree and the shared blocks), read only.
- **What the day's faults left in the code:** the image asks for a 256 KiB stack, with a compile-time bound on what step 3 puts on it (it outgrew the default 64 KiB once and faulted); a mark so that the stages run once a boot, whatever restarts the program; a stage 9 or 10 that is not ready stops there and leaves first light in place.
- **The committed switches are all off:** `BATCHES_ALLOWED` 0, `STAGES_ALLOWED` 0, `UNDO_AFTER_STAGES` false. A machine that boots this `display` reads nothing and writes nothing; the Ryzen's bench builds raise them in a tree that is not pushed.
- The issue's record of the day, and five tasks written down for what is not done: the monitor's own mode from its EDID (0095), hot-plug (0096), table 12's cold path (0097), both screens at start in seconds (0098), both screens by default (0099).

## Tested

- **On LEG9,** the tree 7e973c31f (3e68b77b9's code; the last commit changes only `issues/`):
  - `02_build.sh`: success;
  - the full `scripts/host_tests.sh` with `MIND_SSHD`: 263 suites, none failed;
  - the display suites with and without the Ryzen's VBIOS image (not in the repository): light 31, screen 3, batches 11, display 15, picture 4; `compositor_mirror` 3 and `idl_fuzz` 3.
- **On the Ryzen, today** (that machine and its two monitors, MC-12.1), in bench releases with the switches raised:

| Boot | The branch at | What happened |
|---|---|---|
| 70 | 13c77c7b9 | stage 9 refused; every write of first light undone, 98 restores |
| 71 | d562026b2 | stages 9 and 10 |
| 73 | 34d4c9027 | stages 11 to 14, the firmware's pipe unchanged |
| 74 | 34d4c9027 | stage 15 met an underflow after 41 frames; put back at once, then all undone |
| 75 | e20a0244f | stage 15 clean: 300 frames, no underflow or request timeout on either pipe |
| 76 | aa1d54a93 with 0272 | stage 16: `[COMPOSITOR] SECOND SCREEN 1920X1080, PITCH 7680, Bgr, FROM PID 38: THE FIRST MIRRORED AT 1728X1080 FROM 96,0` |

  - **The maintainer, at the monitors in boot 76:** «На обоих экранах изображение одинаковое (второе следует за первым)», «отметки видны», «кажется, что не рвется».
- **Not shown:**
  - which of the five changes between boots 74 and 75 took the underflow away: all came at once;
  - stage 15's own picture by eye, and DP-2 by eye through stages 11 to 16 (by its registers it did not change);
  - `screen_gone` on hardware: nothing was unplugged, and no undo ran after stage 16;
  - table 12's long path: every boot took the short one, the PHY having been set up by Linux before;
  - another monitor, another mode than 1920x1080, another machine (MC-12.9).
- QEMU has no such engine (`display` is not started there); the aarch64 groups do not build it.
