# MIND CORE — voice: speaking, listening, understanding

**Version:** 0.4 (2026-10-04) · **Status:** V0–V2 done, V3–V4 planned · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) Art. 8 (cognitive plane), 11.5, 11.11 (untrusted input), MC-3.7, MC-3.11 · **Roadmap:** [track G](../../ROADMAP.md) (input methods, UI), D (network for V4) · [Russian](README_RU.md)

The system is meant to talk with people: to speak, to listen and to act on what it heard. This plan says what exists, what is missing, how the pieces fit the Constitution, and which work belongs to the **tools track** (`APP` in [TRACKS.md](../../TRACKS.md): issues 077–099, then u001–u017, and since 2026-10-06 main tasks 250–299 and tasks `NNN-APP-MMMM`) and which to the **kernel track** (issues 150–199; the network track has 100–149).

## 1. What exists

| Piece | State | Where |
|---|---|---|
| Speech synthesis | Done: formant synthesizer, Russian (letter-to-sound rules, a stress dictionary of ~13 700 forms, «ё» restored) and English (~18 700 CMUdict words, spelling rules), prosody for `.` and `?`; ~68 % of Russian and ~53 % of English words recognized by an offline Vosk model on the host | `tts/`, `mind::tts::say`, `say`, [017](../../issues-done/017-tts-on-audio-gateway.done) |
| Audio output | Done: AC97, DMA ring of 32 × 4 KiB at 48 kHz stereo, interrupts over IPC | `audio_gw`, `idl/audio.wit` |
| Microphone | Done: AC97 PCM-in ring, `record-start/read/stop`, overflow count; `listen` records, shows the level and plays back | `audio_gw`, `listen/`, [022](../../issues-done/022-audio-tools-say-listen.done) |
| Speech recognition (V1) | Done: offline recognizer of a command grammar in Russian and English — log-mel features, an int8 network giving phone scores, Viterbi decoding of the grammar against a free phone loop; `hear` prints what it recognized. Trained on our synthesizer's speech only. (Vosk still checks the synthesizer in the host harness.) | `libmind/src/voice`, `hear/`, `voice/`, [078](../../issues-done/078-voice-command-recognizer.done) |
| Hearing (V0) | Done: `mind::voice` — microphone or WAV file as a 16 kHz mono stream, speech detection and endpointing; `listen --vad`, `listen --wav` | `libmind/src/voice`, `listen/`, [077](../../issues-done/077-voice-audio-front-end.done) |
| Voice control (V2) | Done: the shell starts `voice`, push-to-talk (F12 or `voice listen`) while the shell has the focus; the shell runs tools, tells the time, date and free memory, reads a file aloud, stops or restarts a service after a spoken or typed yes, and answers through `tts` | `hear/src/bin/voice.rs`, `shell/src/voicectl.rs`, `idl/voice.wit`, [079](../../issues-done/079-voice-control-in-the-shell.done) |
| Push-to-talk over any program, wake word | Push-to-talk over any program: done ([154](../../issues-done/154-push-to-talk-routing.done)); wake word: none | — |
| Understanding of free speech | **None** (V4); commands and yes/no only | — |
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
- **`voice` program:** the shell starts it as a console program in a launch session and lends it a client of its own endpoint (`idl/voice.wit`) in the ping/pong slot; with the audio, `tts` and read-only file clients every program gets, that is all it holds. It listens for one utterance when the shell says so, recognizes it, reports it, and speaks the shell's reply.
- **Shell:** answers voice's report with the next order, decides what a phrase does with the same programs and lifecycle requests as typed commands, asks for confirmation where needed, and answers through `voice` ("Запускаю файловый менеджер").

## 4. Stages

