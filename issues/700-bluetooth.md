# 700 — Bluetooth: the host stack and its devices (audio, keyboards and mice, others)

**Type:** main task (track `BLT`) · **Owner:** — (the track is open for taking once its time comes) · **Priority:** P3 · **Status:** later: after Wi-Fi ([550](550-network-on-real-hardware.md), [550-DRV-0006](550-DRV-0006-broadcom-wifi.md)), the maintainer's order of 2026-10-09 · **Blocked by:** — · **Roadmap:** — (no roadmap step yet; the track was opened at the maintainer's request on 2026-10-09) · **Constitution:** MC-3.1, MC-3.5, MC-3.8 (each device and profile by its own capability), MC-7.1 (a radio link drops and comes back), MC-10.1, MC-10.3 (what goes out over the radio, and the audit of pairing), MC-11.4 (parsing what comes over the air apart from the keys)

## Problem

MIND Core has no Bluetooth. The maintainer wants Bluetooth devices added: audio (headphones, a headset), keyboards and mice, and other devices of their own kinds.

What the MacBook Pro has (LOG:boot0001.log, 2026-10-09):
- **The controller.** `05AC:8286` is a Broadcom controller (HCI over USB). It sits behind the hub `0A5C:4500` on EHCI 1.
- **The firmware's HID proxy.** `05AC:820A` and `05AC:820B` are the controller's HID proxy. In this mode the controller shows Bluetooth keyboards and mice paired in macOS as plain USB HID devices, and `usb_hid` already takes them.
- **Leaving the proxy.** The proxy ends once the controller is switched to HCI mode. From then on the stack does the HID part.

## Plan

The track's directories:
- `bt_hci`: the transport, a class driver of `usb_host`;
- `bt_host`: the stack, a service like `netstack`;
- `bt`: the tool;
- `idl/bluetooth.wit`;
- `libmind`'s `bluetooth`.

It is split into `700-BLT-MMMM` tasks when the track is taken.

1. **The HCI transport** (`bt_hci`).
   - A class driver of `usb_host` for the wireless controller class (0xE0/01/01). It gets its own badge, like `BADGE_VIDEO`.
   - The endpoints: commands on the control endpoint, events on the interrupt endpoint, ACL data on the bulk pair, SCO voice on the isochronous endpoint (`usb.wit` 1.2's `select` and `isochronous`, from 158).
   - Switching the Mac's controller out of the HID proxy mode, and Broadcom's firmware patch if the controller needs one (its licence first, as for 550-DRV-0006).
2. **The host stack** (`bt_host`).
   - HCI commands and events, L2CAP, SDP, and GATT over ATT for Bluetooth Low Energy.
   - Pairing with Secure Simple Pairing and LE Secure Connections.
   - Link keys kept by `keystore`, sealed where a TPM is.
3. **Profiles**, each reaching its device through a narrow capability.
   - HID (classic and over GATT) into the system's input, as `usb_hid` does.
   - A2DP to `audio_gw` (and HFP for a headset's microphone, which is a sensor: MC-11.4).
   - Other devices (RFCOMM, GATT services) to programs through a gateway. A program is lent a device by the shell: a tool for its own device gets it without a question (CONTRIBUTING.md); anything beyond that is asked.
4. **The tool `bt`**: scan, pair, list, forget, connect.
5. **Policy and audit.**
   - What a radio may do: discoverable or not, and who may pair. These are the user's choice, like `netpolicy`.
   - Each pairing, connection and lending is logged.

## What it needs from other tracks

| Track | What |
|---|---|
| `DRV` | `usb_host`: the badge and the class 0xE0, and isochronous OUT for SCO; `audio_gw`: a Bluetooth sink and source |
| `NET` | `keystore`: link keys |
| `KRN` | `init`: the grants (usb_host's client, the input privilege, the audio and keystore clients); the shell's lending |
| `APP` | `bt` in the shell's help and in `wm`'s menu |

## Acceptance criteria

- **QEMU (CI).**
  - QEMU 8.2 has no Bluetooth controller of its own, so the stack is tested on the host against recorded HCI traffic. This covers pairing, a HID report and an A2DP stream.
  - A bridge to a real controller on the host (`-device usb-host`) is the manual test.
- **The MacBook Pro.**
  - After `bt` pairs a Bluetooth keyboard and mouse, they work without the firmware's proxy.
  - Headphones play `say`.
  - A device of another kind reaches the program it was lent to, and no other.
- **Host tests:** the HCI, L2CAP, ATT and SDP parsers, and the pairing state machine.

## Related

[550](550-network-on-real-hardware.md) and [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) (Wi-Fi first), [158](158-video-capture.md) (isochronous transfers in `usb_host`), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md) (`audio_gw`), [211-DRV-0018](211-DRV-0018-macbook-trackpad-gestures.md) (input from a Mac's devices).
