# MIND CORE — voice: speaking, listening, understanding

**Version:** 0.1 (2026-10-04) · **Status:** plan · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) Art. 8 (cognitive plane), 11.5, 11.11 (untrusted input), MC-3.7, MC-3.11 · **Roadmap:** [track G](../../ROADMAP.md) (input methods, UI), D (network for V4) · [Russian](README_RU.md)

The system is meant to talk with people: to speak, to listen and to act on what it heard. This plan says what exists, what is missing, how the pieces fit the Constitution, and which work belongs to the **tools track** (this branch, issues 077–099) and which to the **kernel track** (issues 150–199; the network track has 100–149).

## 1. What exists

| Piece | State | Where |
|---|---|---|
| Speech synthesis | Done: formant synthesizer, Russian (letter-to-sound rules, a stress dictionary of ~13 700 forms, «ё» restored) and English (~18 700 CMUdict words, spelling rules), prosody for `.` and `?`; ~68 % of Russian and ~53 % of English words recognized by an offline Vosk model on the host | `tts/`, `mind::tts::say`, `say`, [017](../../issues-done/017-tts-on-audio-gateway.done) |
| Audio output | Done: AC97, DMA ring of 32 × 4 KiB at 48 kHz stereo, interrupts over IPC | `audio_gw`, `idl/audio.wit` |
| Microphone | Done: AC97 PCM-in ring, `record-start/read/stop`, overflow count; `listen` records, shows the level and plays back | `audio_gw`, `listen/`, [022](../../issues-done/022-audio-tools-say-listen.done) |
| Speech recognition | **None.** Vosk runs only in the host test harness to check the synthesizer | `tests/qemu_smoke.py --asr-model` |
| Speech detection, push-to-talk, wake word | **None** | — |
| Understanding (commands, intents, dialogue) | **None** | — |
| Voice as an input method | **None**; track G names "input methods" without issues | ROADMAP |

## 2. Principles

1. **The voice path proposes, the user's agent decides (Art. 8.1–8.2, MC-3.7).** Recognition and understanding are the cognitive plane. A recognizer turns sound into text, an interpreter turns text into a typed *intent*; neither holds the authority to act. Intents go to the shell — the user's agent that already decides what programs get (MC-3.11) — and the shell applies the same rules as to typed commands.
2. **Microphone input is untrusted input (Art. 11.5, 11.11).** The recognizer gets the audio client and nothing else: no files beyond its model, no spawn, no lifecycle, no network. Its output is data with provenance (`heard by voice, confidence 0.83`), never a command by itself.
3. **Dangerous actions are confirmed.** Stopping a service, deleting, formatting, rebooting: the shell asks back ("Удалить три файла?") and waits for "да" or a key. Confirmation by voice uses a closed yes/no grammar.
4. **Bounded resources (MC-5).** Fixed-size buffers, a bounded model, a bounded utterance (≤ 8 s), recognition in a budgeted task; the system stays responsive while recognizing.
5. **Offline first.** Commands work without a network. Larger models and remote services (V3–V4) are additions, not a dependency of basic voice control.
6. **Russian and English**, like the synthesizer.

## 3. Architecture

```text
microphone ─► audio_gw ─► front end ─► recognizer ─► interpreter ─► shell ─► action
 (AC97)       (48 kHz)    16 kHz mono   grammar/      text → intent   policy,     │
                          VAD, frames   model         (typed)         confirm     ▼
                                                                         ◄── tts reply
```

