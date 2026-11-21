# DRV landing request: a USB audio endpoint is told its rate (000-DRV-0073); 000-DRV-0080 closed

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-21 06:57 UTC · **Concerns:** 000-DRV-0073, 000-DRV-0080, 000-DRV-0079

**Ready:** `claude/DRV-ryzen-peripherals` ee9c41c01

## What it carries

It follows ed5e76b87, which is in round 162's candidate: this is what came after it, for the next round. On `main` 03c88c514; in `fast-test` db80da50b.

- **000-DRV-0073: a USB audio endpoint that takes its rate by request is told 48 kHz** (09842ae14).
  - **Why:** the DEXP's own USB audio (0573:1573) offers 8, 16 and 48 kHz. The gateway chose its setting because 48 kHz is among them and never said which rate would come. The maintainer heard a rasp in place of the tone.
  - `libmind/src/uac.rs`: `Setting` marks such an endpoint from its class descriptor.
  - `audio_gw`: SET_CUR of the sampling frequency once the setting is selected, as Linux's `init_sample_rate_v1`; the rate the device then names goes to the log.
  - `usb_host`: a client may address an endpoint of its interface's selected setting while that stream runs.
- **000-DRV-0080: the counts of packets played are not carried to another device** (d6a630045), and the issue closed.
- **Text:** the three issues with the DEXP's runs of 2026-11-21.
- **No ABI or IDL change.** `libmind` changes, so every program is built again.

## Tested

- **On LEG9, the tree 09842ae14:**
  - `build (x86)`, `host tests` (`tests/uac_host.rs` has the new case), `build (x86 test programs)` and `x86: keys, shell, tools` pass;
  - the suites `services`, `audio`, `tts`, `listen`, `hda`, `usbaudio`, `link` and `modes` pass. `ahci` was left out: it fails on LEG9 on `main` (000-KRN-0267).
- **On LEG9, d6a630045** (the counts): the build and the suite `usbaudio` pass. ee9c41c01 adds issue text only.
- **On the DEXP** (release 202611210533, its ABI 34 tree with 09842ae14's change), the maintainer listening:
  - its own 0573:1573: `48000 HZ ASKED, THE DEVICE SAYS 48000 HZ`, twelve rising notes over a minute heard clean, `60972 PACKETS SENT, 0 MISSED, 0 TIMES DRY`;
  - the STM-Speaker through the hub and straight in root ports 3 and 2: notes, `say` and `listen` heard;
  - the speakerphone plugged in played in place of 0573:1573, which played again once it was unplugged.
- **Not tested:**
  - QEMU's `usb-audio` has one rate and no such control, so no suite sends the request; the parser's part is a host test;
  - the Ryzen's AMD xHCI. The speakerphone does not answer there today (its own fault or its cable's, not known), so the AMD gate stays in.
