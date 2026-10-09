# Requests for the drivers track (DRV), not numbered yet

**Owner:** drivers track (the assessing session, `claude/ASR-DRV`, TRACKS 1.5) · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-09

The drivers track numbers its own tasks (`NNN-DRV-MMMM`), so requests from other tracks wait here. The drivers track turns each into a task and removes it from this file, and the file goes when it is empty.

## The MacBook Pro's camera: UVC over EHCI (158)

**Recorded by:** the kernel track (KRN), 2026-10-09, for main task [158](158-video-capture.md), after the maintainer's run on the MacBook Pro (fast-test 57242b9c78ff): `camera` does nothing there.

### Problem

- **The camera is found.** `usb_host` enumerates the FaceTime HD camera as `EHCI 0 05AC:8510 ADDRESS 2 (HIGH SPEED)`, behind the rate-matching hub 8087:0024 (LOG:boot0002.log). So it is the USB video class device that 158 expected, not the later PCIe camera.
- **Nothing drives it.** `video_gw` reports `[VIDEO] NO CAMERA`, and `camera` ends at once with `camera: no camera (the video gateway lists none)`. That message goes to a window that closes at the exit, so the user sees nothing (the window's part is requested from `APP`).
- **Ready under it.** EHCI runs on the Mac (211-DRV-0004, its interrupt and long-report work in 211-DRV-0017 and 0018). Its periodic schedule exists for interrupt endpoints.

### Plan (158's steps 2 and 3; the drivers track decides)

- Isochronous IN transfers in `usb_host`'s EHCI driver (iTDs in the periodic frame list for a high-speed device), then on xHCI.
- The UVC class in `video_gw`, or a driver feeding it:
  - probe and commit a format (YUY2 or MJPEG, 320x240 or 640x480);
  - payload headers and frame assembly.
- QEMU's `usb-audio` exercises isochronous transfers without a camera; host tests cover the payload headers.

### Acceptance criteria

On the MacBook Pro, `camera` shows the picture, the camera mark is on, and `camera -s still.bmp` writes a still. The `video` suite still passes on the synthetic source.

## The MacBook Pro's speakers: more of 551's step 2 if the GPIO is not enough (551)

**Recorded by:** the kernel track (KRN), 2026-10-09, for [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md). That task stays with the kernel session until done (TRACKS 1.5).

The kernel session has set the Cirrus codec's amplifier GPIOs for Apple's machines (551-DRV-0010's progress) because the maintainer's run found `beep` and `say` silent. Jack detection, the internal microphone and any further codec setup are 551's step 2, which this track numbers when it takes it.
