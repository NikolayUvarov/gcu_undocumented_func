# 252-APP-0041 — Russian speech from `tts` on the MacBook Pro: measured, a cleaner upsampler

**Type:** tools (`tts`, voice) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [252](252-neural-speech-synthesis.md) · **Roadmap:** track G (voice) · **Constitution:** MC-11.5, MC-12.2

Numbered from the kernel track's request in `requests-APP.md` (2026-10-09), after the maintainer's run on the MacBook Pro: "Russian audio output is barely understandable, very poor, with clicks". The task is to find out whether it can be fixed.

## Problem

`tts` runs its 16 kHz formant synthesizer and upsamples to 48 kHz for `audio_gw`. On the Mac's speakers Russian is hard to follow, and there are clicks. The clicks are looked for in the driver as well ([551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md): polled playback without an interrupt). How intelligible the voice is, is the synthesizer's.

## Plan

1. Tell the synthesis from the upsampling, by measurement on the host.
2. The cheapest gains first: the upsampler's filter, then the voice source and the Russian rules, each measured with a recognizer before and after.
3. Name the larger step: 252's neural voice (`speak`), and whether `say` should use it where the model disk has it.

## Acceptance criteria

- A measured intelligibility before and after each change, on the same Russian sentences.
- On the Mac the maintainer understands a Russian test sentence without clicks (a run on the hardware).

## Progress

- **The upsampler (2026-10-09).** `tts` interpolated linearly from 16 to 48 kHz. That leaves the spectrum's images 20–32 dB below the speech between 8 and 16 kHz, measured on four Russian phrases (−25.6, −25.8, −20.4, −31.7 dB of the speech band), which laptop speakers play as a harsh, metallic edge, and it dulls the band (−1.8 dB at 4 kHz, −4.2 dB at 6 kHz). `tts` now upsamples with the filter `mind::voice` already uses for the opposite direction (`Resampler::between(16_000, 48_000, 1)`: Kaiser windowed sinc, 32 taps a phase, flat to 6 kHz, −1 dB at 7 kHz, 52 dB down from 8.5 kHz and 71 dB from 9 kHz), kept as an integer table in `tts/src/dsp.rs` so the service needs no heap or floating point. `tests/voice_host.rs` checks the table and its samples against the design, and that a 6 kHz tone keeps its level while its 10 kHz image is more than 70 dB down. A recognizer working at 16 kHz cannot see this change; it is measured on the spectrum.
- **Clicks in the synthesizer's output (2026-10-09).** The kernel track's note (fast-test's `requests-APP.md`) found onsets after the synthesizer's silences rising from 0 to 3000–5000 within two or three samples. The cause: `synth.rs` set the amplitudes of voicing, aspiration and frication once a frame (80 samples, 5 ms), so every amplitude change was a step at a frame's start, and a burst, a fricative or a voice after silence started at full level within a sample (a 25 ms attack or an 8 ms blend only made the steps smaller and later). The amplitudes now move from the last frame's to the new frame's over 2 ms (`RAMP`), and nothing else changes: the frame values and their timing are as before. A first try that moved them every sample along the segment's blend lowered intelligibility (CER 39.0 %), as it brought each change forward by part of a frame. `tests/tts_host.rs` `sounds_start_without_a_step` checks eight phrases (it fails on the old synthesizer: "кот", a step to 2395 at 63 ms).
- **Measured (host, the 24 Russian sentences of `scripts/voice_tts/sentences.tsv`, Vosk `vosk-model-small-ru-0.22` at 16 kHz, text normalized as in 250):**

  | Synthesizer | CER | WER | Onsets after ≥ 10 ms of silence reaching > 2000 within 3 samples |
  |---|---|---|---|
  | before | 37.0 % | 84.2 % | 223 |
  | amplitudes every sample along the blend (not kept) | 39.0 % | 89.1 % | 4 |
  | **amplitudes ramped over 2 ms from frame to frame** | **35.6 %** | **84.2 %** | **0** |
  | the same, and a nasal after a pause taking the 25 ms attack (not kept) | 35.8 % | 83.6 % | 0 |

  The small model makes a coarse yardstick: these differences in CER are about one sentence's errors. The command recognizer trained on this synthesizer still recognizes every phrase of its grammar (`tests/voice_host.rs`).
- **Still to do:** the voice source (a return phase at the glottal closure, the source's tilt) and the Russian rules, measured the same way; the comparison with `speak` (252's neural voice); the run on the Mac.

## Related

[252](252-neural-speech-synthesis.md), [087](../issues-done/087-tts-idle-tone.done), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md), [252-APP-0027](../issues-done/252-APP-0027-russian-voice-speaks-in-the-system.done).
