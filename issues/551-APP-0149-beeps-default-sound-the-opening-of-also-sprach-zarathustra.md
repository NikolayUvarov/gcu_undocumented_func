# 551-APP-0149 — `beep`'s default sound: the opening of "Also sprach Zarathustra"

**Type:** tools (`beep`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** in progress (the suites in QEMU) · **Blocked by:** — · **Main task:** [551](551-sound-on-pcs.md) · **Roadmap:** track A · **Constitution:** MC-12.1

**Asked by:** the maintainer, 2026-11-21, through DRV (`APP/2026-11-21T0738Z-DRV-the-maintainer-asks-beeps-default-sound-as-the-opening-of-zarathustra.md` on `internal-dev-messages`):

> «дефолтный тестовый тон давай чуть поменяем на начальные ноты из "так говорил заратустра" - это подходит по идее нашей ос (не сейчас, это кажется вопрос трека APP поэтому просто поставь ему задачу)»

## Problem

`beep` without arguments plays the gateway's demo: three tones of 150 ms (523, 659 and 784 Hz), then a PCM sweep from 300 to 1200 Hz. The maintainer wants the default test sound to be the opening of Richard Strauss's "Also sprach Zarathustra" (Op. 30, 1896), which suits the idea of this system.

## Plan

- **The opening in one voice,** 2710 ms (`beep/src/notes.rs`), all as `beep`'s own samples (`mind::audio::play_all`, which waits for room in the gateway's ring):
  - the trumpets' C, G and C (262, 392 and 523 Hz; 400, 400 and 600 ms);
  - the chord's third falling from E to E flat (659 Hz for 150 ms, 622 Hz for 600 ms);
  - the timpani's C and G (131 and 98 Hz), five strokes of 80 ms.
- **Not the gateway's tones:** `mind::audio::tone` takes only what the gateway's ring holds at once (0.68 s) and does not wait. A first try played C, G and C that way, and the captured sound held C whole, G cut short and the high C not at all. The gateway's tone path stays checked in the same `audio` suite by `wm`'s Sound page tone (880 Hz, 150 ms).
- **Its line:** `[BEEP] PLAYED THE OPENING, 2710 MS`.
- **The suites that hear it** (`tests/qemu_smoke.py`): `audio`, `hda` and `tablet` check the opening's notes in the captured sound, in their order: in 40 ms windows every 50 ms, the strongest of the opening's notes. Each of C, G, C, E and E flat holds for 100 ms or more, and the timpani's C and G follow. They check the order, not the times: QEMU's HD Audio stream stalls a moment now and then, some 50 ms in all, which shifts what follows. `audio` also checks the length. `usbaudio` and `tts` ask only for sound, which holds.
- **Rights:** only five pitches of the work are used, with no score or recording. The work (1896) is in the public domain, as its composer died in 1949. So no line is added to THIRD_PARTY.md, unless the maintainer says otherwise.

## Acceptance criteria

- `tests/beep_host.rs`: the opening's notes, their order and their length, each also taken as `beep`'s arguments.
- The `audio`, `hda`, `usbaudio`, `tts` and `tablet` suites pass in QEMU (x86_64), with the opening checked in the WAV.

## Related

[551](551-sound-on-pcs.md), [551-APP-0138](../issues-done/551-APP-0138-beep-left-and-right.done), u005 (`beep`).
