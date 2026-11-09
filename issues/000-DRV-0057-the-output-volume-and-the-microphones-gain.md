# 000-DRV-0057 — The output volume and the microphone's gain in `audio.wit`

**Type:** driver, interface · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (2026-11-09) · **Blocked by:** — (the shell's levels badge is in `main` already: `mind::audio::BADGE_LEVELS`, which `init` gives the shell alone, 000-KRN-0118) · **Main task:** [000-APP-0048](000-APP-0048-wm-settings.md), for [000-APP-0076](000-APP-0076-output-volume.md) and [000-APP-0077](000-APP-0077-microphone-sensitivity.md) · **Constitution:** MC-3.11, MC-11.5, MC-12.4

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
   - `set-volume` and `set-gain` only for a client badged `BADGE_LEVELS`, which `init` gives the shell alone (000-KRN-0118); any other client gets `rights`;
   - `levels` answers every client.
4. **Not kept by `audio_gw`:** the shell keeps the levels in `data/sound.conf` and sets them again when it starts (000-APP-0076).

## Acceptance criteria

- **The `audio` suite,** AC'97 and HDA:
  - `set-volume(50)` lowers the level QEMU's audio backend records for the same tone, by about 6 dB or by the codec's step;
  - `set-gain` reads back through `levels`, and raises the captured level where QEMU's capture has an input;
  - a client without the badge gets `rights`.
- **On the MacBook Pro:** the speaker's volume follows `set-volume`, and the internal microphone's level follows `set-gain`. That needs the maintainer's ears and [551-DRV-0056](551-DRV-0056-the-macbook-pros-jack-and-internal-microphone.md).

## Progress (2026-11-09)

**Made** (on `claude/ASR-DRV`):
- **`audio.wit` 1.2:**
  - the records `range` (settings, quarter dB a step, the 0 dB setting, `scaled`) and `levels`;
  - `levels` for every client;
  - `set-volume(percent, muted)` and `set-gain(percent, boost)`, which answer the levels as set.

  The binding is regenerated, and `range` and `levels` are in `idl_fuzz_host`'s list.
- **The authority:** only the client badged `BADGE_LEVELS`, which `init` gives the shell alone (000-KRN-0118), sets the levels. Any other client gets `rights`, logged as `[AUDIO] LEVELS: PID <n> HAS NO LEVELS BADGE: REFUSED`. Above 100 % is `invalid`, and without a device `not-found`.
- **AC'97:**
  - the master volume (0x02), whose width is found at start: 5 bits, or 6, as QEMU's has;
  - mute in its bit 15;
  - the record gain (0x1C), 16 settings of 1.5 dB from 0 dB;
  - the microphone's +20 dB boost (0x0E bit 6).

  The PCM output stays at 0 dB.
- **HD Audio:**
  - **the volume:** the output path's amplifier nearest the DAC that has settings, with its mute where it has one;
  - **the gain:** the input path's amplifier nearest the ADC;
  - **the boost:** the input pin's amplifier, where it rises above 0 dB (else it is the gain);
  - **without an amplifier,** the samples are scaled, 1.5 dB a setting: down to -46.5 dB on the output, up to +22.5 dB on the capture. `levels` says `scaled`;
  - **the capabilities** are now the widget's own or the function group's, as its override bit says (the 0 dB unmute too).
- **At start** the gateway logs the levels it begins with, both sides at 0 dB as before, and logs each change.
- **libmind:** `mind::audio::levels`, `set_volume`, `set_gain`.

**Tested:**
- **`tests/audio_levels_host.rs`** (5 tests):
  - per cent and settings both ways;
  - the quarter dB;
  - HD Audio's capabilities and the gain verb;
  - AC'97's registers;
  - the scaled factors, which are -6 dB at 0.501 and clip.
- **The whole `scripts/host_tests.sh`:** 197 suites, none failed (the IDL fuzz included).
- **`scripts/ci_local.sh --only x86`, `x86: services, storage, audio`:**
  - **AC'97:** the levels at start, the master's 64 settings, and `tests/levels_stub` refused twice with the plain client (`levels` read through it);
  - **HDA** (QEMU's hda-duplex): the volume on 75 settings and the gain on 75, no boost;
  - tts, listen, link and modes as before.

**Not tested yet:**
- that `set-volume(50)` lowers the recorded tone and `set-gain` raises the capture in QEMU. That needs a caller with the badge, and only the shell has one: APP's `volume` and `mic` (000-APP-0076, 0077) bring that test;
- the MacBook Pro's speaker and microphone (551-DRV-0056).

## Related

[000-APP-0076](000-APP-0076-output-volume.md), [000-APP-0077](000-APP-0077-microphone-sensitivity.md), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md).
