# 158-DRV-0082 — `video_gw`: a stream for each camera, so two cameras show at once

**Type:** driver (`video_gw`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-21: the gateway's change committed with host cases and the `cameras` suite; two bulk cameras at once on the Ryzen being fixed) · **Blocked by:** — · **Main task:** [158](158-video-capture.md) · **Constitution:** MC-6.1, MC-11.4, MC-12.1

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

## Progress (2026-11-21)

**The gateway's change, reviewed and committed** on `claude/158-DRV-0082-two-cameras-and-their-modes`:
- **The choices that need no device** are in `video_gw/src/choices.rs`: which camera a number names, whether a program may open it beside the streams held, how many patterns `video/synthetic` asks for. Host cases: `tests/video_gw_host.rs` (6).
- **Fixed against the bench diff:**
  - a stream whose camera went is marked so: it takes no more packets, holds its camera for nobody, and is closed without asking the device;
  - a pattern's reader waits for its frame while the cameras' streams are pumped, rather than in one sleep of up to a second.

**Two bulk cameras at once on the Ryzen failed** with the bench diff (release 202611211156, the maintainer at the keyboard, 2026-11-21, log `S/rz-cam-user1.txt`):
- **One program switching** between the cameras worked: for example camera 1 for 61 s, `618 FRAMES`, the camera's `1756 GOOD FRAMES, 63 BROKEN`.
- **Two programs at once did not.** Camera 0 streamed; camera 1, opened beside it, got `NO GOOD FRAME FOR 5000 MS: 0 GOOD FRAMES, 2 BROKEN (THE LAST 65524 BYTES OF 153600), 12 PACKETS` twice, and camera 0's picture jerked.
- **Why, as the code reads:**
  - every bulk read is one blocking call to `usb_host`, which queues one transfer and waits for it;
  - the camera streams from its commit on at its sensor's pace, and `pump_bulk` read a camera until a whole frame ended (about 33 ms);
  - meanwhile the other camera's endpoint had no transfer, so nothing was read from it and its buffer overflowed; its frames came cut (65524 bytes, one payload of three).
- **The change:** while two bulk cameras stream, a pump is one transfer of 16 KiB at most, the cameras in turn, and the loop waits 1 ms at most for a request. Each camera then waits one 16 KiB transfer of the other's, not a frame. The close line of a bulk stream now gives its pumps and its transfers' time (`… PUMPS, … TRANSFERS IN … MS OF … MS OPEN, THE LONGEST … MS`), to see where the time goes.
- **Not run on the Ryzen yet.**

**The `cameras` suite** (`tests/qemu_smoke.py`, in the gate's group `x86: keys, shell, tools`):
- **Through a test program of DRV's,** `tests/video_stub` (`videostub CAMERA SECONDS [WxH] [RATE]`, built by `scripts/x86_fixtures.sh`), since `camera -c` (158-APP-0147) is not in `main`: two patterns listed; the first read for 8 s (81 frames) while the second is read for 3 s (28 frames), each its own pattern (the second's counter 300 ahead); the first busy for a third program; camera 2 not found. Passed on LEG9.
- **Through `camera -c`** when the tree has it (the bench diff's checks: a recording of one, a still of the other, the first busy); until then the suite says it skipped that part.
- **What the gate cannot show:** two real streams starving each other. QEMU 8.2 has no USB camera device (`-device help` lists none), and the patterns move no USB transfer. Only the Ryzen's two cameras show it.


## Related

[158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md) (every USB camera listed), [158-APP-0147](158-APP-0147-switching-between-cameras-in-camera-and-wm.md) (`camera -c`, `n`, Tab), [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md) (a camera's modes).
