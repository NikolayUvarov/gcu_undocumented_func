# DRV landing request: a USB audio endpoint is told its rate, the AMD gate out (000-DRV-0073); 000-DRV-0077 and 000-DRV-0080 closed

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-21 08:00 UTC · **Concerns:** 000-DRV-0073, 000-DRV-0077, 000-DRV-0080, 000-DRV-0081 · **Replaces:** `LANDING/2026-11-21T0657Z-DRV-ready-000-drv-0073-a-usb-audio-endpoint-told-its-rate-0080-closed.md`

**Ready:** `claude/DRV-ryzen-peripherals` 2489de5b3

## What it carries

It follows ed5e76b87, which is in round 162's candidate. On `main` 03c88c514; in `fast-test` 60578fccb.

- **000-DRV-0073: a USB audio endpoint that takes its rate by request is told 48 kHz** (09842ae14). The DEXP's own USB audio offers 8, 16 and 48 kHz and rasped until it was told. `libmind/src/uac.rs`, `audio_gw`, and `usb_host` (a client may address an endpoint of its interface's selected setting while that stream runs).
- **000-DRV-0073: the AMD gate out** (694695e83). An AMD xHCI takes isochronous streams as any other: the Ryzen's stop at the first TD was the Isoch TRB's type, fixed in ed5e76b87. The README says what `usb_host` and `audio_gw` do with USB audio.
- **000-DRV-0080: the counts of packets played are not carried to another device** (d6a630045), and the issue closed.
- **Text:**
  - 000-DRV-0077 closed (devices plugged in on the Ryzen);
  - 000-DRV-0081 opened (a stream closed by telling the device first);
  - 000-DRV-0073 and 000-DRV-0079 with the runs of 2026-11-21.
- **No ABI or IDL change.** `libmind` changes, so every program is built again.

## Tested

- **On LEG9:**
  - the tree 09842ae14: `build (x86)`, `host tests`, `build (x86 test programs)`, `x86: keys, shell, tools`, and the suites `services`, `audio`, `tts`, `listen`, `hda`, `usbaudio`, `link`, `modes` pass. `ahci` was left out (000-KRN-0267);
  - d6a630045: the build and `usbaudio`;
  - 694695e83: the build, `usbaudio` and `x86: keys, shell, tools`. The gate was for vendor 1022 alone, and QEMU's xHCI is not one: no suite runs the lines taken out.
  - What follows 694695e83 is issue text.
- **On the DEXP** (Intel xHCI, release 202611210533 with 09842ae14's change), the maintainer listening: its own 0573:1573 answers `48000 HZ` and plays twelve notes clean; the STM-Speaker plays notes and speech and records, through the hub and straight in two root ports.
- **On the Ryzen** (the second AMD xHCI, release 202611201957: its ABI 34 tree with the Isoch type fixed and the gate off), the STM-Speaker straight in root port 1, the maintainer listening:
  - notes, `say` and `listen` heard;
  - two streams of five minutes: `301043` and `301044 PACKETS SENT, 0 MISSED, 0 TIMES DRY, 0 WITH ERRORS`;
  - the board's own counter stood still through a watched stream and rose by 82 to 92 frames at each close: 000-DRV-0081.
- **Not tested:**
  - the rate request on the Ryzen (its release is older than that change; the speakerphone has one rate and is not asked);
  - the first AMD xHCI, and the speakerphone through the hub on the Ryzen, with the fixed build.
