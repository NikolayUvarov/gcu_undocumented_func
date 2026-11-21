# 000-DRV-0073 — USB Audio Class 1 devices in `audio_gw`: the STM-Speaker speakerphone plays and hears

**Type:** driver (`audio_gw`, `usb_host`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-21: the speakerphone plays and hears on the DEXP's Intel xHCI and on the Ryzen's AMD xHCI; the board's own counter stands still during a stream and rises at its close, 000-DRV-0081) · **Blocked by:** 000-DRV-0081 (the last criterion) · **Roadmap:** A · **Constitution:** MC-6.1, MC-11.4, MC-12.1

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

- [x] Host tests (`tests/uac_host.rs`, 6 passed): the STM-Speaker's own descriptor bytes; formats it cannot use left out; an endpoint that takes its rate by request marked; feedback read in 10.14 and 16.16; the pacer; one channel to and from two.
- [x] QEMU (`--suites usbaudio`): `usb-audio` attached in the codec's place, its OUT endpoint streams about 1950 packets while `beep` plays, the device hears the tone (its wav holds at least a quarter of a second of loud samples), and unplugged, the codec comes back.
- [ ] On the Ryzen, with the maintainer: the speakerphone plays `say` and a tone, and `listen` hears its microphones. The STM-Speaker's own counters (underruns, overruns, clock faults) stay at zero for 5 minutes.
  - **Met, 2026-11-21:** a tone, `say` and `listen`, each heard by the maintainer, on the second AMD xHCI.
  - **The board's counters, in part:** during a stream its count of playback frames lacked stands still, and overruns stay at 0. The maintainer watched it through a minute of notes (92 before, 92 throughout). It rises at each close of a stream, by 82 to 92 frames: 92 after a five-minute stream from zero, 174 after the minute that followed. That is the order in which `usb_host` closes a stream, [000-DRV-0081](000-DRV-0081-a-stream-is-closed-by-telling-the-device-first.md).
  - **Not yet:** a count of 0 after a five-minute stream and its close (with 000-DRV-0081), and the board's clock faults, which were not read.

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

  So the stop at the first control transfer is not MIND Core's alone: Linux and Windows on this machine show it too. Its cause is not settled. The STM-Speaker project answered that the board was short of power in these runs. 000-DRV-0075 keeps such a controller in use: it is set up again, and the device is left alone.
- **The controller's own set-up compared with Linux's** (debugfs on the Ryzen while Linux plays through the hub):
  - Linux: USBCMD `0x5` (INTE), IMAN `0x2` (IE), IMOD `0xA0`, CONFIG 64 slots, an event ring of 2 segments, MSI-X, its DMA through the IOMMU (AMD-Vi, addresses `0xFEDxxxxx`);
  - here: USBCMD `0x1`, IMAN 0, IMOD at its reset value, 16 slots, one segment, no interrupt, physical addresses.
- **The interrupter as Linux's made no difference.** Test release 202611201000 set IMOD `0xA0`, IMAN.IE and USBCMD.INTE; the first isochronous TD still stopped the controller (`USBSTS 00001019`).
- **The IOMMU was not it either.** With the controller's group in the identity domain Linux still played. What was left untried then is the section below.

## The Isoch TRB was written as type 17 (2026-11-20, evening)

**`usb_host/src/xhci.rs` had `TYPE_ISOCH = 17`.** In xHCI 1.2's table 6-91 the Isoch transfer TRB is type 5. Type 17 is the Reset Device command, which has no place on a transfer ring.

**What each controller did with it** (each is shown by its run with the fix: QEMU's wav, the DEXP's tone, the Ryzen's streams below):
- **QEMU 8.2** takes a TD by its endpoint's type and looks at a TRB's type only to gather its buffer. It ended each TD with success and moved no byte. That is the "no byte taken" this issue put down to QEMU.
- **The Ryzen's AMD xHCI** stopped with a host controller error at the first TD.
- **On the DEXP's Intel xHCI** (release 202611201739, the first isochronous stream there, to its own USB audio 0573:1573) everything on the controller stopped at `beep`, twice. Its keyboard, the cable, the Wi-Fi adapter and the boot stick are all on that controller, so the machine looked hung and no log of it was kept. Whether the controller set an error bit is not known.

**Why the comparison with Linux missed it.** Linux's debugfs and trace show a TRB decoded (`Isoch, IOC, ISP, TBC 0, TLBPC 0`). Our fields were compared with that text, never the raw control word with the specification's table. The interrupter, the IOMMU, TD pacing, the slot context and the cache flushes tried since were not the cause.

**The fix:** `TYPE_ISOCH = 5`. In QEMU `usb-audio`'s wav then holds `beep`'s tone (336,968 bytes where a bare header was), and the suite asserts that the device heard it.

**On the DEXP with the fix** (release 202611201916, 2026-11-20): a 3 s tone to its own USB audio, 0573:1573 on root port 6, and the machine went on:
```
59.100 usb_host: [USB] SLOT 5 INTERFACE 2 SETTING 1: ENDPOINT 01 STREAMS OUT (192 BYTES EVERY 1000 US); STATE 1, USBSTS 00000010
59.100 usb_host: [USB] SLOT 5 ENDPOINT 2: FIRST TD AT 647BE000: 647BF000 00000000 000000C0 4DC01421; MFINDEX 26A3; USBSTS 00000010
59.108 usb_host: [USB] SLOT 5 ENDPOINT 2: FIRST ISOCHRONOUS SEND DONE (192 BYTES)
63.079 usb_host: [USB] SLOT 5 ENDPOINT 2: STREAM ENDED: 3972 PACKETS SENT, 0 MISSED, 0 TIMES DRY, 0 WITH ERRORS
63.080 audio_gw: [AUDIO] USB PLAYBACK STOPPED: INTERFACE 2; 3999 PACKETS SENT, 999 OF THEM SILENCE, THE LAST RATE 48.000 FRAMES A MS
```
That shows the stream on an Intel xHCI. What was heard then was a rasp: the next section.

## A device with several rates is told which one comes (2026-11-21)

**The DEXP's own USB audio rasped.** With the stream running, the maintainer heard a rasp and crackle from 0573:1573 in place of the tone. In QEMU the same build's tone was clean: 439.7 Hz, 99.8 % of the power at 440 Hz.

**Its descriptors** (read on the DEXP): playback and capture each offer 8, 16 and 48 kHz, and both endpoints' class descriptors have the sampling frequency control (`07 25 01 01 …`). The gateway chose the setting because 48 kHz is among its rates, and never said which rate would come. The STM-Speaker and QEMU's `usb-audio` have one rate each, so nothing showed there.

**The change:**
- `uac::Setting` has `rated`: the data endpoint's class descriptor says it takes its sampling frequency by request.
- `audio_gw` sends SET_CUR of the sampling frequency (48000) to such an endpoint once its setting is selected, for playback and capture, as Linux's `init_sample_rate_v1`. It then asks the rate back and logs it.
- `usb_host` lets a client send a request to an endpoint of its interface's selected setting while that stream runs.

**On the DEXP** (release 202611210533, the ABI 34 tree with this change):
```
307.620 audio_gw: [AUDIO] USB ENDPOINT 01: 48000 HZ ASKED, THE DEVICE SAYS 48000 HZ
307.620 audio_gw: [AUDIO] USB PLAYBACK STARTED: INTERFACE 2 SETTING 1
```
Twelve rising notes over a minute (300 to 850 Hz, 5 s each): the maintainer heard all twelve, clean, from the lowest (`60972 PACKETS SENT, 0 MISSED, 0 TIMES DRY`).

**Not explained:** of the runs before that one, an 8 s run was not heard at all, and of a minute of four cycles two were heard. Their logs read as the last run's does. In the minute run the log was read through the cable 6 s in, and the ring ran dry twice then; that does not make half a minute of silence.

## The speakerphone on the DEXP, through the hub (2026-11-21)

Release 202611210533, the STM-Speaker's CN13 in the Genesys hub on the DEXP's root port 3. It is set up there (`CAFE:4014 ON PORT 3.2`, then 3.4), and the gateway takes it in place of the DEXP's own device (000-DRV-0080).

- **A tone:** six rising notes, 30 s, three times. The maintainer heard all six.
  ```
  852.913 usb_host: [USB] SLOT 13 INTERFACE 1 SETTING 1: ENDPOINT 01 STREAMS OUT (196 BYTES EVERY 1000 US); STATE 1, USBSTS 00000010
  852.913 usb_host: [USB] SLOT 13 INTERFACE 1 SETTING 1: ENDPOINT 82 STREAMS IN (3 BYTES EVERY 1000 US); STATE 1, USBSTS 00000010
  854.913 audio_gw: [AUDIO] USB PLAYBACK 2 S IN: 2022 PACKETS TAKEN (0 SILENCE), 121476 BYTES QUEUED, RATE 47.981 FRAMES A MS
  883.919 usb_host: [USB] SLOT 13 ENDPOINT 2: STREAM ENDED: 30999 PACKETS SENT, 0 MISSED, 0 TIMES DRY, 0 WITH ERRORS
  883.919 usb_host: [USB] SLOT 13 ENDPOINT 5: STREAM ENDED: 92994 BYTES, 0 MISSED, 0 TIMES DRY, 0 WITH ERRORS
  883.920 audio_gw: [AUDIO] USB PLAYBACK STOPPED: INTERFACE 1; 31004 PACKETS SENT, 998 OF THEM SILENCE, THE LAST RATE 48.001 FRAMES A MS
  ```
  The feedback endpoint is read each millisecond, and the pacer follows it (47.981 to 48.001 frames a millisecond).
- **`say`:** an English sentence, 3.6 s, heard (`4586 PACKETS SENT, 0 MISSED`).
- **`listen 6`:** the maintainer's speech recorded from its microphones and played back to him.
  ```
  2079.570 usb_host: [USB] SLOT 13 INTERFACE 2 SETTING 1: ENDPOINT 81 STREAMS IN (98 BYTES EVERY 1000 US); STATE 1, USBSTS 00000010
  2080.600 audio_gw: [AUDIO] CAPTURE: THE FIRST SECOND'S PEAK 2416 OF 32767
  2085.635 usb_host: [USB] SLOT 13 ENDPOINT 3: STREAM ENDED: 581180 BYTES, 0 MISSED, 0 TIMES DRY, 0 WITH ERRORS
  2085.635 audio_gw: [AUDIO] CAPTURE STOPPED: 6016 MS OF SOUND, PEAK 5510, 0 OVERFLOWS
  ```

**Straight in root port 3,** with the hub moved to port 2 (`CAFE:4014 ON PORT 3 (FULL SPEED)`, set up at the first try): the same six notes twice, heard, `30989 PACKETS SENT, 0 MISSED, 0 TIMES DRY` with the feedback read. So full-speed isochronous transfers run on an Intel root port without a hub between.

**Straight in root port 2,** the sockets changed back: set up at the first try, the six notes heard, `31007 PACKETS SENT, 0 MISSED`, and on the feedback endpoint `93018 BYTES, 0 MISSED, 1 TIMES DRY, 5 WITH ERRORS`. The five errors are the last five packets: the device stopped answering as the stream was closed (below, under what is still open).

This holds for the DEXP's Intel xHCI, through that hub and in its root ports 2 and 3 (MC-12.1). The acceptance criterion names the Ryzen and the speakerphone's own counters over 5 minutes; neither is covered by these runs.

## On the Ryzen's AMD xHCI (2026-11-21)

Release 202611201957: the stick's ABI 34 tree with the Isoch TRB's type fixed and the AMD gate taken off. The STM-Speaker straight in root port 1 of the second AMD xHCI (1022:1639, `XHCI 1`), after its board was reset and its CN13 cable changed (below). This is the controller that stopped with a host controller error at the first isochronous TD.

- **Six rising notes, 30 s:**
  ```
  41132.580 usb_host: [USB] XHCI 1: CAFE:4014 ON PORT 1 (FULL SPEED)
  41222.961 usb_host: [USB] XHCI 1: SLOT 2 INTERFACE 1 SETTING 1: ENDPOINT 01 STREAMS OUT (196 BYTES EVERY 1000 US); STATE 1, USBSTS 00000010
  41222.971 usb_host: [USB] XHCI 1: SLOT 2 ENDPOINT 2: FIRST ISOCHRONOUS SEND DONE (196 BYTES)
  41253.967 usb_host: [USB] XHCI 1: SLOT 2 ENDPOINT 2: STREAM ENDED: 30998 PACKETS SENT, 0 MISSED, 0 TIMES DRY, 0 WITH ERRORS
  41253.967 usb_host: [USB] XHCI 1: SLOT 2 ENDPOINT 5: STREAM ENDED: 92994 BYTES, 0 MISSED, 0 TIMES DRY, 0 WITH ERRORS
  ```
- **`say`:** an English sentence, 4.6 s (`5587 PACKETS SENT, 0 MISSED`).
- **`listen 8`** after three falling tones as the cue to speak: `RECORDED 384000 FRAMES (8000 MS), PEAK 7103, RMS 744, OVERFLOWS 0`. The maintainer heard the tones and his own voice played back. Three recordings before it held silence (peaks 7, 7 and 78): he had not spoken in time.
- **Five minutes of notes:**
  ```
  42094.150 usb_host: [USB] XHCI 1: SLOT 2 ENDPOINT 2: STREAM ENDED: 301043 PACKETS SENT, 0 MISSED, 0 TIMES DRY, 0 WITH ERRORS
  42094.150 usb_host: [USB] XHCI 1: SLOT 2 ENDPOINT 5: STREAM ENDED: 903129 BYTES, 0 MISSED, 0 TIMES DRY, 0 WITH ERRORS
  ```
  The maintainer heard the notes. A second run of five minutes, the board's counters started from zero: `301044 PACKETS SENT, 0 MISSED, 0 TIMES DRY, 0 WITH ERRORS`, and the board showed 92 frames lacked and 0 overrun after it (the acceptance criteria say where the 92 come from).

**The AMD gate is out of the code** with this: a controller of vendor 1022 takes streams as any other. One that stops with an error while it streams is still set up again and takes none afterwards (000-DRV-0075).

This holds for the Ryzen's second AMD xHCI with the device straight in its root port (MC-12.1). Through the hub there, and on the first AMD xHCI, the fixed build was not run.

**Before these runs, on the same port:** the speakerphone did not answer SET_ADDRESS in three tries, twice. After the board's reset button, while the board was starting, one try ended with a transaction error and the next left ADDRESS DEVICE unanswered for 5 s: the controller was set up again and the device left alone (000-DRV-0075), and camera 2 on that controller came back. With the cable changed and plugged in again, it was set up at the first try.

**Still open:**
- **The speakerphone stops answering, and why is not known.**
  - On the evening of 2026-11-20 it was not set up in the DEXP's root port 2 at all: SET_ADDRESS ended with a transaction error on every try (000-DRV-0079 has the log).
  - The next morning it was set up at the first try in that socket and played for 31 s without an error. At the request that closes the stream it stopped answering everything, all at once, with no disconnection: every request after it ended with completion 4.
  - Minutes later, straight in the Ryzen's second AMD xHCI, it did not answer SET_ADDRESS in three tries, twice.
  - After the board's reset and another CN13 cable it was set up there at the first try and ran every stream above. Both were changed at once, so which of them it was is not known.
  - Two explanations stand, neither tested: the board's firmware stopped serving USB (its project's reading), or a contact of a data line. Its project has it as task `19-usb-root-port-02`.

## Related

[000-DRV-0075](../issues-done/000-DRV-0075-a-controller-that-stops-answering.done) (an xHCI that stops answering is set up again), [000-DRV-0079](../issues-done/000-DRV-0079-a-further-try-at-a-device-resets-its-port.done) (a further try resets the port), [000-DRV-0080](../issues-done/000-DRV-0080-of-several-usb-audio-devices-the-one-set-up-last-plays.done) (the device set up last plays), [000-DRV-0081](000-DRV-0081-a-stream-is-closed-by-telling-the-device-first.md) (the close of a stream), [213-KRN-0127](213-KRN-0127-audio-gw-gets-a-second-hd-audio-function.md) (a second HD Audio function), [022](../issues-done/022-audio-tools-say-listen.done) (`say`, `listen`).
