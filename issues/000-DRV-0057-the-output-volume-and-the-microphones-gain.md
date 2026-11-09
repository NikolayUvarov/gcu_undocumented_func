# 000-DRV-0057 — The output volume and the microphone's gain in `audio.wit`

**Type:** driver, interface · **Owner:** `DRV` · **Priority:** P1 · **Status:** open · **Blocked by:** — (the shell's levels badge from `init` is KRN's, to be asked when this starts) · **Main task:** [000-APP-0048](000-APP-0048-wm-settings.md), for [000-APP-0076](000-APP-0076-output-volume.md) and [000-APP-0077](000-APP-0077-microphone-sensitivity.md) · **Constitution:** MC-3.11, MC-11.5, MC-12.4

Numbered from the tools track's request of 2026-10-10, widened on 2026-10-14, in `requests-DRV.md`. The maintainer: "the output volume and the microphone's sensitivity need settings".

## Problem

`audio.wit` 1.1 plays, records and makes tones, but has no level:
- **Output.** `audio_gw` leaves AC'97's master volume at 0 dB and its PCM output at the default, and unmutes HDA's output amplifiers at 0 dB. A program can only scale its own samples.
- **Capture.** The gain is fixed at start:
  - AC'97's microphone at 0 dB, without its +20 dB boost;
  - the record gain at 0 dB;
  - HDA's input amplifiers at 0 dB.
- **So** neither the shell nor `wm`'s Settings can set how loud the sound is or how sensitive the microphone is.

## Plan

1. **`audio.wit` 1.2:**
   - `levels`: the output's level in per cent and whether it is muted; the capture's gain in per cent and the boost on or off; each with its range in dB where the device has one;
   - `set-volume(percent, muted)` and `set-gain(percent, boost)`.
2. **On the device:**
   - **AC'97:** the master volume (0x02), the PCM output (0x18), the record gain (0x1C) and the microphone's boost (0x0E bit 6);
   - **HDA:** the output path's amplifier gain and mute; the input path's amplifier, the ADC's gain and the pin's boost where the codec has them;
   - **without an amplifier,** `audio_gw` scales the samples, and `levels` says so.
3. **Authority:**
   - `set-volume` and `set-gain` only for a client with the badge `init` gives the shell alone; any other client gets `rights`;
   - `levels` answers every client.
4. **Not kept by `audio_gw`:** the shell keeps the levels in `data/sound.conf` and sets them again when it starts (000-APP-0076).

## Acceptance criteria

- **The `audio` suite,** AC'97 and HDA:
  - `set-volume(50)` lowers the level QEMU's audio backend records for the same tone, by about 6 dB or by the codec's step;
  - `set-gain` reads back through `levels`, and raises the captured level where QEMU's capture has an input;
  - a client without the badge gets `rights`.
- **On the MacBook Pro:** the speaker's volume follows `set-volume`, and the internal microphone's level follows `set-gain`. That needs the maintainer's ears and [551-DRV-0056](551-DRV-0056-the-macbook-pros-jack-and-internal-microphone.md).

## Related

[000-APP-0076](000-APP-0076-output-volume.md), [000-APP-0077](000-APP-0077-microphone-sensitivity.md), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md).
