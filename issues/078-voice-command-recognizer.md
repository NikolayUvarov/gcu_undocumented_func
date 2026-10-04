# 078 — Voice V1: offline command recognizer

**Type:** feature · **Owner:** tools track · **Priority:** P1 · **Status:** open · **Blocked by:** 077 · **Roadmap:** track G · **Constitution:** Art. 8.1–8.2, 11.11, MC-5

## Problem

Nothing turns speech into text. Voice control needs a recognizer of a fixed command grammar, in Russian and English, small enough for a 16 MiB program heap and fast enough on SSE2 ([docs/voice](../docs/voice/README.md) §4, V1).

## Plan

- Features: 40 log-mel bands (25 ms / 10 ms, 512-point FFT), per-utterance mean normalization; MFCC with deltas for the model.
- Phonemes: the synthesizer's letter-to-sound rules and lexicons move from `tts/` into a module shared by `tts` and the recognizer (no behaviour change for `tts`; its host tests guard that).
- Model: a small network (two convolutions, three dense layers, int8 weights, ≤ 3 MB) giving per-frame posteriors over the phoneme set plus silence; trained on the host by `scripts/voice_train.py` from speech synthesized by `tts` at several pitches and rates with noise; weights in `voice/model.bin` (header: version, phoneme set, feature parameters, checksum), loaded from the boot disk. Real recordings are a later addition (their licences go to `THIRD_PARTY.md`).
- Grammar: `voice/commands.txt` (phrases with slots, both languages) compiled into a phoneme graph; Viterbi decoding with a filler model for out-of-grammar speech; confidence from the best/second path; below the threshold the result is rejected.
- `hear [--wav file]` (console program, `REQUEST_CONSOLE`): records one utterance (077) and prints `HEARD "открой файлы" INTENT=open TOOL=fm CONFIDENCE=0.91` or `NOT UNDERSTOOD`.
- The recognizer gets only the audio client and reads its model and grammar through the application's read-only file client (Art. 11.11).

## Acceptance criteria

- Host test `tests/voice_host.rs`: every grammar phrase synthesized at three pitches and two rates with 20 dB SNR noise is recognized in ≥ 90 % of cases; 50 out-of-grammar phrases are rejected in ≥ 90 %; recognition of an 8 s utterance takes < 1 s on one host core with the kernel's build flags.
- QEMU: `hear --wav` on synthesized WAVs prints the right phrases.
- The model, its training script and its data provenance are in the repository; `THIRD_PARTY.md` covers anything not written here.

## Related

[077](077-voice-audio-front-end.md), [079](079-voice-control-in-the-shell.md), [017](../issues-done/017-tts-on-audio-gateway.done), [docs/voice](../docs/voice/README.md).
