# 252 — Neural speech synthesis: a compact voice and a quality voice for Russian and English

**Type:** main task, tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G (voice) · **Constitution:** MC-8.1, MC-11.11, MC-12.1, MC-12.2

## Problem

`tts`, our formant synthesizer, is small and clear enough for short answers. An offline recognizer understands about 68 % of its Russian words and 53 % of its English ones ([docs/voice](../docs/voice/README.md)). On 2026-10-08 the maintainer asked for voices that sound good, chosen as follows:

- two variants for each language: `compact`, optimized for size, and `quality`, optimized for pronunciation;
- no model that does badly;
- any free licence;
- the models kept in the model cache of [251](251-model-cache-and-model-disk.md).

## Comparison (2026-10-08)

### Method

- **Sentences:** 24 per language ([scripts/voice_tts](../scripts/voice_tts/README.md)).
  - Four for listening: two of the system's own phrases, one with homographs, and one FLEURS sentence.
  - Twenty FLEURS test sentences (CC BY 4.0) without digits.
  - The slowest models read fewer. Chatterbox, and Qwen3-TTS in English, read the four listening sentences and the first four FLEURS ones. ESpeech read the four listening sentences only.
  - So the tables also give every voice's figures on these eight common sentences.
- **Intelligibility:** the best recognizers of [250](250-voice-dictation.md) transcribe the synthesized speech: Vosk 0.54 for Russian, Parakeet TDT-CTC 110M for English. CER and WER are counted against the text, normalized as in 250.
  - Lower means clearer. The figure also includes the recognizer's own errors, so it compares voices with each other and is not an absolute measure.
  - The eight sentences are about 800 characters, so a difference of 0.3 points is two or three characters.
- **Naturalness:** UTMOSv2, a predicted listener score from 1 to 5.
  - It was trained on English ratings, and across model families its order is doubtful. It puts Piper's lessac (3.76) above every Kokoro voice (2.30–2.99), the reverse of the order listeners gave in the public TTS Arena.
  - So the choice is made by ear, on a listening page, and UTMOS only orders voices of one family.
- **Speed:** time per second of speech on this host (a 4-core Xeon at 2.1 GHz). The ONNX voices run on one thread, the PyTorch ones on four.
- **Runtimes:**
  - sherpa-onnx 1.13.8 for Piper, Kokoro and Kitten;
  - vosk-tts 0.3.61 for Vosk TTS;
  - PyTorch on the CPU for Pocket TTS, ESpeech, Qwen3-TTS and Chatterbox.
- **Cloned voices:** ESpeech and Qwen3-TTS copy the voice of a FLEURS recording, for this comparison only. A system voice needs its own recording, made with the speaker's consent.
- **What the numbers mean:** how the published models do on these sentences on this host. They are not evidence about MIND Core (MC-12.1, MC-12.2).

### Russian

Size is what the runtime loads. CER is in %: "24" on all 24 sentences, "8" on the eight common ones. UTMOS is on the eight common sentences. Time is seconds per second of speech, with the threads used in brackets.

| Voice | Kind | Size | CER 24 | CER 8 | UTMOS 8 | Time | Licence: weights; training data |
|---|---|---|---|---|---|---|---|
| Piper `irina` (medium) | VITS | 63 MB | 1.16 | 0.65 | 3.37 | 0.13 (1) | MIT; RHVoice recordings, licence not stated |
| Piper `dmitri` (medium) | VITS | 63 MB | 1.59 | 1.17 | 2.73 | 0.10 (1) | MIT; CC0 |
| Piper `denis` (medium) | VITS | 63 MB | 2.59 | 3.25 | 2.64 | 0.12 (1) | MIT; CC0 |
| Piper `ruslan` (medium) | VITS | 63 MB | 2.31 | 2.21 | 2.60 | 0.11 (1) | MIT; RUSLAN corpus, CC BY-NC-SA 4.0 |
| Vosk TTS 0.7, voices 0–4 | VITS-like, five speakers, its own phonemes and stress dictionary | 247 MB | 0.80–1.75 (voice 3: 0.80) | 1.17–1.95 | 2.84–3.36 | 0.06 (1) | Apache-2.0 |
| **Vosk TTS 0.9**, voices 0–4 | a duration predictor and a diffusion transformer (DiT), BERT (626 MB of the 937) for prosody, five speakers | 937 MB | 0.24–1.24 (voice 3: **0.24**, 2: 0.32, 4: 0.36) | 0.00–1.43 | 2.80–3.31 | 0.27–0.33 (1) | Apache-2.0 |
| ESpeech TTS-1 RL-V2 | F5-TTS (flow matching) and RUAccent for stress; a cloned voice | 2.7 GB | — | 0.99 (4 sentences) | 2.87 | 26.4 (4) | Apache-2.0 |
| Qwen3-TTS 0.6B Base | a language model over sound tokens; a cloned voice | 2.5 GB | 0.60 | 0.91 | 3.03 | 5.3 (4) | Apache-2.0 |
| Chatterbox Multilingual | a language model over sound tokens, 23 languages | 3.0 GB | — | 2.34 | 2.99 | 4.7 (4) | MIT |

