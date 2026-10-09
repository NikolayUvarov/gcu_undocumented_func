# 250 — Voice V3: dictation with a Zipformer model, Russian first

**Type:** main task, tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — ([150](../issues-done/150-user-memory-beyond-the-arena.done) and [153](../issues-done/153-xsave-avx-state.done) are done) · **Roadmap:** track G, voice V3 ([plan](../docs/voice/README.md)) · **Constitution:** MC-8.1, MC-11.5, MC-11.11, MC-12.1, MC-12.2

## Problem

V2 recognizes a closed grammar of commands. Dictation needs a large vocabulary: text into `edit`, a search in `fm`. The plan asks for a model of 40–80 MB that works offline, for Russian and English. Its kernel prerequisites are done: models beyond the 64 MiB arena (150) and AVX state (153).

## Model choice (measured 2026-10-08)

### Method

- **Test sets:** FLEURS (Google, CC BY 4.0): read Wikipedia sentences at 16 kHz.
  - 272 Russian sentences (51 min) and 281 English (45 min).
  - One recording per sentence.
  - No reference contains digits, so «5» against «пять» is never an error.
- **Text:** every hypothesis and reference is normalized the same way: lower case, ё → е, no punctuation.
- **Runtimes:**
  - each model through its published ONNX export in sherpa-onnx 1.13.8 (onnxruntime);
  - the Kaldi models through vosk 0.3.45.
- **Speed:** one thread; timed again with each model alone on the first 40 sentences of the set. The host is a Xeon at 2.1 GHz.
- **Reproducing it:** the scripts and the raw numbers are in [scripts/voice_v3](../scripts/voice_v3/README.md).
- **What the numbers mean:** how the published models do on these sentences on this host. They are not evidence about MIND Core. Our implementation, its int8 weights and a live microphone need their own measurements (MC-12.1, MC-12.2).

WER and CER are word and character error rates. "Time" is decoding time per second of speech: 0.026 means 26 ms.

### Russian

| Model | Kind | Parameters | WER % | CER % | Time | Weights licence |
|---|---|---|---|---|---|---|
| **alphacep `vosk-model-ru` 0.54** | Zipformer2 transducer, whole utterance | 65.0 M (int8 72.5 MB) | **5.20** | 1.30 | 0.026 | Apache-2.0 |
| NVIDIA Parakeet TDT 0.6B v3 (reference) | FastConformer transducer, 25 languages | 627 M (int8 670 MB) | 7.08 | 1.85 | 0.166 | CC BY 4.0 |
| alphacep `vosk-model-streaming-ru` 0.54 | Zipformer2 transducer, streaming | 65.6 M | 9.53 (fp32) | 2.25 | 0.106 (fp32) | Apache-2.0 |
| T-Bank T-one | Conformer CTC over 34 letters, streaming, 8 kHz input | 71.7 M | 9.91 | 2.16 | 0.172 | Apache-2.0 |
| NVIDIA multilingual FastConformer CTC (reference) | 10 languages | 110 M | 10.05 | 2.26 | ≈0.043 | NGC terms of use |
| alphacep `vosk-model-small-streaming-ru` 0.54 | Zipformer2 transducer, streaming | 23.2 M (int8 27.8 MB) | 10.60 | 2.65 | 0.039 | Apache-2.0 |
| `vosk-model-small-ru-0.22` | Kaldi TDNN and WFST decoder | 45 MB archive | 17.61 | 4.42 | 0.122 | Apache-2.0 |
| OpenAI Whisper base | encoder-decoder, 99 languages | 72.6 M | 20.48 (fp32) | 5.16 | 0.326 | MIT |
| OpenAI Whisper tiny | encoder-decoder, 99 languages | 37.8 M | 36.30 (fp32) | 9.69 | 0.172 | MIT |

### English

