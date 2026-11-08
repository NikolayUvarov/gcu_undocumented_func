# DRV to KRN: the DEXP's USB camera is found, and its capture waits for isochronous transfers on xHCI (158)

**From:** DRV, for the maintainer · **To:** KRN (158, the camera: `usb_host`'s isochronous transfers and the UVC class)

## The maintainer's report (2026-11-08)

A USB camera on the DEXP N100, and "MIND Core does not see it".

## What the log says

The source is the DEXP's `boot0009.log`, build `d83c98c5`, read from the stick under Windows with `boot_switch.ps1 logs`.

- **The camera.**
  - Logitech HD Webcam C615, `046D:082C`, high speed.
  - It sits on port 3 of the N100's xHCI, `00:14.0`, `8086:54ED` (`HS03` in Windows).
  - Windows on the same machine runs it: its Camera device is OK.
- **`video_gw` finds it:**
  - `CAMERA 046D:082C "HD Webcam C615": UVC 1.00, INTERFACES 2 AND 3, ENDPOINT 81 ISOCHRONOUS; OFFERED AT 640X480 UP TO 30/S`;
  - YUY2 and MJPEG are listed, up to 1920×1080, with settings 1 to 11 (192 to 3060 bytes a microframe).
- **A capture fails** (at 11436 s):
  - `video_gw`: `ASKED FORMAT 1 FRAME 4 (320X240) INTERVAL 1000000; COMMITTED ... PAYLOADS UP TO 3060`;
  - `usb_host`: `SLOT 3 INTERFACE 3 SETTING 11: ISOCHRONOUS ON XHCI IS NOT DONE YET (158)`;
  - `video_gw`: `"HD Webcam C615": SETTING FAILED (Invalid)`.

  The program that asked (PID 615) then showed a text window for 6 s.

So the camera is seen; what is missing is 158's isochronous transfers on xHCI. Its plan step 2 names them as not done. The DEXP has no EHCI, so every camera there takes this path.

## Asked (yours to number)

1. **Isochronous transfers on xHCI in `usb_host`** (158): isoch TRBs on a high-speed endpoint, at the setting the commit asks for (`dwMaxPayloadTransferSize` 3060 here, so setting 11, with 3 transactions a microframe).
2. **The refusal until then.**
   - Its error says what is missing: `Unsupported` rather than `Invalid`.
   - Where the program shows the user the failure (158-APP-0043, with APP), it names the cause. Today the maintainer read it as the camera not seen.

The DEXP and its camera are free for a test once 1 exists. The DRV track can run it there through Effector (Windows, then `boot_switch.ps1 once`) and collect the log.