| Stage | Result | Track | Issues |
|---|---|---|---|
| **V0. Hearing** (done) | 16 kHz mono stream, speech detection and endpointing, WAV source, `listen --vad` | tools | [077](../../issues-done/077-voice-audio-front-end.done) |
| **V1. Commands** (done) | Offline recognizer of a fixed grammar (Russian, English) from a model file; `hear` prints what it recognized; tested by synthesizer loopback | tools | [078](../../issues-done/078-voice-command-recognizer.done) |
| **V2. Voice control** (done) | `voice` + shell integration: intents, confirmations, spoken replies, push-to-talk in the shell, a command set covering the tools | tools | [079](../../issues-done/079-voice-control-in-the-shell.done) |
| **V2+. Push-to-talk anywhere** | A key that reaches the voice program whatever has the focus | kernel | [154](../../issues-done/154-push-to-talk-routing.done) (done) |
| **V3. Dictation** | Large-vocabulary recognition (text into `edit`, search in `fm`) from a 40–80 MB model | kernel first, then tools | [150](../../issues-done/150-user-memory-beyond-the-arena.done), [153](../../issues-done/153-xsave-avx-state.done); a tools issue when they are done |
| **V4. Understanding and dialogue** | Free speech → intent through a language model: remote first (through the network track's policy broker and TLS service), local later | network, tools | 101–103 (network track); a tools issue then |

### V0 — hearing (tools, done)

`mind::voice` (libmind, feature `alloc`; the device-free part in `libmind/src/voice/front.rs` is shared with the host tests):

- `Source` — interleaved 16-bit samples with a rate and a channel count: `Microphone` (the audio client, 48 kHz stereo, counts capture overflows, stops capture when dropped) and `Wav` (16-bit PCM or `WAVE_FORMAT_EXTENSIBLE` PCM, 1–8 channels, 4–192 kHz; `Wav::open` reads up to 8 MiB through the file client).
- `Stream` — any source as 16 kHz mono: channels averaged, the rate changed by L/M with a polyphase windowed-sinc low-pass (Kaiser, cut at 7.5 kHz, about 70 dB stopband, integer Q16 filtering, designed once per stream); 16 kHz passes through.
- `Detector` — 25 ms frames every 10 ms after a DC blocker; a frame is voiced 9 dB above an adaptive noise floor, or 4 dB above it with clearly more zero crossings than the background (fricatives); nothing under -50 dBFS counts. An utterance starts after 3 voiced frames and ends after 200 ms below the threshold; its bounds widen over adjacent frames 3 dB above the floor (a breathy «х», a fading vowel; at most 250 ms); 0.2–8 s (a «да» is a quarter of a second), longer ones are cut. `Utterance { start_ms, samples, level }` holds the 16 kHz samples and the level in dBFS.
- `listen --vad [seconds]` (1–60, default 10) prints each utterance of the microphone (`SPEECH AT 1230 MS, 850 MS, LEVEL -18 DBFS`); `listen --vad --wav FILE` does the same for a file; `listen --wav FILE` converts a file to 16 kHz mono, measures it and plays it back.
- Tests: `tests/voice_host.rs` — the resampler (1 kHz within 0.2 dB from 8, 22.05, 32, 44.1 and 48 kHz; 10 kHz below -40 dB; no 8 kHz upsampling image), WAV parsing, detection on four synthesized phrases (Russian and English) in silence and in white noise at 20 dB SNR (each phrase once, bounds within 50 ms), nothing in silence, noise or a click, 10 s of voice cut into 8 s + 2 s; QEMU `listen` suite — a 48 kHz stereo WAV of three phrases from the host build of `tts` on the boot disk, `listen --vad --wav` finds each where it begins, and `listen --vad` finds nothing in the microphone's silence.

### V1 — commands (tools, done)

- **Phonetics** (`phonetics/`): the synthesizer's letter-to-sound rules, stress dictionary and English lexicon moved out of `tts/` into a crate both `tts` and the recognizer use; the phone alphabet is `phonetics::phonemes::PHONES` (37 phonemes, soft and hard consonants alike) plus silence.
- **Features** (`mind::voice::features`): 40 log-mel bands (64 Hz–8 kHz) of 25 ms frames every 10 ms after pre-emphasis and a Hamming window, a 512-point FFT, all in integers (tenths of a dB), per-utterance mean normalization. The host trainer runs the same code.
- **Model** (`mind::voice::model`, `voice/model.bin`, 260 KB): a time convolution over 11 frames and three dense layers (440 → 256 → 256 → 256 → 38) with int8 weights and integer requantization; per frame it scores the 38 classes, relative to the best. The file has a header (version, bands, context, input scale, thresholds, class names) and a checksum.
- **Training** (`scripts/voice_train.rs`, a host Rust program): speech from our synthesizer — every grammar phrase in six voices and 4000 strings of random Russian and English dictionary words, pitch 82–168 Hz, rate 75–135 %, in silence or white noise (8–35 dB SNR) — with the phone of every sample known from the synthesizer (`synth::speak_labeled`); f32 training (Adam), int8 quantization, then the decoder's thresholds calibrated on separate speech (other voices, phrases outside the grammar). About 7 minutes on 4 cores; seeded, so the same sources give the same model.
- **Grammar** (`voice/commands.txt`, `mind::voice::grammar`): `intent: phrase | phrase` with `{slot}` words (`slot tool: файлы = fm, editor = edit, …`); every filled-in phrase becomes a chain of phone states (two frames at least, optional silence between words); Viterbi decoding against every chain. The free phone loop (any phone sequence) is the filler: its score is 0 by construction, so a phrase's mean deficit per frame measures how well it fits. A phrase is accepted when the deficit is under the model's threshold (and ahead of the best phrase meaning something else).
- **`hear [seconds]`**, **`hear --wav FILE`** (console program; it has the audio and read-only file clients only): `HEARD "открой файлы" INTENT=open TOOL=fm CONFIDENCE=0.95`, `NOT UNDERSTOOD (CLOSEST "...", CONFIDENCE=0.00)` or `NOTHING HEARD`.
- **Tests:** `tests/voice_host.rs` — all 147 phrases at three pitches and two rates in 20 dB noise: 882 of 882 recognized; 50 phrases outside the grammar (some close to commands): none accepted; 8 s of speech recognized in about 70 ms on one host core. QEMU `listen` suite: `hear --wav` on a WAV made by the host build of `tts` recognizes two commands and refuses a third phrase.
- **Limits:** trained and tested on one synthetic voice: it says nothing yet about real voices, accents or room noise. Real recordings (and their licences) are the next step before V2 is offered to users.

### V2 — voice control (tools, done)

- **`voice on [--wav FILE] [SECONDS]`** in the shell starts `voice` (a console program in the background) and lends it a client of the shell's endpoint (`idl/voice.wit`; init gives the shell a quota of one endpoint for it). `voice` drops the clock and loader clients at start and keeps the line to the shell, the audio, `tts` and read-only file clients (Art. 11.11); `caps` shows it. `voice off` ends it, `voice` tells whether it runs.
- **The line** (`voice.wit` `next`): `voice` reports what it heard — nothing, a refused utterance with its closest phrase, or a phrase with its intent, slots and confidence — and asks for the next order: speak a text, then wait, listen, listen for yes or no only, or quit. The shell defers the call (`Call::defer`) until push-to-talk, so `voice` never listens on its own. After speaking, `voice` waits for the speech to end, so it does not hear itself.
- **Push-to-talk:** F12 (or `voice listen`) whatever program has the focus: `voice` listens for one utterance — up to SECONDS (default 6) from the microphone, or the next utterance of the WAV file standing in for it. Esc cancels. While voice control is on, the shell takes F12 out of the focused program's input (`INPUT_LISTEN`, [154](../../issues-done/154-push-to-talk-routing.done)); Esc cancels only while the shell has the focus.
- **The shell decides** (`shell/src/voicectl.rs`), answering in the language of the phrase: open a tool (`fm`, `edit`, `top`, `memmap`, `load`, `hw`, `dmesg`, `svc` — started like a typed command, in the foreground), close the last one it opened, the time and date in words (the synthesizer would read digits one by one), free memory, read a file aloud (`docs/notes.txt`), stop or restart a service (`rtc`, `netstack`, `sysmon`), reboot, help, repeat. Each phrase shows in the shell as `VOICE: "который час" (973) -> Сейчас восемнадцать часов четыре минуты`. Stopping or restarting a service and rebooting are asked back ("Остановить службу rtc?"); `voice` then listens with a closed grammar of yes, no and cancel, and only yes goes ahead; Enter answers yes and Esc no from the keyboard. A refused phrase is answered "Не понял" and runs nothing.
- **Recognition for short answers:** the detector keeps utterances from 0.2 s (a «да» is a quarter of a second), and an utterance goes to the recognizer with up to 200 ms of the stream before and after it, as the model was trained (`hear::CONTEXT_MS`) — the detector's tight bounds had cost «нет» its final «т».
- **`audio_gw`** (`audio.wit` 1.1): one capture owner at a time; `record-start`, `record-read` and `record-stop` answer `busy` to others while the owner lives, so `listen`, `hear` and `voice` never mix streams (`hear`: `THE MICROPHONE IS BUSY (ANOTHER PROGRAM RECORDS)`).
- **Tests:** `tests/voice_host.rs` — the extended grammar (181 phrases): 1086 of 1086 recognized at three pitches and two rates in 20 dB noise, none of the 50 outside phrases accepted; the closed yes/no grammar takes «да», «нет», «конечно», "yes", "no", «отмена» and refuses commands; «да» and «нет» are found as utterances in silence and noise. QEMU `listen` suite — `hear` cannot record while `listen` does; `voice on --wav` with a WAV of eight utterances from the host build of `tts`: «открой файлы» starts `fm`, «который час» is answered with the time, «останови службу rtc» asks back and «нет» cancels, asked again «да» stops `rtc`, «прочитай заметки» speaks the first line of `docs/notes.txt`, «сегодня хорошая погода» is answered «Не понял» and runs nothing; `voice` holds endpoints in slots 1, 3, 4 and 6 only.
- **Limits:** one synthetic voice in training (see V1); the command set is fixed in `voice/commands.txt` and the shell's tables (tools, services, files to read).

## 5. Who does what

### Tools track (this branch, 077–099)

| № | Task | Blocked by |
|---|---|---|
| [077](../../issues-done/077-voice-audio-front-end.done) | V0: audio front end — 16 kHz mono, speech detection, WAV source, `listen --vad` — **done** | — |
| [078](../../issues-done/078-voice-command-recognizer.done) | V1: offline command recognizer — features, model file, grammar from the synthesizer's phonemes, `hear` — **done** | — |
| [079](../../issues-done/079-voice-control-in-the-shell.done) | V2: `voice` program, intents in the shell, confirmations, spoken replies, capture ownership in `audio_gw` — **done** | — |
| later | V3 dictation, V4 understanding through a language model | 150, 153; 101–103 |

### Kernel track (150–199)

| № | Task | Needed by |
|---|---|---|
| [150](../../issues-done/150-user-memory-beyond-the-arena.done) | User memory beyond the 64 MiB kernel arena: task heaps and memory objects from free RAM; large read-only memory objects for models, shared between tasks (done) | V3 (models of 40–80 MB), larger tools |
| [153](../../issues-done/153-xsave-avx-state.done) | XSAVE: AVX/AVX2 register state per task (done) | V3 (inference speed), V1 optional |
| [154](../../issues-done/154-push-to-talk-routing.done) | Push-to-talk routing: keys delivered to a registered listener regardless of the focus, like the attention key (done) | V2+ |

Network track (100–149, in progress elsewhere): network stack, policy broker, TLS — needed by V4.

V0–V2 needed **no kernel change**: the voice program is an ordinary application that the shell starts and lends capabilities to (init gives the shell a quota of one endpoint for it).

## 6. Testing without a microphone

- **Synthesizer loopback:** our own `tts` produces the test speech, in both languages and at several pitches and rates, mixed with recorded or generated noise at set signal-to-noise ratios. The recognizer must recognize grammar phrases and reject others.
- **WAV source:** every voice program accepts `--wav file`; QEMU tests put the WAV on the boot disk. (QEMU has no file-backed microphone; the AC97 capture path keeps its own `listen` test.)
- **Host tests** run the front end, features, model and decoder on the host like the other `*_host.rs` tests.
- **Real microphone:** manual check on hardware or QEMU with a host audio backend; results recorded in the issue.

## 7. Limits and risks

- Memory: an image is at most 4 MiB; a program heap is 16 MiB by default and more when the program asks for it (`mind::request!(…, memory: MiB)`), from the free RAM ([150](../../issues-done/150-user-memory-beyond-the-arena.done)). A 40–80 MB model fits, loaded once as a sealed read-only object that several tasks map.
- Compute: the kernel saves AVX state per task on CPUs that have it (XSAVE, [153](../../issues-done/153-xsave-avx-state.done)); a program may use AVX/AVX2 after checking CPUID and XCR0 (`cpus` shows `FPU=XSAVE+AVX`), in code built with those target features. int8 inference of a small model for an 8 s utterance takes well under a second on one core with SSE2.
- Accuracy: a synthesized training and test voice overestimates accuracy on real voices; real recordings must be added before V2 is claimed done for users.
- Privacy: audio never leaves the machine in V0–V3; V4 sends text (not audio) only through the policy broker with the user's consent.

## 8. Decisions to take

1. Push-to-talk key in the shell: F12 (taken in V2).
2. Training data for V1: synthesized only at first (reproducible, licence-clean), real recordings later (which corpus, licence).
3. V4: which remote model service, if any; what may be sent.
