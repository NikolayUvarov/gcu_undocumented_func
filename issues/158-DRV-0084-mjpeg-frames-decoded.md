# 158-DRV-0084 — MJPEG frames decoded, so a camera's larger modes are offered

**Type:** driver (`video_gw`, a decoder in `libmind`) · **Owner:** `DRV` · **Priority:** P3 · **Status:** open (2026-11-21) · **Blocked by:** [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md) · **Main task:** [158](158-video-capture.md) · **Constitution:** MC-6.1, MC-10.1, MC-12.1

## Problem

On USB 2 a camera sends its larger sizes and higher rates as MJPEG alone: uncompressed, they do not fit the bus. `video_gw` decodes none: a camera without YUY2 gets the line `HAS NO YUY2 FORMAT (MJPEG IS NOT DECODED YET)`, and with [158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md) its MJPEG modes are listed as not offered.

## Plan

1. **Choose the decoder and say why:** a baseline JPEG decoder of our own (sequential, 8 bit, 4:2:2 and 4:2:0, the standard's Huffman tables when a frame carries none, as UVC allows), or a third-party crate with its source and licence in `THIRD_PARTY.md`.
2. **The input is the device's:** every length and table is checked, and a malformed frame is dropped with a count, never trusted. The decoder goes into the fuzzing list (`ASR`).
3. **`video_gw`** decodes into the size asked and offers the MJPEG modes.
4. **Measure** the time a frame takes to decode on the Ryzen at each size: a mode the machine cannot decode at its rate is offered at the rate it reaches, and the list says so.

## Acceptance criteria

- [ ] Host cases: frames recorded from the Ryzen's two cameras decode to the picture Linux decodes, within a stated error; truncated and corrupted frames are dropped.
- [ ] The decoder is fuzzed with the other parsers.
- [ ] On the Ryzen an MJPEG mode of each camera is opened and shown, with its frame rate and decode time recorded (MC-12.1).

## Related

[158-DRV-0083](158-DRV-0083-a-cameras-modes-listed-and-chosen.md), [158-DRV-0072](158-DRV-0072-every-usb-camera-and-bulk-streaming.md).
