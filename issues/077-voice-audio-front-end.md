# 077 — Voice V0: audio front end (16 kHz mono, speech detection, WAV source)

**Type:** feature · **Owner:** tools track · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track G · **Constitution:** Art. 11.5, MC-5

## Problem

The system records the microphone (`audio_gw`, `listen`) but does nothing with the sound. Recognition needs a clean 16 kHz mono stream cut into utterances, and tests need a way to feed known speech without a microphone ([docs/voice](../docs/voice/README.md) §4, V0).

## Plan

- `mind::voice` (libmind, feature `alloc`): `Source` trait with `Microphone` (the audio client: `record-start/read/stop`, overflow count) and `Wav` (16-bit PCM WAV, any rate, mono or stereo); `Stream` downmixes and resamples to 16 kHz mono with a polyphase low-pass filter (no aliasing above 8 kHz).
- Speech detection: 25 ms frames every 10 ms; frame energy against an adaptive noise floor plus zero-crossing rate; start after 3 voiced frames, end after 200 ms below the threshold; utterances of 0.3–8 s, longer ones are cut; `Utterance { start_ms, samples, level }`.
- `listen --vad [seconds]` shows each detected utterance (start, length, level); `listen --wav <file>` reads a file instead of the microphone; `listen` without options behaves as before.
- Host test `tests/voice_host.rs`: resampler response (a 1 kHz tone passes, 10 kHz is attenuated ≥ 40 dB), detection on synthesized speech (the `tts` host build) with silence and white noise at 20 dB SNR: every phrase found once, bounds within 50 ms, nothing in pure noise.
- QEMU: a WAV made by the host `tts` on the boot disk; `listen --vad --wav` reports the expected number of utterances.

## Acceptance criteria

- The host test and the QEMU `listen` suite pass; the `listen` suite still checks microphone capture.
- `docs/voice` and the README describe `mind::voice` and `listen --vad/--wav`.

## Related

[docs/voice](../docs/voice/README.md), [022](../issues-done/022-audio-tools-say-listen.done), [078](078-voice-command-recognizer.md).
