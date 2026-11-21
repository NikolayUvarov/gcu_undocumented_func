# 551-APP-0149 — `beep`'s default sound: the opening of "Also sprach Zarathustra"

**Type:** tools (`beep`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** in progress · **Blocked by:** — · **Main task:** [551](551-sound-on-pcs.md) · **Roadmap:** track A · **Constitution:** MC-12.1

**Asked by:** the maintainer, 2026-11-21, through DRV (`APP/2026-11-21T0738Z-DRV-the-maintainer-asks-beeps-default-sound-as-the-opening-of-zarathustra.md` on `internal-dev-messages`):

> «дефолтный тестовый тон давай чуть поменяем на начальные ноты из "так говорил заратустра" - это подходит по идее нашей ос (не сейчас, это кажется вопрос трека APP поэтому просто поставь ему задачу)»

## Problem

`beep` without arguments plays the gateway's demo: three tones of 150 ms (523, 659 and 784 Hz), then a PCM sweep from 300 to 1200 Hz. The maintainer wants the default test sound to be the opening of Richard Strauss's "Also sprach Zarathustra" (Op. 30, 1896), which suits the idea of this system.

## Plan

- **The opening in one voice,** 2710 ms (`beep/src/notes.rs`):
  - the trumpets' C, G and C (262, 392 and 523 Hz; 400, 400 and 600 ms) as the gateway's own tones (`mind::audio::tone`);
  - then the chord's third falling from E to E flat (659 Hz for 150 ms, 622 Hz for 600 ms) as `beep`'s own samples (`mind::audio::play_all`);
  - then the timpani's C and G (131 and 98 Hz), five strokes of 80 ms, as samples too.

  So both of the gateway's paths still sound, as the demo's tones and sweep did.
- **Its line:** `[BEEP] PLAYED THE OPENING, 2710 MS`.
- **The suites that hear it** (`tests/qemu_smoke.py`): `audio`, `hda` and `tablet` check each note of the opening in the captured sound. Each note must be stronger than every other note, over 100 ms, since E and E flat lie 37 Hz apart, which 40 ms would not tell. They also check the timpani's low C, and `audio` checks the length. `usbaudio` and `tts` ask only for sound, which holds.
- **Rights:** only five pitches of the work are used, with no score or recording. The work (1896) is in the public domain, as its composer died in 1949. So no line is added to THIRD_PARTY.md, unless the maintainer says otherwise.

## Acceptance criteria

- `tests/beep_host.rs`: the opening's notes, their order and their length, each also taken as `beep`'s arguments.
- The `audio`, `hda`, `usbaudio`, `tts` and `tablet` suites pass in QEMU (x86_64), with the opening checked in the WAV.

## Related

[551](551-sound-on-pcs.md), [551-APP-0138](../issues-done/551-APP-0138-beep-left-and-right.done), u005 (`beep`).
