# 158 — Video capture devices: cameras through a video gateway

**Type:** drivers + service · **Owner:** kernel track (USB and the driver), with the tools track for the programs · **Priority:** P1 (the maintainer's MacBook Pro camera, 2026-10-09) · **Status:** open · **Blocked by:** — (the USB stack, step 1 below, is done: [164](../issues-done/164-usb-hid-keyboard-and-mouse.done)) · **Roadmap:** tracks A and G · **Constitution:** MC-3.3, MC-10.2, MC-11.4 (a camera is a sensor of the user's surroundings), Appendix B.6

## Problem

MIND Core can speak (`tts`) and hear (`listen`, voice V0–V2), but it cannot see. There is no driver for a camera and no service that hands frames to a program with the user's consent.

## Plan

1. **A general USB stack** — done in [164](../issues-done/164-usb-hid-keyboard-and-mouse.done): `usb_host` and `idl/usb.wit`; a UVC driver needs isochronous transfers added to it. Originally: `usb_storage` drove the xHCI controller only for one mass-storage device.
   - Split it into an xHCI host-controller driver (`usb_host`: ports, enumeration, device and configuration descriptors, control, bulk, interrupt and isochronous transfers; USB 2 and 3) and class drivers that get one device's interface through it.
   - `usb_storage` becomes the first class driver. This is also the path to USB HID (keyboards and mice without the legacy PS/2, see `docs/legacy.md`).
2. **UVC class driver** (`uvc`, USB Video Class 1.1/1.5): the video control and streaming interfaces, the formats and frame sizes a camera offers (YUY2, MJPEG), isochronous or bulk streaming, payload headers and frame assembly.
3. **Video gateway** (`video_gw`, `idl/video.wit`), like `audio_gw` for sound:
   - It lists the cameras with their formats and opens a stream (format, size, rate).
   - Frames go to the client through a shared ring like `mind::netring` (issue 107), with timestamps from the monotonic clock. It converts YUY2 to the screen's pixel format on request.
   - **Consent:** a program gets a camera only through `REQUEST_CAMERA`, lent by the shell after the user agreed to it for that program. The gateway logs every open and close.
   - **Indicator:** while any stream is open, the compositor shows a camera mark that no program can hide.
4. **A test source.** QEMU 8.2 has no camera device (`-device help` lists none), so CI cannot test a camera.
   - `video_gw` gets a synthetic source (moving test pattern, frame counter, timestamps) when no camera is present and the test asks for it. It plays the role the WAV source plays for voice.
   - A real camera reaches QEMU with USB passthrough (`-device qemu-xhci -device usb-host,vendorid=…,productid=…`) for manual tests on a host with a webcam.
5. **Programs** (tools track):
   - `camera`: shows the stream in a window (088) or full screen, takes a still (BMP, as `screenshot`) and records (AVI/MJPEG, as `record` in 093);
   - later, vision for the voice dialogue: describing what the camera sees through a model on the host bench, the way `hear` works.

## Progress (2026-10-06)

Steps 3–5 are done on the synthetic source; step 2 (UVC with isochronous transfers in `usb_host`) is open.

- **Done — the gateway** `video_gw` (a boot service), `idl/video.wit` 1.0:
  - `cameras`, `open` (one owner a camera; a stream whose owner ended is closed), `read` (the next frame into a lent buffer, when it is due), `frame` (its number, time and size), `close`.
  - Every open and close is logged with the PID.