### English

| Voice | Kind | Size | CER 24 | CER 8 | UTMOS 8 | Time | Licence: weights; training data |
|---|---|---|---|---|---|---|---|
| **Kitten TTS nano 0.2** (fp16), voices 0 and 1 | StyleTTS-like, 15M parameters | 24 MB | 1.10, 0.98 | 0.65, 0.65 | 2.48, 1.92 | 0.30 (1) | Apache-2.0 |
| Kitten TTS mini 0.1, voices 0 and 1 | StyleTTS-like, 80M parameters | 166 MB | 0.80, 0.87 | 1.17, 0.91 | 2.91, 3.15 | 1.37 (1) | Apache-2.0 |
| Piper `lessac` (high) | VITS | 114 MB | 1.36 | 2.08 | 3.76 | 0.73 (1) | MIT; Blizzard 2013 Lessac, a research licence |
| Piper `ryan` (high) | VITS | 121 MB | 1.29 | 0.52 | 3.52 | 0.69 (1) | MIT; RyanSpeech, CC BY-NC-SA 4.0 |
| **Kokoro-82M v1.0** (fp32): `af_heart`, `af_bella`, `am_michael`, `bf_emma` | StyleTTS 2 with an iSTFTNet decoder | 354 MB | 0.76, 0.68, 0.72, 0.91 | 0.65, 0.52, 0.39, 1.82 | 2.90, 2.30, 2.99, 2.92 | 0.86 (1) | Apache-2.0 |
| Pocket TTS, voice `alba` | Kyutai, 100M parameters | 219 MB | 0.87 | 0.65 | 3.17 | 0.62 (4) | CC BY 4.0 |
| Qwen3-TTS 0.6B Base | as in Russian; a cloned voice | 2.5 GB | — | 1.17 | 1.96 | 5.3 (4, from the Russian run) | Apache-2.0 |
| Chatterbox Multilingual | as in Russian | 3.0 GB | — | 1.30 | 3.37 | 4.7 (4) | MIT |

### Findings

- **Every neural voice is far clearer than `tts`.**
  - Their word error rates here are 0–11 %.
  - Vosk recognizes about 68 % of the Russian words and 53 % of the English words of our formant synthesizer in its own test. That test used other sentences and another recognizer, so the comparison is rough.
- **Russian:**
  - Vosk TTS 0.9 is the clearest. Voices 2–4 make 0.24–0.36 % CER on 24 sentences, at 0.3 s per second of speech on one core, but the model takes 937 MB.
  - The clearest small voice is Piper's `irina`: 1.16 %, 63 MB, 0.13 s. The licence of its recordings (RHVoice) is not stated.
  - Vosk TTS 0.7 is the fastest: 0.06 s, 247 MB, Apache-2.0. Its voices 3 and 1 make 0.80 and 0.88 %.
  - Piper's `denis` and `ruslan` (2.3–2.6 %) and Chatterbox (2.34 %) are the least clear.
    - Chatterbox said «курс положен» for «курс проложен» and «файлов иминеджер» for «файловый менеджер».
    - Of these, the maintainer's rule excludes Piper's `denis` and `ruslan` and Chatterbox in Russian.
- **English:**
  - Kokoro is the clearest family: 0.68–0.91 % on 24 sentences, 354 MB in fp32, 0.86 s on one core.
  - Kitten TTS nano makes 1.0–1.1 % with 24 MB at 0.30 s.
  - Pocket TTS makes 0.87 %, at 0.62 s on four threads.
  - Piper's voices make about 1.3 %, and their recordings are licensed for research or non-commercial use.
  - Kitten mini is slower than real time on one core (1.37 s per second of speech), so it is excluded.
- **The large models are too slow for a CPU.** Qwen3-TTS, Chatterbox and ESpeech need 5–26 s per second of speech on four threads.
  - They are no clearer than Vosk TTS 0.9 or Kokoro.
  - Chatterbox's English repeats sounds: "spepeaking", "offffice".
  - They stay on the listening page for comparison, not as candidates for MIND Core without a GPU.
- **UTMOS** ranks Piper highest in both languages and Kitten nano's voice 1 lowest. The choice is by ear.

### Preliminary recommendation (the maintainer chooses by ear)

