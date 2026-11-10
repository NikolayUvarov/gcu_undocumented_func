# 252-APP-0128 — `tts` speaks Russian with the neural voice from `models:`, the formant voice its fallback

**Type:** tools (`tts`, voice) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (`tts`'s side made and run on QEMU with the asked grant applied locally; KRN's grant left, 2026-11-10) · **Blocked by:** [252-KRN-0230](../issues-done/252-KRN-0230-tts-reads-the-neural-voice.done) (`tts`'s scope of `models:` and its memory line) · **Main task:** [252](252-neural-speech-synthesis.md), step 5 · **Roadmap:** track G (voice) · **Constitution:** MC-8.1, MC-11.5, MC-11.11, MC-12.1

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

## Progress (2026-11-10): `tts`'s side, and a run with the grant applied locally

- **The way:** the first one, in `tts` itself, asked of KRN as a concrete step (`requests-KRN.md`). It leaves `nnd` for later: `tts`'s side is the same up to the synthesis call. NRL has not answered the question of 2026-11-09 yet.
- **`tts`** (`tts/src/neural.rs`, `route.rs`, `main.rs`):
  - **The build:** for the float target, as `speak`, with `libmind`'s heap.
  - **The load,** at the first Russian sentence, when it holds a read-only scope of `models:` in `SLOT_FILE`:
    - it probes the memory for `voice.bin`, `russian.dic` and 96 MiB of workspace first, as a failed heap allocation would end the service;
    - it reads both files through `mind::model_file`, which checks each against the disk's `MANIFEST.json`.
  - **Who speaks:** each sentence with a Cyrillic letter goes to Vosk TTS 0.7's speaker 3, resampled to 48 kHz. Any other sentence, a sentence the voice fails on, and every sentence with no voice loaded go to the formant voice as before.
  - **The log** says once which voice speaks Russian, and why.
- **`02_build.sh`** takes the newer of a crate's two builds, so `tts`'s old `x86_64-unknown-none` build is not copied any more.
- **Tests:**
  - **Host:** `tests/tts_host.rs`, the sentence's route.
  - **The `tts` suite,** without a model disk: the greeting's Russian is the formant voice's as before (5731 ms), and the log says `NO SCOPE OF models:`.
  - **The new `ttsneural` suite** (named only, `--tts-models-disk`), on QEMU x86, 4 CPUs, TCG. The disk held `voice.bin` (128 351 372 bytes, from `scripts/voice_dictate/convert.py`) and `russian.dic` (13 373 144 bytes, from `scripts/voice_tts/dictionary.py`), and `data/memory.conf` said `tts 320`. Run with init edited locally as the request asks, which is not committed:
    - `say Говорит разум корабля.` was the neural voice's;
    - the voice was ready in 23.9 s, and the whole took 176 s, for 1544 ms of speech: emulated, not a machine's times (MC-12.1);
    - from the audio captured at QEMU's AC'97, Vosk's small model and Vosk 0.54's transducer both heard `говорит разум корабля`.
- **With KRN's grant** (252-KRN-0230, in the landing queue): the `tts` suite's machine gets a model disk holding only its manifest. The suite takes either reason for the formant voice: no scope of `models:`, or no voice's file on such a disk. `git merge-tree` of the two branches has no conflict.
- **aarch64:** `tts` builds for `aarch64-unknown-none-softfloat` through its `.cargo/config.toml`, and the image's `normal` and `shell` suites pass on QEMU (4 CPUs, TCG), 2026-11-10. The voice is not run there.
- **Left:**
  - KRN's grant ([252-KRN-0230](../issues-done/252-KRN-0230-tts-reads-the-neural-voice.done)), then the same run on `main`'s `init`;
  - a voice control answer;
  - the latency of a later phrase;
  - aarch64, whose programs are soft-float, so emulated synthesis there would take far longer;
  - the MacBook Pro.

## Related

[252](252-neural-speech-synthesis.md), [252-APP-0041](252-APP-0041-russian-speech-on-the-macbook.md), [252-APP-0026](../issues-done/252-APP-0026-vits-voices-in-mind-nn.done), [252-APP-0027](../issues-done/252-APP-0027-russian-voice-speaks-in-the-system.done), [262](262-model-runtime-on-the-machines-full-hardware.md).