- **Front end** (`mind::voice`, library): downmix and resample 48 kHz stereo → 16 kHz mono, pre-emphasis, 25 ms frames every 10 ms, energy/zero-crossing speech detection with hangover, utterance endpointing; a source trait for the microphone or a WAV file (tests, offline use).
- **Recognizer** (`mind::voice` + model file): log-mel/MFCC features; a small acoustic model (int8 weights, SSE2) giving phoneme posteriors; decoding constrained by a **grammar** of phrases whose pronunciations come from the synthesizer's own letter-to-sound rules and lexicons (one phoneme set for speaking and listening). Confidence from the best/second-best path ratio; below a threshold the result is "not understood".
- **Interpreter:** grammar phrases carry slots ("открой файл {name}", "покажи {tool}", "который час"); a match yields an intent `{verb, object, arguments, confidence, utterance}`.
- **`voice` program:** the shell starts it as a console program in a launch session and lends it the audio client, the `tts` client and an endpoint back to the shell (the ping/pong slot). It listens while the push-to-talk key is held (or after the key toggles listening), sends intents to the shell, and speaks replies.
- **Shell:** receives intents on that endpoint as if typed, runs them through the same command table and policy, asks for confirmation where needed, and answers through `tts` ("Запускаю файловый менеджер").

## 4. Stages

