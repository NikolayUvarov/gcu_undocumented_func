# 087 — `tts`: a quiet tone stays after every phrase

**Type:** bug · **Owner:** tools track · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Roadmap:** track G · **Constitution:** —

## Problem

After the last sound of a phrase the synthesizer does not fall silent: it keeps producing a periodic signal of about RMS 55 (−55 dBFS), a tone near 1.6 kHz (period of 10 samples at 16 kHz), until the phrase's samples end. `tests/voice_host.rs` found it while testing speech detection (issue 077): the tail of "hello world" is 300 ms of `−66, −84, −63, −43, −52, −74, −67, −28, −2, −22, …` repeating. On a loudspeaker it is barely audible; to a speech detector working down to digital silence it is sound, which is why `mind::voice::Detector` ignores everything under −50 dBFS.

The likely cause is a limit cycle of the fixed-point filters: `Resonator::run` and the DC blocker in `synth.rs` (`self.dc_y * 4064 >> 12`) round their feedback with an arithmetic shift, which rounds toward minus infinity, so a small state never decays to zero (the same fault the voice front end had in its DC blocker and fixed by dividing, which rounds toward zero). The frication resonator also keeps running with input 0 while `af` is 0.

## Plan

- Find which filter cycles (log each resonator's state in the tail of a phrase on the host).
- Round the feedback paths toward zero (or to nearest) in `Resonator`, `Antiresonator` and the DC blocker; clear the frication resonator's state when frication stops; if a cycle remains, reset the filters' state once the amplitudes reach 0 in a pause.
- Keep the voice unchanged otherwise: the formant targets, timing and levels stay as they are.

## Acceptance criteria

- A host test synthesizes phrases in both languages and checks that within 50 ms after the last segment with non-zero amplitude the output is digital silence (all samples 0), and that pauses between words are silent the same way.
- `tests/tts_host.rs` and the QEMU `tts` suite pass; with `--asr-model` the recognized share of words does not drop.
- `tests/voice_host.rs` still passes; the detector's −50 dBFS floor may then be lowered (a separate change, measured on real recordings).

## Related

[017](../issues-done/017-tts-on-audio-gateway.done), [077](../issues-done/077-voice-audio-front-end.done).
