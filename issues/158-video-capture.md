# 158 — Video capture devices: cameras through a video gateway

**Type:** drivers + service · **Owner:** kernel track (USB and the driver), with the tools track for the programs · **Priority:** P2 · **Status:** open · **Blocked by:** — (the USB stack is the first part) · **Roadmap:** tracks A and G · **Constitution:** MC-3.3, MC-10.2, MC-11.4 (a camera is a sensor of the user's surroundings), Appendix B.6

## Problem

MIND Core can speak (`tts`) and hear (`listen`, voice V0–V2), but it cannot see. There is no driver for a camera and no service that hands frames to a program with the user's consent.

## Plan

1. **A general USB stack.** `usb_storage` today drives the xHCI controller only for one mass-storage device.
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
   - `camera`: shows the stream in a window (088) or full screen, takes a still (BMP, as `screenshot`) and records (AVI/MJPEG, as `record` in 090);
   - later, vision for the voice dialogue: describing what the camera sees through a model on the host bench, the way `hear` works.

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

[090](090-screen-recording.md), [088](088-text-window-manager.md), [107](../issues-done/107-batched-frame-path.done), [docs/voice](../docs/voice/README.md), [docs/legacy.md](../docs/legacy.md).