| Stage | Result | Track | Issues |
|---|---|---|---|
| **V0. Hearing** | 16 kHz mono stream, speech detection and endpointing, WAV source, `listen --vad` | tools | [077](../../issues/077-voice-audio-front-end.md) |
| **V1. Commands** | Offline recognizer of a fixed grammar (Russian, English) from a model file; `hear` prints what it recognized; tested by synthesizer loopback | tools | [078](../../issues/078-voice-command-recognizer.md) |
| **V2. Voice control** | `voice` + shell integration: intents, confirmations, spoken replies, push-to-talk in the shell, a command set covering the tools | tools | [079](../../issues/079-voice-control-in-the-shell.md) |
| **V2+. Push-to-talk anywhere** | A key that reaches the voice program whatever has the focus | kernel | [154](../../issues/154-push-to-talk-routing.md) |
| **V3. Dictation** | Large-vocabulary recognition (text into `edit`, search in `fm`) from a 40–80 MB model | kernel first, then tools | [150](../../issues/150-user-memory-beyond-the-arena.md), [153](../../issues/153-xsave-avx-state.md); a tools issue when they are done |
| **V4. Understanding and dialogue** | Free speech → intent through a language model: remote first (through the network track's policy broker and TLS service), local later | network, tools | 101–103 (network track); a tools issue then |

### V0 — hearing (tools)

- `mind::voice::Source`: `Microphone` (audio client) and `Wav` (a 16-bit PCM file, for tests and offline use); `Stream` resamples 48 kHz stereo to 16 kHz mono (polyphase low-pass, no aliasing above 8 kHz).
- Speech detection: frame energy against an adaptive noise floor and zero crossings, 200 ms hangover, utterance bounds 0.3–8 s; reports start/end and level.
- `listen --vad` shows the detected utterances; `listen --wav file` uses a file.
- Tests: `tests/voice_host.rs` (resampler response, detection on synthesized speech with noise and silence); QEMU: a WAV on the boot disk through `listen --vad --wav`.

### V1 — commands (tools)

- Features: 40 log-mel bands, 25 ms / 10 ms, mean normalization; MFCC for the model.
- Model: a small network (two convolutional layers and a few dense layers, ~0.5–3 MB int8) giving per-frame posteriors over the synthesizer's phoneme set plus silence; trained offline on the host (script in `scripts/`, data recorded or synthesized; the training corpus and its licences are listed in `THIRD_PARTY.md`), weights in a versioned file `voice/model.bin` with a header (phoneme set, feature parameters, checksum).
- Grammar: phrases in a text file (`voice/commands.txt`), compiled into a phoneme graph with the synthesizer's letter-to-sound rules (shared code moved from `tts/` into a module both use); Viterbi decoding over the graph; garbage/filler model for out-of-grammar speech.
- `hear [--wav file]` prints the phrase, its slots and confidence.
- Tests: host loopback — the synthesizer speaks every grammar phrase in both voices and pitches, mixed with noise; at least 90 % recognized, out-of-grammar phrases rejected; QEMU: `hear --wav` on a synthesized WAV.

### V2 — voice control (tools)

- `voice` (console program): push-to-talk in the shell (hold F12, or press it once to listen for one utterance), recognition, intent to the shell over the lent endpoint, the reply through `tts`.
- Shell: an intent queue next to the keyboard; intents become command lines (`run fm`, `top`, `date`, `stop`…), shown in the shell as `voice: открой файлы` before they run; confirmation (voice yes/no or Enter/Esc) for stop, kill, delete, format, reboot; "отмена"/"стоп" cancels.
- The command set covers the tools: open/close tools, show processes/memory/load/log, what time/date, read a file aloud, say what is on the screen line.
- `audio_gw`: one capture owner at a time (`record-start` fails with `busy` while another client records), so `listen` and `voice` do not mix streams (`audio.wit` 1.1).
- Tests: QEMU with a WAV source standing in for the microphone: "открой файлы" starts `fm`; "останови службу rtc" asks for confirmation and stops only after "да"; nonsense is rejected and spoken as "не понял".

## 5. Who does what

### Tools track (this branch, 077–099)

| № | Task | Blocked by |
|---|---|---|
| [077](../../issues/077-voice-audio-front-end.md) | V0: audio front end — 16 kHz mono, speech detection, WAV source, `listen --vad` | — |
| [078](../../issues/078-voice-command-recognizer.md) | V1: offline command recognizer — features, model file, grammar from the synthesizer's phonemes, `hear` | 077 |
| [079](../../issues/079-voice-control-in-the-shell.md) | V2: `voice` program, intents in the shell, confirmations, spoken replies, capture ownership in `audio_gw` | 078 |
| later | V3 dictation, V4 understanding through a language model | 150, 153; 101–103 |

### Kernel track (150–199)

| № | Task | Needed by |
|---|---|---|
| [150](../../issues/150-user-memory-beyond-the-arena.md) | User memory beyond the 64 MiB kernel arena: task heaps and memory objects from free RAM; large read-only memory objects for models, shared between tasks | V3 (models of 40–80 MB), larger tools |
| [153](../../issues/153-xsave-avx-state.md) | XSAVE: AVX/AVX2 register state per task | V3 (inference speed), V1 optional |
| [154](../../issues/154-push-to-talk-routing.md) | Push-to-talk routing: one configured key delivered to a registered listener regardless of the focus, like the attention key | V2+ |

Network track (100–149, in progress elsewhere): network stack, policy broker, TLS — needed by V4.

V0–V2 need **no kernel change**: the voice program is an ordinary application that the shell starts and lends capabilities to.

## 6. Testing without a microphone

- **Synthesizer loopback:** our own `tts` produces the test speech, in both languages and at several pitches and rates, mixed with recorded or generated noise at set signal-to-noise ratios. The recognizer must recognize grammar phrases and reject others.
- **WAV source:** every voice program accepts `--wav file`; QEMU tests put the WAV on the boot disk. (QEMU has no file-backed microphone; the AC97 capture path keeps its own `listen` test.)
- **Host tests** run the front end, features, model and decoder on the host like the other `*_host.rs` tests.
- **Real microphone:** manual check on hardware or QEMU with a host audio backend; results recorded in the issue.

## 7. Limits and risks

- Memory: a program heap is at most 16 MiB in 32 blocks, an image at most 4 MiB, and user memory comes from the 64 MiB kernel arena — enough for V0–V2 (model ≤ 3 MB, loaded from a file), not for V3 (150).
- Compute: SSE2 only (FXSAVE context); int8 inference of a small model for an 8 s utterance takes well under a second on one core; larger models need AVX (153).
- Accuracy: a synthesized training and test voice overestimates accuracy on real voices; real recordings must be added before V2 is claimed done for users.
- Privacy: audio never leaves the machine in V0–V3; V4 sends text (not audio) only through the policy broker with the user's consent.

## 8. Decisions to take

1. Push-to-talk key in the shell: F12 (proposed), or another.
2. Training data for V1: synthesized only at first (reproducible, licence-clean), real recordings later (which corpus, licence).
3. V4: which remote model service, if any; what may be sent.
