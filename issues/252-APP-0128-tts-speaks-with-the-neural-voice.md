# 252-APP-0128 — `tts` speaks Russian with the neural voice from `models:`, the formant voice its fallback

**Type:** tools (`tts`, voice) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (the way to settle first) · **Blocked by:** — for the design; `KRN` for `tts`'s grants once it is chosen · **Main task:** [252](252-neural-speech-synthesis.md), step 5 · **Roadmap:** track G (voice) · **Constitution:** MC-8.1, MC-11.5, MC-11.11, MC-12.1

**Numbered by:** the tools track, 2026-11-09, from [252-APP-0041](252-APP-0041-russian-speech-on-the-macbook.md)'s measurements.

## Problem

- **What speaks today:** `say`, voice control's answers and every client of the `tts` service speak with the 16 kHz formant synthesizer.
- **How well it is understood** (252-APP-0041): on the 24 Russian sentences of `scripts/voice_tts/sentences.tsv`, Vosk 0.54's transducer understands it at CER 40.5 % and WER 50.5 %.
  - Its source, tilt, voicing balance and rate did not move that measurably, by the two recognizers together.
- **The neural voice** (Vosk TTS 0.7, through `mind::nn`) is understood at CER 0.80–1.75 % and WER 3.5–7.1 % by the same recognizer on the same sentences, in 252's run.
  - MIND Core already runs it, in `speak` (252-APP-0027), from the model disk.
  - No service uses it.

## Plan

252's step 5: `tts` speaks with the chosen voice from `models:`, and keeps the formant synthesizer as the fallback without a model disk.

**First, the way, to settle with `KRN` and `NRL`:**
- **In `tts` itself:**
  - `init` gives the service a read-only scope of `models:tts-ru-vosk-0.7/` and the memory the voice needs (`speak` asks for 320 MiB).
  - The service loads the voice once, at its first Russian phrase, and speaks with it.
- **Or through `nnd`,** when 262's stage 1 gives it the VITS family:
  - `tts` sends the phonemes' ids and receives the audio;
  - `nnd` holds the model and the memory, within its own bounds.

**Either way:**
- **The language:** Russian text takes the neural voice, and English the formant one until an English voice is chosen.
- **Without the model disk,** or when the voice cannot load, the formant voice speaks, and the log says why once.
- **No new interface for clients:** `tts.wit` stays as it is, unless the voice's choice needs a field.
- **The latency is measured,** for the first phrase and the next: voice control's answers wait on it.

## Acceptance criteria

- **QEMU, x86 and aarch64,** with the model disk: `say` and a voice control answer speak with the neural voice. The host's recognizer understands the system's phrases as in 252's comparison.
- **Without the model disk:** the formant voice, as before, and the log says so.
- **The latency** of the first and a later phrase is measured, and the configuration is named (MC-12.1).
- **The maintainer** hears it on the MacBook Pro.

## Related

[252](252-neural-speech-synthesis.md), [252-APP-0041](252-APP-0041-russian-speech-on-the-macbook.md), [252-APP-0026](../issues-done/252-APP-0026-vits-voices-in-mind-nn.done), [252-APP-0027](../issues-done/252-APP-0027-russian-voice-speaks-in-the-system.done), [262](262-model-runtime-on-the-machines-full-hardware.md).
