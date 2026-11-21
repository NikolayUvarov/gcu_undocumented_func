# 158-DRV-0083 — A camera's modes: listed by `video_gw` and chosen by the program

**Type:** driver and interface (`video_gw`, `idl/video.wit`, `libmind` `uvc`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-21: `video.wit` 1.1 and the gateway's side committed; camera 0's ten offered modes opened and counted on the Ryzen; Linux's lists and camera 1's modes as a whole to come) · **Blocked by:** — (the programs' part is APP's 158-APP-0152, on APP's branch `claude/wizardly-franklin-kec1a9`) · **Main task:** [158](158-video-capture.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.4, MC-12.7

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

- [x] `video.wit` is 1.1; a program built against 1.0 runs against the new gateway (a host case of the old wire: `tests/idl_host.rs`, `a_video_1_0_program_works_with_a_1_1_gateway`).
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

**On the Ryzen (2026-11-21, 15:04-15:51 UTC), with the `usb_host` of 158-DRV-0082's bulk streams):**
- **The lists:**
  - camera 0 (`0471:A6CA`): `MODES: 26, 10 OFFERED; NOT: 3 YUY2 BEYOND 1280X1024 …, 13 MJPEG`;
  - camera 1 (`0471:A6CD`): `MODES: 24, 11 OFFERED; NOT: 1 YUY2 …, 12 MJPEG`;
  - each frame size has one interval;
  - their descriptors (1126 and 948 bytes) are in `tests/data/uvc-0471-a6ca.hex` and `-a6cd.hex`, and the host cases check the 13 and 12 YUY2 modes and the 10 and 11 offered;
  - **not compared with Linux's `v4l2-ctl --list-formats-ext`:** a Linux session needs the owner session's word.
- **Each offered mode of camera 0 opened in turn, 4 s each** (`videostub step 0 4`, c687acf56's gateway): every one delivered frames of its size. Good frames in about 3.7 s of stream, and broken ones:

  | mode | good | broken | longest gap |
  |---|---|---|---|
  | 640×480 at 25 | 63 | 6 | 6 ms |
  | 1280×1024 at 5 | 17 | 0 | 5 ms |
  | 1280×960 at 9 | 29 | 21 | 5 ms |
  | 1280×720 at 10 | 33 | 19 | 5 ms |
  | 1024×768 at 15 | 48 | 4 | 15 ms |
  | 800×600 at 20 | 56 | 9 | 4 ms |
  | 640×360 at 30 | 56 | 5 | 7 ms |
  | 352×288 at 30 | 57 | 3 | 6 ms |
  | 320×240 at 30 | 58 | 0 | 5 ms |
  | 160×120 at 30 | 58 | 0 | 4 ms |

  - **Rates:** camera 0 gives about 16 good frames a second in its 30-a-second modes in this room's light, as it did alone.
  - **Large frames** broke before they were converted in bands: 1280×720 had 12 good and 32 broken, the longest gap 55 ms.
- **Camera 1** (the infrared one):
  - 640×480 at 30: 112 to 113 good frames in 4 s, 0 or 1 broken;
  - 1280×1024 at 5: 16 good, 1 broken;
  - 1280×960 at 5: no good frame in 5 s, the camera's data slower than its rate;
  - 1280×720 at 10 (an earlier build): every frame 348 bytes short, which is 29 payload headers' bytes, and its payload headers carried the error bit (`0xCD`);
  - its other modes were not reached on this run, nor compared.
- **A change of mode:**
  - from one stream's close to the next stream's first frame takes 0.6 to 0.95 s on camera 0, of which the gateway's open to first frame is 0.2 to 0.73 s (more for larger frames);
  - the clear-halt alone did not stop a bulk camera's stream: camera 0 kept sending 640×480 frames after 1280×1024 was committed. Selecting setting 0 at the close cured that;
  - camera 1 still kept its old size once when committed 20 ms after the close; it now waits 250 ms after its last close before it is committed again.
- **Not established:**
  - the tolerance asked for by the third criterion;
  - camera 1's modes as a whole;
  - why camera 1's high modes break.

**Not done yet:**
- **On the Ryzen:**
  - each camera's list against Linux's (`v4l2-ctl --list-formats-ext`), which needs a Linux session;
  - camera 1's offered modes all opened and counted with the last build.
- **The programs' keys:** 158-APP-0152 (APP, on `claude/wizardly-franklin-kec1a9`) waits for this branch.


## Related

[158-DRV-0082](158-DRV-0082-a-stream-for-each-camera.md), [158-DRV-0084](158-DRV-0084-mjpeg-frames-decoded.md), [158-APP-0147](158-APP-0147-switching-between-cameras-in-camera-and-wm.md).