- **Done — the test source** (`libmind/src/video.rs`), only when the boot disk holds `video/synthetic`: eight colour bars moving left 4 pixels a frame, and the frame number in 32 cells of the bottom 16 rows. The module also has the YUY2 conversion a UVC camera will need.
- **Done — consent:**
  - `init` gives the gateway's only client to the shell (`SLOT_CAMERA` = 24, so `SLOT_DYNAMIC` is now 25).
  - The shell lends it for `REQUEST_CAMERA` (8192). It asked `<NAME> ASKS FOR THE CAMERA. ALLOW? (Y/N)` every time until 2026-10-09, when the maintainer ruled that a tool started for its purpose gets its device without a question (CONTRIBUTING.md); the camera mark still shows while a stream is open.
  - A script must declare `camera` too (msh's words; `gpio` was added there as well).
- **Done — the indicator:**
  - `display.wit` 1.1 `camera`: the gateway's heartbeat while a stream is open.
  - The compositor draws a green camera mark left of the capture dot for 1.5 s after the last heartbeat, on the framebuffer only.
- **Done — `camera`:**
  - it shows the stream (screen or `wm` window);
  - `-s` writes a still (BMP);
  - `-t` records an AVI of Motion JPEG frames (`mind::jpeg`, `mind::avi`, as `record`).
- **Done — tests:**
  - `tests/video_host.rs`: the pattern, the counter read back, sizes, YUY2.
  - QEMU `vfs` suite, x86 and aarch64 (`camera_check`, `camera_files`):
    - the shell asks, and a refused `camera` runs without a camera;
    - a still whose pixels are the pattern's frame exactly;
    - 3 s at 10/s give 30 frames numbered over 29 steps, every timestamp on the rate's grid (a slow encoder repeats pictures), and ffprobe reads the AVI;
    - the camera mark is on the screen during the stream and gone after it.
- **Fixed on the way:**
  - **The loader's slots:** the loader's list of slots a launcher may fill lacked `SLOT_GPIO` (207) and now `SLOT_CAMERA`; both are allowed.
  - **A program that ends at once:** a screen program that ended before the shell's `FOCUS` (`camera` without a camera, on aarch64) lost its output and its exit notice. The shell now starts foreground programs in front with `commit-in-front` (issue 160), so the kernel's foreground exit path keeps both.
  - **A frame wait of a minute** ([158-DRV-0001](../issues-done/158-DRV-0001-video-gw-frame-wait.done), 2026-10-07): `video_gw` read the clock twice. A frame that came due between the reads wrapped the wait into the kernel's 60 s cap, which left no frames and no camera mark. The gateway now reads the clock once a pass.
- **Camera time:** a reader slower than the rate (MJPEG under TCG on aarch64) gets the latest frame. `camera -t` keeps camera time: each number it did not get repeats the picture before it, as `record` keeps screen time. Every timestamp lies on the rate's grid.
- **Changed — "a program without it gets `ERR_RIGHTS`":** a program without `REQUEST_CAMERA`, or one the user refused, holds no capability to the gateway at all. Nothing in its capability space names the gateway, which is a stronger property than a refusal by the gateway. The test checks the refused program.
- **Open:**
  - step 2, UVC (isochronous transfers in `usb_host`, payload headers and frame assembly, with host tests) and a camera passed through to QEMU (manual);
  - YUY2 and MJPEG sources;
  - vision for the voice dialogue.

## The MacBook Pro's camera (2026-10-09)

The maintainer asked that the mind can use the test MacBook Pro's camera, started by an explicit program (`camera`) now and as a service started by hand later (173).

- **Where the camera sits.** On the MacBook Pro Retina of 2012 the FaceTime HD camera is expected, from Linux reports, to be a USB video class device (05AC:8510) on the internal EHCI side. The later models use a PCIe camera that is not UVC. The hardware report (174-KRN-0038) and `usb_host`'s log will say which.
- **What it needs, in order:**
  1. EHCI running on the Mac: [211-DRV-0004](211-DRV-0004-ehci.md), whose registers the kernel refused until [211-KRN-0021](../issues-done/211-KRN-0021-registers-inside-a-page.done) moved them (in `fast-test`, waiting for the maintainer's run);
  2. isochronous transfers in `usb_host`, on EHCI (iTD) and on xHCI. QEMU's `usb-audio`, an isochronous device, tests the transfer path without a camera;
  3. the UVC class itself (step 2): the probe and commit of a format, payload headers, frame assembly, and YUY2 and MJPEG into `video_gw`.

## Acceptance criteria

- **QEMU** (CI, synthetic source):
  - a program with `REQUEST_CAMERA` gets frames at the requested rate with increasing timestamps and the expected pattern;
  - a program without it gets `ERR_RIGHTS`;
  - the camera mark is on the screen while the stream runs;
  - `camera` writes a still and a 3-second AVI that ffprobe accepts.
- **USB:** `usb_storage` works unchanged on top of `usb_host` (the `ahci`/USB image suites).
- **Manual** (documented): with a passed-through webcam, `camera` shows its picture.
- **Host tests:** UVC payload-header parsing and frame assembly (with packet loss); YUY2 conversion.

## Related

[093](../issues-done/093-screen-recording.done), [088](../issues-done/088-text-window-manager.done), [107](../issues-done/107-batched-frame-path.done), [docs/voice](../docs/voice/README.md), [docs/legacy.md](../docs/legacy.md).