| Model | Kind | Parameters | WER % | CER % | Time | Weights licence |
|---|---|---|---|---|---|---|
| NVIDIA Parakeet TDT 0.6B v3 (reference) | FastConformer transducer, 25 languages | 627 M | 8.04 | 3.79 | 0.171 | CC BY 4.0 |
| NVIDIA Parakeet TDT-CTC 110M (CTC head) | FastConformer CTC | 109.3 M | 8.92 (fp32) | 3.66 | 0.040 | CC BY 4.0 |
| NVIDIA multilingual FastConformer CTC (reference) | 10 languages | 110 M | 10.19 | 3.93 | ≈0.043 | NGC terms of use |
| OpenAI Whisper base | encoder-decoder | 72.6 M | 10.29 (fp32) | 4.83 | 0.244 | MIT |
| Kroko EN community | Zipformer2 transducer, streaming | 65.7 M | 11.08 | 6.32 | 0.046 | "CC-BY-SA" without a version; the LICENSE file is empty |
| NVIDIA Conformer CTC medium | Conformer CTC | 30.8 M | 11.15 (fp32) | 4.56 | ≈0.034 | NGC terms of use |
| **k2-fsa Zipformer GigaSpeech 2023-12-12** | Zipformer2 transducer, whole utterance | 65.0 M | **11.28** (fp32), 11.98 (int8) | 6.43 | 0.026 (int8) | Apache-2.0; trained on GigaSpeech (see below) |
| Useful Sensors Moonshine base | encoder-decoder | 61 M | 13.05 (int8) | 8.11 | 0.059 | MIT |
| Useful Sensors Moonshine tiny | encoder-decoder | 27 M | 13.41 (int8) | 6.99 | 0.030 | MIT |
| OpenAI Whisper tiny | encoder-decoder | 37.8 M | 13.68 (fp32) | 6.14 | 0.131 | MIT |
| NVIDIA Conformer CTC small | Conformer CTC | 13.2 M | 14.46 (fp32) | 6.14 | ≈0.019 | CC BY 4.0 |
| `vosk-model-small-en-us-0.15` | Kaldi TDNN and WFST decoder | 40 MB archive | 22.05 | 11.30 | 0.179 | Apache-2.0 |
| k2-fsa Zipformer Libriheavy small | Zipformer2 transducer, audiobooks only | 65 M | 36.64 | 20.68 | ≈0.027 | Apache-2.0 |
| k2-fsa streaming Zipformer 20M | Zipformer2 transducer, LibriSpeech only | 23 M | 71.01 | 64.98 | ≈0.042 | Apache-2.0 |

≈: timed during the accuracy runs, three at a time, not alone.

### Findings

- **Russian.** alphacep's Vosk 0.54 is the most accurate model measured. It is a Zipformer2 transducer over the whole utterance, and it beats even the 627M multilingual Parakeet. It is also the fastest of the models with more than 30M parameters.
- **No small model is good at both languages:**
  - Whisper base: 10.3 % in English, 20.5 % in Russian.
  - The multilingual FastConformer: 10.0 % and 10.2 %, but NVIDIA publishes it under the NGC terms of use, not an open licence.
  - Parakeet v3: 7.1 % and 8.0 %, but with 627M parameters it takes 670 MB in int8.
- **English.** The GigaSpeech Zipformer has the same architecture and the same parameter count as the Russian Vosk model (65.0M). It gives 11.3 % in fp32 and 12.0 % with its published int8 weights. The alternatives:
  - Whisper base is 1 point better and about 9 times slower;
  - Parakeet 110M (CC BY 4.0) is 2.4 points better, with 109M parameters.
- **Several published int8 exports are broken or lose a lot,** so our own quantization must be checked against fp32 (acceptance criteria):
  - Whisper tiny: 23.6 % in int8 against 10.7 % in fp32, on the first 60 English sentences.
  - The streaming 65M Vosk model: 20.6 % against 9.5 %.
  - The small non-streaming Vosk model takes only one input length (2337 frames), in int8 and in fp32. It could not be measured; alphacep gives 9.8 % on Common Voice.
- **Streaming starts.** The streaming 65M Vosk model also began some sentences with words of a voice assistant («поставь», «хочу»). Half a second of silence before the speech did not change that.
- **English Zipformers trained on audiobooks only** (LibriSpeech, Libriheavy) fail on FLEURS, at 37–71 %, though they decode their own samples correctly.
- **Left out:**
  - the Kaldi models: their TDNN acoustic model and WFST decoder are the most code to port, and they are slow;
  - Kroko: its licence is unclear;
  - GigaAM v3 from the compact variant: 220–240M parameters, beyond its budget. It was measured later as the quality variant (below);
  - T-one: Russian only, telephone sound at 8 kHz, and less accurate here than Vosk 0.54. It remains the simplest to decode, with CTC over 34 letters.

