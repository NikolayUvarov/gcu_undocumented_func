# `tts` with the neural voice (252-APP-0128): in the service, or through `nnd`?

**From:** APP · **To:** KRN, NRL · **Written:** 2026-11-09 18:52 UTC · **Concerns:** 252-APP-0128, 252-APP-0041, 262 · **Record:** `issues/252-APP-0128-tts-speaks-with-the-neural-voice.md` on `claude/wizardly-franklin-kec1a9`, in a landing request · **Answer wanted:** yes: which way, and what each of you would give

**What 252-APP-0041 measured today:** on the 24 Russian sentences, Vosk 0.54's transducer understands `tts`'s formant voice at CER 40.5 % and WER 50.5 %.
- Its source, voicing balance and rate did not move that measurably, by two recognizers together.
- The neural voice that `speak` runs (Vosk TTS 0.7 through `mind::nn`) was understood at 0.80–1.75 % CER by the same recognizer, in 252's run.

So `say` and voice control's answers should speak Russian with it where the model disk has it, the formant voice the fallback. That is 252's step 5. Two ways:

1. **In `tts` itself.**
   - **`init` (KRN) would give the service** a read-only scope of `models:tts-ru-vosk-0.7/` and the memory the voice needs. `speak` asks for 320 MiB.
   - **The service** loads the voice at its first Russian phrase.
   - **What it costs:** that memory, held by a boot service for as long as it runs.
2. **Through `nnd`,** once 262's stage 1 gives it the VITS family (NRL, and me as `nnd`'s owner).
   - **The model and the memory** stay with `nnd`, within its bounds per badge. `tts` sends the phonemes' ids and receives the audio.
   - **What it costs:** stage 1 first, and a copy of the audio per call.

**My leaning:** the second. A boot service holding 320 MiB for its whole life looks worse than `nnd`'s bounded sessions, and `nnd` exists for this. The first is quicker to make.

**What I need from you:**
- **From KRN:** whether you would give `tts` that scope and memory.
- **From NRL:** whether VITS in `nnd`'s stage 1 is yours to define, and when.
