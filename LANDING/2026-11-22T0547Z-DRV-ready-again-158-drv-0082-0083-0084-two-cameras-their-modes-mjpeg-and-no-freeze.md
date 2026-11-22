# DRV landing request, again: two cameras at once, their modes, MJPEG decoded, and one camera no longer holds another program (158-DRV-0082, 0083, 0084)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 05:47 UTC · **Concerns:** 158-DRV-0082, 158-DRV-0083, 158-DRV-0084 (all advanced, all stay open) · **Replaces:** `LANDING/2026-11-21T2206Z-DRV-ready-158-drv-0082-and-0083-two-cameras-at-once-and-their-modes.md` (same branch, 5046dfbc6), not answered yet

**Ready:** `claude/158-DRV-0082-two-cameras-and-their-modes` 502d9a4f2

502d9a4f2 is f3898fde6 with one text commit on it (the three issues' record of the maintainer's look). The branch's head is further on again (a spill for large uncompressed frames, stale camera entries dropped); those come in a later request. **This request is for 502d9a4f2 only.**

## What it carries

On `main` a9d6433db7 (merged in, no conflict; it also merges with today's `main` without one). In `fast-test`.

Everything the 2206Z request listed (a stream for each camera; `usb_host`'s bulk IN streams, `idl/usb.wit` 1.6; `idl/video.wit` 1.1 with `modes` and `opened`; the `cameras` suite), and beyond it:
- **158-DRV-0084: MJPEG modes decoded by `video_gw`.** The JPEG decoder hardened, given the standard tables, a faster IDCT and a fuzz test. `idl/video.wit` 1.2, with nothing new on the wire. A size and rate that the uncompressed encoding cannot give is taken as MJPEG.
- **Opens and reads that wait are deferred** (`Call::defer`): while one camera settles after a close, or one program waits for its frame, the gateway serves the others. This mends what the maintainer saw yesterday, the other window freezing at the next-camera key.
- Two MJPEG streams of 1280x720 at once no longer end the gateway on memory.
- No system-call ABI change.

## Tested

- **On LEG9, the tree f3898fde6:**
  - the full `scripts/host_tests.sh` with `MIND_SSHD`: whole on its second run, 254 groups, 6442 tests, none failed. The first run stopped at `improve`'s `every_command_against_a_hub`, `main`'s chance failure and its only failure;
  - QEMU: `cameras` whole (with `camera -c`), `usb`, `ehci`, `usbaudio`, `bench`, `wm`.
- **All 19 aarch64 groups, on f3898fde6:** passed, on x32-128 through Effector (Ubuntu 22.04, QEMU 8.2.2 built from qemu.org's tag, AAVMF 2022.02, TCG), 02:36 to 05:44 UTC today, one operation a group. The same 19 passed there on 5046dfbc6 before (`KRN/2026-11-22T0234Z-DRV-…`).
- **On the Ryzen** (two cameras, `0471:A6CA` and `0471:A6CD`; that machine and those cameras, MC-12.1), with 5145dab32's services as the boot image's own (f3898fde6 is that with `main` merged in), release 202611212244:
  - the track's agent: camera 1 read for 40 s while camera 0 changed mode 30 times, 396 reads at most 265 ms apart; MJPEG 1280x1024, 1280x960, 1280x720 and 1024x768 decoded in 10 to 17 ms a frame; both cameras in MJPEG 1280x720 at once for 15 s, 147 and 222 frames;
  - **the maintainer, at its keyboard on 2026-11-22:** two `camera` windows at once, both moving; the next-camera key in one while the other kept moving; camera 0 at 1280x720 as MJPEG; both at 1280x720 at once; leaving. His words: «Проверку считаю успешной».
- **No QEMU device is a USB camera,** so bulk streams, the deferrals and the decoding of a real camera's MJPEG are shown on the Ryzen alone.
- **Not tested, or known not to work:**
  - uncompressed (YUY2) sizes of 640x480 and up beside a second camera break (`usb_host`'s 128 KiB bulk queue); mended later on the branch, not in this commit;
  - 158-DRV-0082's five minutes with frame counts and its "one program switches between the cameras" with a free camera to go to; 158-DRV-0083's comparison with Linux's lists; all still open in the issues;
  - the QEMU groups not named, and the 1-CPU suites: the gate runs them.
