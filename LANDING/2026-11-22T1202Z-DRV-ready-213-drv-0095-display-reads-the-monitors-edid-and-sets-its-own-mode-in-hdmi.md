# DRV landing request: `display` reads the monitor's EDID and sets its own mode, in HDMI (213-DRV-0095)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 12:02 UTC · **Concerns:** 213-DRV-0095 (advanced, stays open: one criterion, the maintainer's look)

**Ready:** `claude/213-DRV-0095-display-the-monitors-own-mode` 1e148b81f

**After** `LANDING/2026-11-22T0838Z-DRV-ready-213-drv-0092-display-puts-mind-cores-screen-on-the-ryzens-second-output.md` (3e68b77b9): this branch grew from 0092's and holds all of it, so landing this one lands that one too. If you would rather take one request, take this and call the other superseded.

## What it carries

On `main` 1e9d8beb5 (merged in; one text conflict, `issues/README.md`'s finished table, both rows kept). No ABI change (`common/abi.rs` untouched), no IDL change.

- **Batch 11 of step 1,** read only: HDMI-A-1's DDC line and the engine's I2C block as the firmware left them.
- **Step 2, the EDID:** both blocks of the monitor's EDID read over DDC1 through the engine's own I2C, checksums checked, and the I2C engine given back as batch 11 found it. Behind `EDID_ALLOWED`. The log carries the monitor's name, its preferred timing and a hash, not the bytes.
- **HDMI signalling:** tables 4, 12 and 76 run in HDMI mode, Display Core's stream set-up, and an AVI infoframe. Behind `HDMI_SIGNALLING`; without it the output is driven as before (DVI signalling).
- **The monitor's own mode:** the EDID's preferred timing in place of the fixed 1920x1080, and a surface whose pitch is rounded up to 64 pixels. Behind `MODE_FROM_EDID`.
- **The committed switches are all off:** `BATCHES_ALLOWED` 0, `STAGES_ALLOWED` 0, `UNDO_AFTER_STAGES`, `EDID_ALLOWED`, `HDMI_SIGNALLING` and `MODE_FROM_EDID` false. A machine that boots this `display` reads nothing and writes nothing; the Ryzen's bench builds raise them in a tree that is not pushed.
- The issue's record of the day's boots, and a section for whoever continues ("For a successor").

## Tested

- **On LEG9,** on 1e148b81f itself (WSL, `MIND_SSHD` from the unpacked .deb):
  - `02_build.sh`: success;
  - the full `scripts/host_tests.sh`: 264 suites, none failed;
  - the display suites with and without the Ryzen's VBIOS image (not in the repository): light 33, ddc 5, display 16, picture 5, batches 12, screen 3; `compositor_mirror` 3.
- **On the Ryzen, today** (that machine and that monitor, a KQ34C144C on HDMI-A-1; MC-12.1), in bench releases with the switches raised:

| Boot | Release | The branch at | What happened |
|---|---|---|---|
| 77 | 202611220908 | 3b67f7528 | batch 11 read, nothing written |
| 78 | 202611220916 | f13c247a8 | both EDID blocks read, checksums good, the I2C engine back as batch 11 found it |
| 79 | 202611220957 | 356af608b | HDMI signalling at 1920x1080: the tables' step and write counts as the host model's, 300 frames with no underflow, the mirror up |
| 81 | 202611221107 | 7bb169828 (a2ee81ca5's code) | 3440x1440 at 324 MHz from the EDID, in HDMI: counts and stage 9's values as the model's, 300 frames with no underflow, `[COMPOSITOR] SECOND SCREEN 3440X1440, PITCH 13824, Bgr, FROM PID 38: THE FIRST MIRRORED AT 2304X1440 FROM 568,0` |

  Each ran at its first try. Boot 81 was still up 34 minutes later (read through the server at 11:45 UTC), with no line from `display` after stage 16.
- **Not shown:**
  - **the maintainer's look** at either HDMI run: the picture's sharpness, the mode in the monitor's own menu, a minute without a drop. All of the above is by the log and the registers. He is asked; the issue's criterion 4 waits for it;
  - table 12's long path (213-DRV-0097): every boot took the short one, the PHY having been set up by Linux before;
  - a monitor whose preferred mode the engine cannot carry on one pipe, and any refusal path on hardware (the host cases cover the arithmetic only);
  - another monitor, another mode, another machine (MC-12.9).
- QEMU has no such engine (`display` is not started there); the aarch64 groups do not build it.

It is in `fast-test` (5cbef0631).
