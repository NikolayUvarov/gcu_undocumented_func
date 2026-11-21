# 158-DRV-0082 — `video_gw`: a stream for each camera, so two cameras show at once

**Type:** driver (`video_gw`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-21: the gateway's change written and run on the bench, not committed) · **Blocked by:** the `cameras` suite needs APP's `camera -c` ([158-APP-0147](158-APP-0147-switching-between-cameras-in-camera-and-wm.md)) in `main` · **Main task:** [158](158-video-capture.md) · **Constitution:** MC-6.1, MC-11.4, MC-12.1

## Problem

`video_gw` holds one stream. It lists every camera ([158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md)), but a second program that opens another camera is refused while the first streams. The maintainer asked for both ways at once (2026-11-21): two windows with a live picture each, **and** one program that switches between the cameras. The Ryzen has two USB cameras (`0471:A6CA` and `0471:A6CD`), each straight in a root port of its own xHCI.

`idl/video.wit` already says that a camera has one owner at a time. So the interface does not change: the gateway's one-stream limit goes.

## What exists

- **The gateway's change,** written on 2026-11-21 and not committed:
  - a stream for each camera, keyed by the camera, one owner each; a second owner of a camera that is held gets `busy`;
  - while one reader waits for its frame, the other cameras' transfers are pumped;
  - `video/synthetic` beginning with `2` gives two test patterns.
- **A QEMU suite `cameras`,** written beside it: two test patterns listed, one recorded for 8 s (80 frames) while a still is taken of the other, and the camera a program holds is busy for another. It passed on the bench tree, which had APP's `camera -c` merged.
- **On the Ryzen** that gateway is in release 202611211156. Nobody has looked at its picture yet.

## Plan

1. Commit the gateway's change with its host cases.
2. Add the `cameras` suite to `tests/qemu_smoke.py`, `scripts/ci_local.sh` and `.github/workflows/ci.yml`, once `camera -c` is in `main`.
3. On the Ryzen, with both cameras:
   - two `camera -c <n>` windows in `wm`, both live, for five minutes; `video_gw`'s frame counts for each;
   - one `camera` that switches (`n` or Tab) while a second program holds neither camera, and while it holds one;
   - what each camera's stream costs its bus (isochronous or bulk, the setting chosen), from the gateway's lines.
4. Update `README.md` and `docs/api` where they say one stream.

## Acceptance criteria

- [ ] `cameras` runs in the gate and passes: two cameras streaming to two programs at once; a held camera is `busy` for another program.
- [ ] On the Ryzen two windows show the two cameras at once for five minutes, with the frame counts recorded; the maintainer saw both pictures move. This holds for that machine and those cameras (MC-12.1).
- [ ] One program switches between the cameras while the other window stays live.
- [ ] `idl/video.wit` is unchanged.

## Related

[158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md) (every USB camera listed), [158-APP-0147](158-APP-0147-switching-between-cameras-in-camera-and-wm.md) (`camera -c`, `n`, Tab), [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md) (a camera's modes).
