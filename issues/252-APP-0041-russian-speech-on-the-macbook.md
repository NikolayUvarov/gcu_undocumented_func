# 252-APP-0041 — Russian speech from `tts` on the MacBook Pro: measured, a cleaner upsampler

**Type:** tools (`tts`, voice) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (the source measured, nothing kept; the neural voice is the fix, 252-APP-0128; the Mac's run left, 2026-11-09) · **Blocked by:** — · **Main task:** [252](252-neural-speech-synthesis.md) · **Roadmap:** track G (voice) · **Constitution:** MC-11.5, MC-12.2

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
- **Still to do then:** the voice source (a return phase at the glottal closure, the source's tilt) and the Russian rules, measured the same way; the comparison with `speak` (252's neural voice); the run on the Mac.

## Progress (2026-11-09): the source measured with two recognizers, and the comparison with `speak`

- **The measurement is a script now:** `scripts/voice_tts/formant_asr.py`. It builds the audio with `tests/tts_host.rs` and transcribes it with one of two recognizers:
  - **Vosk's small Kaldi model** (`vosk-model-small-ru-0.22`), as above. It gives the table's line again: CER 35.6 %, WER 84.2 %, no clicks.
  - **Vosk 0.54's transducer** (`alphacep/vosk-model-ru`, int8, through sherpa-onnx), the recognizer 250 and 252 measured with. On the same voice: CER 40.5 %, WER 50.5 %.
- **The source,** each variant against the same 24 sentences. The bold lines were measured with both recognizers.

  | Variant | Small model CER / WER | Transducer CER / WER |
  |---|---|---|
  | **as in `main`** (tilt pole 3/4, open phase 60 %) | **35.6 / 84.2** | **40.5 / 50.5** |
  | less tilt: pole 1/2, 1/4, none | 39.0–39.5 / 89.3–90.7 | — |
  | **more tilt: pole 7/8** | **32.9 / 77.0** | **40.7 / 50.5** |
  | **pole 15/16** | **32.1 / 69.4** | **39.9 / 49.5** |
  | open phase 50 %, 70 % (pole 3/4) | 35.3 / 79.5, 35.9 / 84.4 | — |
  | pole 7/8 with open phase 50 %, 40 % | 33.1 / 75.7, 33.9 / 79.5 | — |
  | **voicing at 0.4 of its gain** (pole 3/4) | **34.0 / 71.3** | **41.1 / 50.0** |
  | voicing at 0.6, 0.25 | 34.8 / 80.1, 33.5 / 59.6 | — |
  | **rate 90 %, 80 %** | **41.5 / 97.0, 46.6 / 110.7** | **33.5 / 42.6, 30.1 / 42.6** |

- **What the two recognizers say together:**
  - **The source.** More tilt and less voicing look better to the small model only. Both turn the voicing down beside the noise sources, which are not tilted: the voiced level falls 1.5 to 8 dB. The transducer sees no change beyond one sentence's errors (39.9–41.1 % CER). None of them is kept.
  - **The rate.** The two disagree. The transducer gains about 8 WER points from a slower rate, and the small model loses 13 to 26. The default stays at 100 %. `say -r 85` slows it for a listener who wants it.
- **The comparison with `speak`,** 252's neural voice (Vosk TTS 0.7 through `mind::nn`, from the model disk): the same transducer on the same 24 sentences gave CER 0.80–1.75 % and WER 3.5–7.1 % (252's run of 2026-10-08, `scripts/voice_tts/results-2026-10-08.jsonl`), against 40.5 % and 50.5 % for the formant voice. These are two runs, with the same recognizer and the same sentences.
- **So,** to the task's question:
  - The formant voice's clicks are fixed.
  - Its intelligibility is not near usable, by either recognizer, and the cheap changes do not move it measurably.
  - **The fix is the neural voice where the model disk has it,** 252's step 5: `tts` speaks with it and keeps the formant synthesizer as the fallback. It is numbered as [252-APP-0128](252-APP-0128-tts-speaks-with-the-neural-voice.md).
- **Left:**
  - the Mac run, of `speak` beside `say`, with and without `-r 85`;
  - the Russian rules of the formant voice, for the machine without a model disk: measured with the transducer, the yardstick these runs show to be the steadier one.

## Related

[252](252-neural-speech-synthesis.md), [087](../issues-done/087-tts-idle-tone.done), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md), [252-APP-0027](../issues-done/252-APP-0027-russian-voice-speaks-in-the-system.done).
