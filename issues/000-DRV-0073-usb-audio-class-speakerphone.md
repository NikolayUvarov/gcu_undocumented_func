# 000-DRV-0073 — USB Audio Class 1 devices in `audio_gw`: the STM-Speaker speakerphone plays and hears

**Type:** driver (`audio_gw`, `usb_host`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-20) · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1, MC-11.4, MC-12.1

## Problem

**On the Ryzen, 2026-11-20** (`fast-test` 92c2c6e9), the maintainer's own speakerphone (the STM-Speaker project: STM32F746G-DISCO, UAC1 full duplex with HID) neither plays nor hears in MIND Core. In Linux it is `snd-usb-audio`. MIND Core enumerates it and leaves it alone:
```
1.333 usb_host: [USB] XHCI 1: CAFE:4014 ON PORT 1.1 (FULL SPEED)
1.514 usb_hid: [USB_HID] CAFE:4014 INTERFACE 3: SUBCLASS 0 PROTOCOL 0, ENDPOINT 83
1.560 usb_hid: [USB_HID] CAFE:4014 NEITHER KEYBOARD NOR POINTER, LEFT ALONE
```
`audio_gw` drives HD Audio codecs only, and no driver takes a USB audio interface.

**The device,** from its firmware (`firmware/usb/sp_usb_descriptors.c`, `sp_usb_types.h`):
- 48 kHz, 16-bit, stereo playback on isochronous OUT endpoint 0x01. It is asynchronous, at most 196 bytes a frame, with a feedback endpoint 0x82.
- Capture on isochronous IN endpoint 0x81: one or two channels, at most 98 or 196 bytes a frame.

## Plan

1. **`usb_host`:** isochronous OUT transfers, and the feedback IN endpoint read each interval. Isochronous IN already exists for `video_gw`.
2. **A UAC1 path in `audio_gw`:**
   - parse the audio control and streaming interfaces;
   - choose the 48 kHz, 16-bit alternate setting;
   - feed playback at the rate the feedback asks;
   - read capture.
   It is offered beside the HD Audio codec as another output and input, through the same interface. A USB device plugged in later becomes available without a restart.
3. **Choosing the device** goes through the existing audio interface (`idl/audio.wit`). If a choice between outputs is missing there, that is a new IDL version, agreed with its owner first.

## Acceptance criteria

- [ ] Host tests: the UAC1 descriptors of the STM-Speaker and of a plain USB headset parsed; the feedback value (10.14) turned into the frames a millisecond; a malformed descriptor refused with its reason.
- [ ] QEMU: `usb-audio` (QEMU's UAC1 device) plays a tone, and the `audio` suite passes as before.
- [ ] On the Ryzen, with the maintainer: the speakerphone plays `say` and a tone, and `listen` hears its microphones. The STM-Speaker's own counters (underruns, overruns, clock faults) stay at zero for 5 minutes.

## Related

[213-KRN-0127](213-KRN-0127-audio-gw-gets-a-second-hd-audio-function.md) (a second HD Audio function), [022](../issues-done/022-audio-tools-say-listen.done) (`say`, `listen`).
