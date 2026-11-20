# 158-DRV-0072 — `video_gw` serves every USB camera, bulk streaming too, and lets a program switch between them

**Type:** driver (`video_gw`, `libmind` `uvc`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-20: the gateway's part done, the programs' switch asked of APP) · **Blocked by:** — · **Main task:** [158](158-video-capture.md) · **Constitution:** MC-6.1, MC-11.4, MC-12.1

## Problem

**On the Ryzen, 2026-11-20** (`fast-test` 92c2c6e9), two USB cameras are plugged in. Linux's `uvcvideo` drives both. MIND Core shows neither:
```
0.741 usb_host: [USB] 0471:A6CA ON PORT 1 (HIGH SPEED)
1.239 usb_host: [USB] XHCI 1: 0471:A6CD ON PORT 2 (HIGH SPEED)
1.813 video_gw: [VIDEO] CAMERA 0471:A6CA "GDZS-DC004-EFVT-V": UVC 1.00, INTERFACES 0 AND 1, ENDPOINT 81 BULK; OFFERED AT 640X480 UP TO 25/S
447.744 video_gw: [VIDEO] "GDZS-DC004-EFVT-V" STREAMS IN BULK: NOT DONE YET
```
- **One camera at a time.** `video_gw` keeps a single USB camera, the first UVC pair it finds, so 0471:A6CD on the second xHCI is never listed. `idl/video.wit` already has `cameras` (up to 4) and `open(camera, …)` by number, so the interface need not change.
- **Bulk streaming is missing.** `camera -s data/link/cam1.bmp` opened A6CA, and `video_gw` refused it: that camera streams over a bulk endpoint, and `video_gw` reads isochronous ones only.
- **The maintainer's word (2026-11-20):** «надо уметь переключаться между камерами».

## Plan

1. **Every UVC camera on every xHCI,** up to `video.wit`'s 4, in a stable order: controller, then port path. Each is listed by `cameras`, with its name and largest mode, and the test pattern comes last. A camera unplugged leaves the list and ends its stream. A camera plugged in later is added.
2. **Bulk streaming:** UVC payloads read from a bulk IN endpoint, with the same header parsing, frame assembly and timing as the isochronous path. The commit and probe stay as they are.
3. **Switching:** `open(n)` on another camera closes the owner's earlier stream first, as now for the same camera. One owner per camera still holds (MC-11.4), and the camera mark stays.
4. **The program's side** (a camera number in `camera`, and a key or menu in `camera` and `wm` to switch) is the tools track's, asked on `internal-dev-messages`.

## Acceptance criteria

- [x] Host tests: bulk payloads with split headers and frames, assembled as the isochronous ones are (`tests/uvc_host.rs`, 13 passed). The list's order and a camera gone mid-stream are exercised on the Ryzen only, as QEMU has no USB camera.
- [x] QEMU x86: the camera checks of `x86: the window manager` and `x86: files and block writes` pass (the test pattern). aarch64 runs at the gate.
- [ ] On the Ryzen: both cameras listed (done); a still from each (done, camera 1 through a test build in reversed order); opened by number and switched back and forth in one program run: waits for `camera`'s switch (APP).

## Progress (2026-11-20): the gateway on the Ryzen

**The commit** is `158-DRV-0072` on `claude/DRV-ryzen-peripherals`. It went in through `replace` of `video_gw` on `fast-test` 92c2c6e9:
```
2214.411 video_gw: [VIDEO] CAMERA 0471:A6CA "GDZS-DC004-EFVT-V": UVC 1.00, INTERFACES 0 AND 1, ENDPOINT 81 BULK; OFFERED AT 640X480 UP TO 25/S
2214.436 video_gw: [VIDEO] CAMERA 0471:A6CD "GDZS-DC004-EFVT-IR??????": UVC 1.00, INTERFACES 0 AND 1, ENDPOINT 81 BULK; OFFERED AT 640X480 UP TO 30/S
2214.436 video_gw: [VIDEO] CAMERAS: 0 "GDZS-DC004-EFVT-V", 1 "GDZS-DC004-EFVT-IR??????"
2288.287 video_gw: [VIDEO] PID 65 OPENED CAMERA 0 "GDZS-DC004-EFVT-V" (ITS 320X240 YUY2, BULK) 320x240 AT 10/S
2288.626 video_gw: [VIDEO] FIRST FRAME AFTER 339 MS: 11 PACKETS, 375448 BYTES
```
- **`camera -s`** wrote a 320 × 240 still of the room from camera 0.
- **Camera 1, the infrared one,** gave a grey picture of the same room. It was taken through a test build that lists the cameras in reverse order (first frame after 499 ms; 2 good frames, 11 broken at the start).
- **Since then** the name's trailing characters outside ASCII are left out (`GDZS-DC004-EFVT-IR`).
- **What remains** is the switch in a program run. `camera` and `wm` take a camera number once APP adds it.

## Related

[158-APP-0043](158-APP-0043-the-camera-from-wm-and-console.md) (the camera from `wm` and the console), [158-DRV-0001](../issues-done/158-DRV-0001-video-gw-frame-wait.done).
