# 000-DRV-0073 — USB Audio Class 1 devices in `audio_gw`: the STM-Speaker speakerphone plays and hears

**Type:** driver (`audio_gw`, `usb_host`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-20: the Isoch TRB's type was wrong and is fixed; the hardware runs with the fix follow) · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1, MC-11.4, MC-12.1

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

- [x] Host tests (`tests/uac_host.rs`, 5 passed): the STM-Speaker's own descriptor bytes; formats it cannot use left out; feedback read in 10.14 and 16.16; the pacer; one channel to and from two.
- [x] QEMU (`--suites usbaudio`): `usb-audio` attached in the codec's place, its OUT endpoint streams about 1950 packets while `beep` plays, the device hears the tone (its wav holds at least a quarter of a second of loud samples), and unplugged, the codec comes back.
- [ ] On the Ryzen, with the maintainer: the speakerphone plays `say` and a tone, and `listen` hears its microphones. The STM-Speaker's own counters (underruns, overruns, clock faults) stay at zero for 5 minutes.

## Progress (2026-11-20): the path, and the Ryzen's AMD xHCI

**On `claude/000-DRV-0073-usb-audio`:**
- `usb_host` streams isochronous endpoints on xHCI both ways. Each TD names its frame, as Linux does on this host.
- `usb.wit` is 1.5 (`isochronous-out`, `BADGE_AUDIO`), and `init` gives `audio_gw` the client (213-KRN-0263, carried here).
- `audio_gw` drives a UAC1 device in the codec's place while it is plugged in.

**On the Ryzen** (test releases through the cable's `data/link/rel`, each confirmed by its trial):
- The STM-Speaker is attached: `PLAYBACK YES (INTERFACE 1, 2 CHANNELS, ENDPOINT 01, 196 BYTES, FEEDBACK), CAPTURE YES (INTERFACE 2, 1 CHANNELS, ENDPOINT 81, 98 BYTES)`.
- **The second xHCI (AMD 1022:1639) stops with a host controller error** (`USBSTS 00001011`) within 2 ms of the first isochronous TD. That happens for playback and capture alike, before the TD's own frame comes, with SIA and with frame IDs. The Configure Endpoint succeeds and leaves both endpoints running.
- **Linux on the same machine plays through the speakerphone** (`speaker-test`, no xHCI error). Its debugfs gives the same decoded contexts as ours, word for word:
  - `00030000 00C40008 … 00C400C4` for the OUT endpoint;
  - `00030000 00030028 … 00030003` for the feedback endpoint;
  - the slot `38100001 00010000 00000101`;
  - TDs `Isoch, IOC, ISP, TBC 0, TLBPC 0`, consecutive frame IDs.
- **What did not help:** the hub's TT think time 0, and no hub fields at all, as Linux's hub slot shows. Both were reverted.
- **Not yet compared:** the memory the rings and buffers lie in (ours at 0xC199xxxx, Linux's at 0xFEDxxxxx), and the controller's set-up (USBCMD, IMAN, CONFIG).
- **The bench is kept:**
  - such a controller is set up again at once, its devices found anew, and it takes no stream afterwards;
  - the link's converter, on the same controller, comes back within a second;
  - `audio_gw` goes back to the codec until the device is plugged in again.
- **Straight into a root port of the first AMD xHCI** (05:00.3, port 1, in camera 1's place; the boot stick and the Bluetooth are on that controller), the speakerphone stops the controller at its first control transfer, before any stream:
  - in MIND Core, ADDRESS DEVICE and ENABLE SLOT got no completion;
  - in Linux, at boot and again after a PCI remove and rescan, `xHCI host not responding to stop endpoint command … assume dead`;
  - the STM-Speaker project's own Windows measurements on this machine show a transaction error on the feedback endpoint, then a command timeout and an internal controller reset on PCI 5:0:4.

  So the fault belongs to this device (or its cable) on this AMD controller in every system, not to MIND Core's isochronous TDs alone. 000-DRV-0075 keeps such a controller in use: it is set up again, and the device is left alone. **Next:** the speakerphone on the DEXP's Intel xHCI, and another cable for its CN13.
- **The controller's own set-up compared with Linux's** (debugfs on the Ryzen while Linux plays through the hub):
  - Linux: USBCMD `0x5` (INTE), IMAN `0x2` (IE), IMOD `0xA0`, CONFIG 64 slots, an event ring of 2 segments, MSI-X, its DMA through the IOMMU (AMD-Vi, addresses `0xFEDxxxxx`);
  - here: USBCMD `0x1`, IMAN 0, IMOD at its reset value, 16 slots, one segment, no interrupt, physical addresses.
- **The interrupter as Linux's made no difference.** Test release 202611201000 set IMOD `0xA0`, IMAN.IE and USBCMD.INTE; the first isochronous TD still stopped the controller (`USBSTS 00001019`).
- **Next:** the IOMMU, from Linux's side. With the controller's IOMMU group in the identity domain, Linux's DMA uses physical addresses as ours does. If Linux's playback then stops the controller too, the addresses are the cause.

## The Isoch TRB was written as type 17 (2026-11-20, evening)

**`usb_host/src/xhci.rs` had `TYPE_ISOCH = 17`.** In xHCI 1.2's table 6-91 the Isoch transfer TRB is type 5. Type 17 is the Reset Device command, which has no place on a transfer ring.

**What each controller did with it** (QEMU's is shown by the fix; the two real controllers' stops are put down to it until their runs with the fix):
- **QEMU 8.2** takes a TD by its endpoint's type and looks at a TRB's type only to gather its buffer. It ended each TD with success and moved no byte. That is the "no byte taken" this issue put down to QEMU.
- **The Ryzen's AMD xHCI** stopped with a host controller error at the first TD.
- **On the DEXP's Intel xHCI** (release 202611201739, the first isochronous stream there, to its own USB audio 0573:1573) everything on the controller stopped at `beep`, twice. Its keyboard, the cable, the Wi-Fi adapter and the boot stick are all on that controller, so the machine looked hung and no log of it was kept. Whether the controller set an error bit is not known.

**Why the comparison with Linux missed it.** Linux's debugfs and trace show a TRB decoded (`Isoch, IOC, ISP, TBC 0, TLBPC 0`). Our fields were compared with that text, never the raw control word with the specification's table. The interrupter, the IOMMU, TD pacing, the slot context and the cache flushes tried since were not the cause.

**The fix:** `TYPE_ISOCH = 5`. In QEMU `usb-audio`'s wav then holds `beep`'s tone (336,968 bytes where a bare header was), and the suite asserts that the device heard it.

**Still open:**
- the DEXP and the Ryzen with the fix, on their own controllers;
- the AMD gate (`no_isochronous` for 1022) stays until the Ryzen's run shows a stream there;
- what this issue said of the root port (the fault "belongs to this device") is too strong, as the STM-Speaker project answered: the board was short of power in those runs.

## Related

[000-DRV-0075](../issues-done/000-DRV-0075-a-controller-that-stops-answering.done) (an xHCI that stops answering is set up again), [213-KRN-0127](213-KRN-0127-audio-gw-gets-a-second-hd-audio-function.md) (a second HD Audio function), [022](../issues-done/022-audio-tools-say-listen.done) (`say`, `listen`).