| Language | `compact` | `quality` |
|---|---|---|
| Russian | Piper `irina` (63 MB), or Vosk TTS 0.7 voice 3 (247 MB, Apache-2.0 throughout) | Vosk TTS 0.9, voice 3 or 2 (937 MB) |
| English | Kitten TTS nano (24 MB) | Kokoro-82M, `am_michael` or `af_heart` (354 MB; an int8 export of v1.1 takes 168 MB with its voices, not measured yet) |

### The maintainer's choice (2026-10-08)

The maintainer listened to the voices and chose eight: a female and a male voice for each language and variant. Pitch tells them apart: the female voices sit at 171–232 Hz, the male ones at 103–147 Hz.

| | Russian `compact` | Russian `quality` | English `compact` | English `quality` |
|---|---|---|---|---|
| female | Vosk TTS 0.7, speaker 0 | ESpeech TTS-1 RL-V2 in the voice of FLEURS sentence 1825 | Piper `lessac` | Kokoro-82M `bf_emma` |
| male | Vosk TTS 0.7, speaker 3 | Vosk TTS 0.9, speaker 4 | Piper `ryan` | Piper `ryan` |

All eight are in `models/manifest.toml`, and every file was fetched and matched its SHA-256 on this host. Together they take 5.3 GB.

- **ESpeech** needs three models more: the Vocos vocoder, RUAccent's stress models and the reference recording.
  - Its reference is a FLEURS recording (CC BY 4.0), and the attribution travels with it.
  - The speaker agreed to FLEURS, not to being a system's voice. A recording made with its speaker's consent can take its place without other changes.
  - On a CPU ESpeech takes 26 s per second of speech, so the system can use it only for phrases made ahead of time, unless it has a GPU.
- **Terms that limit use:**
  - Piper `ryan` was trained on RyanSpeech (CC BY-NC-SA 4.0), Piper `lessac` on the Blizzard 2013 Lessac data (a research licence).
  - The maintainer allowed any free licence (issues-human, section 6), and each model's terms are in the manifest.
- **What the cache leaves out:** espeak-ng (GPL-3.0), which the Piper and Kokoro runtimes use for phonemes. MIND Core will make its own phonemes (step 3).

## Plan

1. **The choice** (done, above). The maintainer listens to the voices on a listening page made by `scripts/voice_tts/page.py` (four sentences per voice, with the measurements) and picks a `compact` and a `quality` voice per language.
2. **The cache** (done). The chosen models are in `models/manifest.toml` with their licences, terms and voices, so `scripts/models.py` fetches them and puts them on the model disk (251).
3. **Phonemes.**
   - Piper and Kokoro take espeak-ng phonemes. espeak-ng is GPL-3.0 and cannot be built into MIND Core.
   - So the `phonetics` crate (our own letter-to-sound rules and stress dictionary) learns to give the IPA symbols these models expect. It is checked against espeak-ng on the host only.
   - Vosk TTS has its own Russian dictionary and rules, and ESpeech reads letters with stress marks from RUAccent.
4. **The engine** in `mind::tts`, `no_std` with `alloc`, for the chosen families:
   - VITS (Piper, Vosk TTS 0.7): a text encoder, a duration predictor, a flow and a HiFi-GAN decoder.
   - Vosk TTS 0.9: a duration predictor, a diffusion transformer over several steps, and a BERT for prosody. It is the largest to port and the slowest per sentence.
   - StyleTTS 2 (Kokoro): a text encoder, a style vector per voice, a duration and prosody predictor, and an iSTFTNet decoder.
   - F5-TTS (ESpeech): a diffusion transformer over 32 steps, the Vocos vocoder and RUAccent's stress models. It runs only ahead of time on a CPU, so it is the last to port.
   - Host test: our audio equals onnxruntime's within a set tolerance on the 24 sentences.
5. **The service.** `tts` speaks with the chosen voice from `models:` and keeps the formant synthesizer as the fallback without a model disk. The model is one read-only memory object (150), and the service holds no other authority (MC-8.1, MC-11.11).
6. **QEMU suite:** the system phrases spoken by the neural voice are recognized by the host recognizer as in this comparison.

Tasks are numbered `252-APP-MMMM` from the tools track's next counter.

## Acceptance criteria

- The maintainer has chosen the voices by ear, and each is in `models/manifest.toml` with its licence and terms; `scripts/models.py fetch --role tts` fetches them (done 2026-10-08).
- MIND Core speaks the system's phrases with the chosen voice on x86 and aarch64 (QEMU), from the model disk of 251.
- Its speech of the 24 sentences is within 0.5 CER points of the published model, measured as above.
- Nothing uses the network. The synthesizer holds the text, the model and its audio line, and no other authority (MC-8.1, MC-11.11).

## Related

[250](250-voice-dictation.md), [251](251-model-cache-and-model-disk.md), [docs/voice](../docs/voice/README.md), [017](../issues-done/017-tts-on-audio-gateway.done), [scripts/voice_tts](../scripts/voice_tts/README.md).
