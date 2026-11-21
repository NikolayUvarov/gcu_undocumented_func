# 158-DRV-0083 — A camera's modes: listed by `video_gw` and chosen by the program

**Type:** driver and interface (`video_gw`, `idl/video.wit`, `libmind` `uvc`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-21) · **Blocked by:** [158-DRV-0082](158-DRV-0082-a-stream-for-each-camera.md) (the same file); the programs' part is asked of APP · **Main task:** [158](158-video-capture.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.4, MC-12.7

## Problem

The maintainer (2026-11-21): switching must be possible from inside the application, of the camera **and of its modes**.

- **The camera** is switched in `camera` and in `wm` since [158-APP-0147](158-APP-0147-switching-between-cameras-in-camera-and-wm.md).
- **The modes** cannot be:
  - `video.wit` 1.0 gives a camera one size and rate (`cameras`: the largest YUY2 frame up to 640 wide);
  - `open(camera, width, height, rate)` takes the frame size nearest to what was asked and scales it;
  - so a program cannot learn what a camera offers, nor ask for one of its own modes.

## Plan

1. **`idl/video.wit` 1.1.** A new call lists a camera's modes; the wire of 1.0 stays as it is, so a 1.0 program keeps working (MC-12.4).
   - `modes(camera)` answers a list of modes: width, height, frames a second, the encoding the camera sends (YUY2 or MJPEG), and whether the gateway offers it now.
   - `open` is unchanged on the wire. Asked for a listed mode's exact size and rate, it gives that mode unscaled; `frame` says the size delivered.
   - The generated files are made by `scripts/mind_idl.py` and committed; the new types go into `idl_fuzz_host`'s list.
2. **`video_gw`:**
   - keeps each camera's frame sizes and intervals, which `mind::uvc` already parses, and answers `modes`;
   - marks a mode as not offered when its payload exceeds what the camera's streaming settings carry, and every MJPEG mode until [158-DRV-0084](158-DRV-0084-mjpeg-frames-decoded.md);
   - gives the test patterns three modes, for the tests.
3. **A change of mode** is the owner's `close` and `open`. Measure the gap on a real camera.
4. **Tests:**
   - host cases: the modes of the Ryzen's two cameras from their descriptors (their bytes are in the gateway's log lines), a camera with MJPEG alone, a mode too large for the bus;
   - the `cameras` suite: modes listed, one camera opened in two of its modes one after another, the frames of the size asked.
5. **The programs** (APP's): `camera` and `wm`'s camera show the mode and step through the offered ones with a key. Asked of APP on `internal-dev-messages`; until it lands, the hardware runs use `camera`'s options or a test program.
6. **On the Ryzen:** each camera's list against what Linux lists for it (`v4l2-ctl --list-formats-ext`), and each offered mode opened and its frames counted.

## Acceptance criteria

- [ ] `video.wit` is 1.1; a program built against 1.0 runs against the new gateway (a host case of the old wire).
- [ ] `modes` lists each of the Ryzen's cameras' YUY2 modes as Linux lists them; the differences, if any, are named.
- [ ] Each offered mode opens and delivers frames of its size at its rate, within a stated tolerance; the figures recorded for that machine and those cameras (MC-12.1).
- [ ] The `cameras` suite checks `modes` and a change of mode, in the gate.
- [ ] With APP's part: in `camera`, a key steps through the camera's modes and another through the cameras, and the picture follows.

## Related

[158-DRV-0082](158-DRV-0082-a-stream-for-each-camera.md), [158-DRV-0084](158-DRV-0084-mjpeg-frames-decoded.md), [158-APP-0147](158-APP-0147-switching-between-cameras-in-camera-and-wm.md).