### Two variants (2026-10-08)

The maintainer asked for two variants of each model, one optimized for size and one for accuracy, without models that do badly. Any free licence is allowed; terms that limit use are recorded with each model ([issues-human](../issues-human/README.md), section 6). The models are kept in the model cache of [251](251-model-cache-and-model-disk.md).

GigaAM v3 (Salute Developers, MIT), measured the same way on the 272 Russian sentences:

| Model | Kind | Size (int8) | WER % | CER % | Time | Weights licence |
|---|---|---|---|---|---|---|
| **GigaAM v3 RNNT** | Conformer transducer, Russian | 229 MB | **3.02** | 0.82 | ≈0.122 | MIT |
| GigaAM v3 CTC | Conformer CTC, Russian | 225 MB | 3.31 | 0.87 | ≈0.117 | MIT |

≈: timed during the accuracy run, not alone.

| Language | `compact` | `quality` |
|---|---|---|
| Russian | `vosk-model-ru` 0.54: 5.20 %, 73 MB | GigaAM v3 RNNT: 3.02 %, 229 MB |
| English | Zipformer GigaSpeech, int8: 11.98 %, 74 MB | Parakeet TDT 0.6B v3: 8.04 %, 671 MB (also 7.08 % in Russian) |

The compact models share one engine (step 3 of the decision below). The quality models are a Conformer and a FastConformer transducer, which need a second engine after the first.

### What each family takes to port

| Family | Models | What has to be written |
|---|---|---|
| Zipformer2 transducer | Vosk 0.54 (ru), GigaSpeech (en) | 80-band Kaldi fbank; a convolutional front end; six encoder stacks at 50 to 6.25 frames a second, with downsampling and upsampling; in each layer, attention weights shared by two attention modules and a non-linear attention module, two convolution modules and three feed-forward modules, BiasNorm and the SwooshL/R activations; relative positions; a stateless decoder (an embedding and a convolution over the last two tokens); the joiner; greedy search over 500 BPE pieces. The reference is icefall (Apache-2.0). It is the largest encoder of the four, but one engine serves both languages. |
| Conformer CTC | T-one, Parakeet 110M, NeMo | A conformer encoder; decoding is the best symbol of each frame. |
| Encoder-decoder | Whisper, Moonshine | An encoder and an autoregressive decoder with a cache of keys and values; a 50k byte-level BPE vocabulary. Whisper always encodes 30 s of audio, and it can invent text in silence. |
| Kaldi | Vosk 0.22 | A TDNN-F acoustic model, i-vectors, and a lattice decoder over WFST graphs (HCLG). |

### Decision

1. **One engine:** a Zipformer2 transducer over the whole utterance with greedy search. It lives in `mind::voice`, which the host tests and the system share as in V1. Weights are int8 with per-channel scales, and matrix products are in integers (AVX2 on x86, NEON on aarch64).
2. **Russian first:** alphacep `vosk-model-ru` 0.54 (Apache-2.0). It has 65.0M parameters, about 66 MB of int8 weights, which fits the plan's 40–80 MB.
3. **English with the same engine:** the k2-fsa Zipformer GigaSpeech 2023-12-12 (weights under Apache-2.0).
   - GigaSpeech's audio is licensed for non-commercial research and education only. The maintainer decided on 2026-10-08 that any free model may ship, and the terms travel with the model ([issues-human](../issues-human/README.md), section 6).
   - Parakeet 110M (CC BY 4.0) and Whisper base (MIT) stay alternatives; each needs a second engine.
4. **Then the quality variant:** a Conformer transducer for GigaAM v3 (Russian) and Parakeet TDT 0.6B v3 (English), from the model disk of 251.
5. **Later, text that appears while one speaks:** the streaming Vosk models (small: 23M parameters, 10.6 %; 65M: 9.5 %). They need chunked attention with caches on top of the same layers.

## Plan

