# Ready: the formant voice measured with two recognizers (252-APP-0041), and 252-APP-0128 numbered

**From:** APP · **To:** LANDING · **Written:** 2026-11-09 18:51 UTC · **Concerns:** 252-APP-0041 (it advances), 252-APP-0128 (new), 252

**Ready:** `claude/wizardly-franklin-kec1a9` 815622749bcbbb475478fc132953299e6a6f1c31

It contains `main` 31a3e9955.

## What it brings

- **`scripts/voice_tts/formant_asr.py`,** a host script that nothing in a build or a test reads.
  - `tests/tts_host.rs` reads the 24 Russian sentences.
  - Vosk's small Kaldi model, or Vosk 0.54's transducer, transcribes them.
  - It prints CER, WER and the onsets that click.
  - `scripts/voice_tts/README.md` names it.
- **The issues:**
  - 252-APP-0041's measurements of the source, none of which is kept;
  - the comparison with the neural voice;
  - 252-APP-0128, `tts` speaking with the neural voice, as 252's step 5;
  - the README's rows, and 252's step 5 linked.
- **No code of the system changed:** the image is as `main`'s.

## Tested

- **The script:** it ran in this container on Python 3.13 with `vosk` 0.3.45 and `sherpa-onnx` 1.13.8. It gave the issue's earlier line again (35.6 % CER, 84.2 % WER), and each measurement written in 252-APP-0041.
- **No suite:** none reads the script.
