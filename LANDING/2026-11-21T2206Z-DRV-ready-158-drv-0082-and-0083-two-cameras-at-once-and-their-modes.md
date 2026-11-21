# DRV landing request: two cameras stream at once, and a camera's modes are listed and chosen (158-DRV-0082, 158-DRV-0083)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-21 22:06 UTC · **Concerns:** 158-DRV-0082 and 158-DRV-0083 (both advanced, both stay open)

**Ready:** `claude/158-DRV-0082-two-cameras-and-their-modes` 5046dfbc6

The branch's head is further on (decoded MJPEG, deferred opens and reads); those commits are still in their tests and come in a later request. **This request is for 5046dfbc6 only.**

## What it carries

On `main` 3c5ef74ca3 (merged in, ABI 37). `main` a9d6433db7 (round 166) is **not** merged into this commit: it merges with it without a conflict, and the merge with round 167's candidate (5667eb9655) built and has run on the Ryzen since 18:26 UTC. In `fast-test` since 85c425079.

- **`video_gw`: a stream for each camera** (158-DRV-0082). Two programs hold two cameras at once; a held camera is `busy` for another program; one program moves between the cameras.
- **`usb_host`: a bulk IN endpoint kept polled** (`idl/usb.wit` 1.6, `bulk-stream(handle, address, on)`): up to 32 page-sized transfers queued on the endpoint. The bench's cameras deliver their video over bulk, and one blocking transfer at a time starved the second camera.
  - A stream is made only when a client calls `bulk-stream`, and only `video_gw` calls it. Every other path of `usb_host` runs as before; isochronous streams keep their 4 pages.
  - One difference: `bulk` is refused on an endpoint that has a stream at the same endpoint number. No device of ours has both.
- **`idl/video.wit` 1.1** (158-DRV-0083): `modes(camera)` lists a camera's modes (encoding, size, rate, whether offered), `opened()` says which mode a stream runs in and whether it is scaled. A program built against 1.0 works with the 1.1 gateway (`tests/idl_host.rs`, `a_video_1_0_program_works_with_a_1_1_gateway`). Generated files committed. APP was told (`APP/2026-11-21T1742Z-DRV-…`).
- **A reset of a bulk camera at its close, frames converted in bands,** the two cameras' descriptors as test data (`tests/data/uvc-0471-a6ca.hex`, `-a6cd.hex`), `tests/video_stub`.
- **A new QEMU suite, `cameras`,** in `scripts/ci_local.sh` and `.github/workflows/ci.yml`: two cameras streaming to two programs at once, the held camera `busy`, `modes` and a change of mode, and APP's `camera -c`.
- **No system-call ABI change.**

## Tested

- **On LEG9, the tree 5046dfbc6:**
  - `02_build.sh` (x86) passes;
  - the full `scripts/host_tests.sh` with `MIND_SSHD`: 254 groups, 6427 tests, none failed;
  - QEMU, 4 CPUs: `cameras` whole (with `main`'s `camera`: 8 s recorded beside a still of the other camera, the held camera `busy`), `usb`, `ehci`, `iso`, `usbaudio`, `link`, `bench`, `wm`, `vfs`, `boot`;
  - the USB image's boot (`make_usb_image.py` and `usb_image_smoke.py`, 4 checks).
- **On the Ryzen** (two cameras, `0471:A6CA` and `0471:A6CD`; MC-12.1: this machine and these cameras):
  - with the branch's services put in by the cable's `replace` (13ea6031d, b712571ac, ABI 35): both cameras streamed at once to two test programs, 320x240 each and 640x480 each for 20 s; camera 0's ten offered modes opened in turn; **the maintainer saw two `camera` windows show the two cameras at once**, and saw that the next-camera key in one window makes the other freeze for a moment (the reset at a close blocks the gateway 0.3 to 0.5 s; mended later on the branch, not in this commit);
  - as the boot image's own services (5046dfbc6 in the bench releases since 18:26 UTC, ABI 37): both cameras are found at boot. Nobody has looked at the two windows again on these builds.
- **Not tested:**
  - the SMP, isolation, heap and services suites on 1 CPU, and the QEMU groups not named above: `usb_host` is not a kernel change, and the gate runs them;
  - the aarch64 groups: `usb_host`'s change is in the shared xHCI path, so they matter; none was run here;
  - 158-DRV-0082's five minutes with frame counts, and 158-DRV-0083's comparison with Linux's lists of modes: both still open in the issues.