1. **Model files.** A host converter turns the published fp32 ONNX into `voice/dictate-ru.bin`: a header, the BPE pieces, int8 weights with per-channel scales and a checksum.
   - The 66 MB file is not committed. The model cache of 251 (`scripts/models.py`) fetches the pinned revision and checks its SHA-256; the converter reads it from there.
   - THIRD_PARTY.md records the source and the licence.
   - The image builder puts the file on the boot disk.
2. **Features.** 80 log-mel bands as Kaldi's fbank computes them: 25 ms Povey window, 10 ms shift, 20 Hz to 7.6 kHz. They are checked against sherpa-onnx's features on the host.
3. **The network.** The encoder, decoder and joiner go in `mind::voice::zipformer` (`no_std`, `alloc`), with greedy search over the 500 BPE pieces. Host test: our text equals onnxruntime's fp32 text on a set of clips.
4. **Quantization.** WER on this issue's Russian sentences stays within 0.5 points of the published model, measured by hand with `scripts/voice_v3`.
5. **Memory.** The model is one read-only memory object shared by every recognizer (150), and the memory of one recognition is bounded.
6. **`dictate`.** Push-to-talk utterances become text: `edit` inserts it at the cursor, and `fm` puts it in its search field.
   - Recognized text is data: nothing in it is run (MC-11.5).
   - The recognizer holds the audio, the model and its line to the program it serves, and no other authority (MC-8.1, MC-11.11).
7. **QEMU suite.** A clip on the boot disk gives the expected text on x86 and aarch64.
8. **English.** The same steps for the English model.

Tasks are numbered `250-APP-MMMM` as they start.

## Progress

- **Step 2, the features: done** in [250-APP-0020](../issues-done/250-APP-0020-dictation-features.done). `mind::voice::fbank` gives kaldi-native-fbank's features to within 2·10⁻⁴ on the host and in the system (x86).
- **Steps 1 and 3, first part: done** in [250-APP-0021](../issues-done/250-APP-0021-network-interpreter.done). `convert.py` writes the model as a `MINDNN01` file, and `mind::nn` interprets its graphs. On ten FLEURS clips, greedy search gives onnxruntime's text but for one word in 193, at 0.59 of real time on one core without SIMD. Next come fast products, the run in the system and WER on all 272 sentences.
- **Step 3, speed: done** in [250-APP-0022](../issues-done/250-APP-0022-fast-products.done). With AVX2 products (exact in int8), weights in panels and strided element-wise operators, the ten clips take 0.095 of real time on one core of this host, and an 8.3 s clip 0.68 s. VNNI and NEON are left. Next come the run in the system and WER on all 272 sentences.
- **How the engines compute (2026-10-09).** Programs build for soft-float targets.
  - On x86, `x86_64-unknown-none` refuses SSE in a program's code: the `x86_softfloat_sse` lint is to become a hard error, so `#[target_feature]` is no way out. The engines therefore build for `targets/x86_64-mind-float.json`: SSE2 and the hard-float ABI, with core and alloc built from source (`rust-src`, now in `rust-toolchain.toml`). AVX2 and FMA come through `#[target_feature]` after a CPUID check, and the kernel saves their state ([153](../issues-done/153-xsave-avx-state.done)).
  - On aarch64, FP/SIMD is disabled at EL0. A request to the kernel track is in [requests-KRN.md](requests-KRN.md); until it is done, the engines run on x86 only.

## Acceptance criteria

- On the test clips, the system gives the same text as the host build, on x86 and aarch64 (QEMU).
- With our int8 weights, WER on the 272 Russian FLEURS sentences is within 0.5 points of the 5.20 % measured here.
- On the host build, an utterance of 8 s is recognized in under 1 s on one core (x86 with AVX2).
- Nothing uses the network: audio does not leave the machine.
- Each model's source, revision, hash, licence and terms are in `models/manifest.toml`, and THIRD_PARTY.md points to it.

## Related

[docs/voice](../docs/voice/README.md) (V3), [251](251-model-cache-and-model-disk.md), [078](../issues-done/078-voice-command-recognizer.done), [079](../issues-done/079-voice-control-in-the-shell.done), [150](../issues-done/150-user-memory-beyond-the-arena.done), [153](../issues-done/153-xsave-avx-state.done), [scripts/voice_v3](../scripts/voice_v3/README.md).
