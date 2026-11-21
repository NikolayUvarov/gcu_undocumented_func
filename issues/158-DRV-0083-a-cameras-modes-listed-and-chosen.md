# 158-DRV-0083 — A camera's modes: listed by `video_gw` and chosen by the program

**Type:** driver and interface (`video_gw`, `idl/video.wit`, `libmind` `uvc`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-21: `video.wit` 1.1 and the gateway's side committed; the Ryzen's runs to come) · **Blocked by:** — (the programs' part is APP's 158-APP-0152, on APP's branch `claude/wizardly-franklin-kec1a9`) · **Main task:** [158](158-video-capture.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.4, MC-12.7

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

## Progress (2026-11-21)

**`idl/video.wit` 1.1,** on `claude/158-DRV-0082-two-cameras-and-their-modes`. The note at its head says what changed. The calls of 1.0 keep their numbers and wire; two calls are new:
- **`modes(camera) -> result<list<mode, 256>, error>`** (method 6):
  - `record mode { encoding: encoding, width: u16, height: u16, rate: u8, interval: u32, offered: bool }`;
  - `enum encoding { yuy2, mjpeg, uncompressed, synthetic }`;
  - `interval` is the camera's own, in 100 ns units; `rate` is 10^7 / interval rounded down, 0 under one a second;
  - **the order is the descriptors'** (format, frame size, interval), so the same camera gives the same list each time;
  - a continuous range of intervals is listed by its shortest and its longest;
  - not-found for a number past the list.
- **`opened() -> result<opened, error>`** (method 7), the mode the camera sends after `open`, since `open`'s wire stays as 1.0 has it:
  - `record opened { camera: u8, index: u16, mode: mode, scaled: bool }`;
  - `index` is the place in that camera's `modes`: the listed mode of the frame size committed whose interval is nearest the committed one;
  - 65535 for a test pattern drawn at a size none of its modes has;
  - `scaled` when the frames come at another size, the one asked for;
  - not-open without a stream.
- **`open`'s refusals, as the `.wit` now says:** busy, another task owns the camera; not-found, the camera is gone or the number is past the list; invalid, the gateway does not offer that size or rate now.
- **A 1.0 gateway rejects `modes` and `opened`** (invalid), and a 1.0 program works with a 1.1 gateway. Host case: `tests/idl_host.rs` against 1.0's bindings kept in `tests/idl/video_1_0.rs`.
- The generated `libmind/src/idl/video.rs` comes from `scripts/mind_idl.py`. `Encoding`, `Mode` and `Opened` are in `tests/idl_fuzz_host.rs`'s list.

**`mind::uvc`:**
- `Camera::modes` lists each frame size of each format at each listed interval.
- `Camera::capacity(speed)` gives the bytes a second the isochronous settings carry: the largest setting's bytes an interval, a 12-byte header aside, at 8000 microframes a second (high speed) or 1000 frames (full), divided by 2^(bInterval - 1). It gives none for a bulk camera, and none above high speed, where the SuperSpeed companion's bursts are not read.
- Host cases in `tests/uvc_host.rs`: the modes of the laptop-like descriptors in their order, a range's two ends, a mode slower than one a second, the capacity by speed and interval, a bulk camera unbounded, a camera with MJPEG alone.

**`video_gw`:**
- **It keeps each camera's modes** from its descriptors, the first 256, and answers `modes` and `opened`.
- **A mode is offered** (`choices::offered`) when it is YUY2, within what `open` takes (16 to 1280 × 16 to 1024, 1 to 60 a second), and carried by the streaming settings. MJPEG is never offered until [158-DRV-0084](158-DRV-0084-mjpeg-frames-decoded.md). Host cases are in `tests/video_gw_host.rs`.
- **What it logs:**
  - at start, each frame size with every listed rate (`640X480 (25,20,15,10,5/S)`), and a line `MODES: n, m OFFERED; NOT: …` with the reasons counted;
  - each `OPENED` line ends with the mode's index (`; MODE 22`, `, SCALED`);
  - each camera's configuration descriptor in hex (`DESCRIPTORS n/m: …`, 72 bytes a line), from which the host cases of a real camera's modes are made.
- **The test patterns list three modes:** 640×480 at 30, 320×240 at 10, 160×120 at 5.

**`tests/video_stub`** also takes `videostub modes CAMERA` and `videostub step CAMERA SECONDS`, which opens each offered mode in turn. **The `cameras` suite** (passed on LEG9):
- lists the second pattern's three modes;
- opens each for 2 s, with frames of its size and its own pattern (61, 21 and 11 frames);
- checks that `opened` names each mode, and gives 65535 for 256×192;
- checks that `modes` of camera 2 is not found.

**Not done yet:**
- **On the Ryzen:**
  - each camera's list against Linux's (`v4l2-ctl --list-formats-ext`), and its descriptors' bytes for host cases;
  - each offered mode opened and its frames counted;
  - the gap of a change of mode.
  The machine was in another track's hands, then hung in Linux, when this was written.
- **The programs' keys:** 158-APP-0152 (APP, on `claude/wizardly-franklin-kec1a9`) waits for this branch.


## Related

[158-DRV-0082](158-DRV-0082-a-stream-for-each-camera.md), [158-DRV-0084](158-DRV-0084-mjpeg-frames-decoded.md), [158-APP-0147](158-APP-0147-switching-between-cameras-in-camera-and-wm.md).
