# DRV → APP: the maintainer asks for `beep`'s default sound to be the opening notes of "Also sprach Zarathustra"

**From:** DRV, passing on the maintainer's words · **To:** APP · **Concerns:** `beep` (u005), the test tone · **Answer wanted:** no; the task is yours to number

**The request.** The maintainer, 2026-11-21, while we listened to test tones on the DEXP and the Ryzen:

> «дефолтный тестовый тон давай чуть поменяем на начальные ноты из "так говорил заратустра" - это подходит по идее нашей ос (не сейчас, это кажется вопрос трека APP поэтому просто поставь ему задачу)»

In English: the default test tone should become the opening notes of "Also sprach Zarathustra", which suits the idea of this system. It is not urgent. He named APP as the track.

**What it touches, as far as I read it:**
- **`beep/src/main.rs`, `demo`:** `beep` without arguments now plays 523, 659 and 784 Hz for 150 ms each, then a sweep from 300 to 1200 Hz.
- **The opening** (Richard Strauss, Op. 30, 1896) is the rising C, G, C, then the major chord falling to minor (E to E flat). In one voice that is about 262, 392 and 523 Hz, then 659 and 622 Hz. The lengths and whether the timpani's C and G follow are yours to choose.
- **Tests that listen to the default sound** (`tests/qemu_smoke.py`):
  - the `audio` suite checks 523, 659 and 784 Hz in the captured sound, each stronger than the other two (`demo`'s tones, near line 10438);
  - the `usbaudio` suite, DRV's, asks only for a quarter of a second of loud samples after `run beep &`, so it holds for any tune of that length;
  - `hda` and `tts` start `run beep &` too.

**A note on rights:** the composer died in 1949, and only the notes are used, not a recording. Whether that needs a line in THIRD_PARTY.md is for you and the maintainer.

**Nothing on DRV's side waits for this.**
