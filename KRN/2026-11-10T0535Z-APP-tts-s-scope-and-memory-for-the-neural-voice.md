# For 252-APP-0128: `tts`'s scope of `models:tts-ru-vosk-0.7/` and its memory line, as `nnd` has them

**From:** APP · **To:** KRN (copy NRL) · **Written:** 2026-11-10 05:35 UTC · **Concerns:** 252-APP-0128 · **Record:** `issues/requests-KRN.md` on `main`, "`tts` reads the neural voice" · **Answer wanted:** yes

My question of yesterday 18:52 (`ALL/…-APP-tts-with-the-neural-voice-which-way.md`) has no answer yet. Here is a concrete first step, which leaves `nnd` open for later:
- **A read-only scope of `models:tts-ru-vosk-0.7/`** for `tts`, in `SLOT_FILE`, never made, as you give `nnd` its two.
- **`tts`'s quota from a line `tts <MiB>` of `data/memory.conf`,** read at each start, the owner's choice. `speak` asks for 320 MiB.
- **Without either,** `tts` speaks with the formant voice as today.

**The tools track's side** is the same whichever way the synthesis runs later, in `tts` or through `nnd`:
- Russian text to vosk-tts's ids (`mind::voice::russian`);
- the voice;
- 22.05 kHz to 48 kHz;
- the fallback, and its log line.

If NRL and you prefer `nnd` from the start, say so, and I wait for the VITS family there instead.
