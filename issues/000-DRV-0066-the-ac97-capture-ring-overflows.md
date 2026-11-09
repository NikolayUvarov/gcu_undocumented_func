# 000-DRV-0066 — `audio_gw`: the AC97 capture ring overflows in the `listen` suite on x86 TCG

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [000-APP-0114](000-APP-0114-hear-waits-for-a-whole-command.md) (the suite's check stays as it is) · **Constitution:** MC-6.1, MC-12.1

Numbered from the tools track's request of 2026-10-30 in `requests-DRV.md`.

## Problem

- **What fails.** The `listen` suite's 1 s recording reports `[LISTEN] RECORDED 48000 FRAMES (1000 MS), PEAK 0, RMS 0, OVERFLOWS 1`, where it requires `OVERFLOWS 0`. `audio_gw` logs `CAPTURE STOPPED: 1002 MS OF SOUND, PEAK 0, 1 OVERFLOWS`.
- **How often** (x86 QEMU, 4 CPUs, TCG, the tools track's container, 2026-10-30): 7 runs of 13. Among them:
  - 1 of 2 with `main`'s own `listen`, so 000-APP-0114's change is not the cause;
  - 1 of 4 with the check typing nothing while it records.
- **What was seen with it:** the compositor's frames came 5 to 10 a second, the longest up to 285 ms. The runs without an overflow had holds as long.
- **What it may be,** not shown: the ring is 16 buffers of 4 KiB (`CAPTURE_BUFFERS`), 341 ms of 48 kHz stereo. Under TCG the emulated card fills it at the host's pace, so a reader late by a third of a second overflows. Playback has 32 buffers.

## Plan

1. **Find the cause:** `audio_gw` logs, on an overflow, how long its reader had not taken a buffer and how full the ring was.
2. **Then the fix:** a capture ring as long as playback's (32 buffers, 683 ms), or a read that keeps up, whichever the log shows is needed.

## Acceptance criteria

- The `listen` suite passes 20 runs in a row on x86 TCG, with 4 CPUs, beside a build.
- The check's requirement, `OVERFLOWS 0`, is unchanged.

No real machine is needed.

## Related

[000-APP-0114](000-APP-0114-hear-waits-for-a-whole-command.md), [000-DRV-0057](000-DRV-0057-the-output-volume-and-the-microphones-gain.md).
