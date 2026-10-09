# Speech synthesis models: how they were compared

These scripts measured the voices compared in main task [252](../../issues/252-neural-speech-synthesis.md). They run on a host with Python 3.11 and the network. Nothing here is built into MIND Core.

## Steps

```bash
W=/path/to/work                       # about 12 GB: models, speech, results
A=/path/to/asr-work                   # the work directory of scripts/voice_v3, with its recognizers and FLEURS data
cp scripts/voice_tts/sentences.tsv scripts/voice_tts/refs.json $W/
python3 -m venv $W/venv-onnx          # Piper, Kokoro, Kitten, Vosk TTS
$W/venv-onnx/bin/pip install sherpa-onnx==1.13.8 vosk-tts==0.3.61 soundfile==0.14.0 numpy jiwer==4.0.0 huggingface_hub
python3 -m venv $W/venv-torch         # Pocket TTS, ESpeech, Qwen3-TTS, Chatterbox, UTMOSv2 (CPU)
$W/venv-torch/bin/pip install --index-url https://download.pytorch.org/whl/cpu torch torchaudio torchvision
$W/venv-torch/bin/pip install pocket-tts==3.3.0 f5-tts==1.1.22 ruaccent==1.5.8.3 qwen-tts==0.1.1 'datasets<4' \
  huggingface_hub==0.36.2 transformers==4.57.3 soundfile==0.14.0
$W/venv-torch/bin/pip install --no-deps chatterbox-tts==0.1.7 diffusers==0.29.0 s3tokenizer==0.3.0 conformer==0.3.2 resemble-perth==1.0.1
$W/venv-torch/bin/pip install git+https://github.com/sarulab-speech/UTMOSv2@cc2700db57bb83ee13dc31ebe1b868c254e15d09
PYTHON=$W/venv-onnx/bin/python scripts/voice_tts/fetch_all.sh $W
export HF_HOME=$W/dl/hf-home ASR=$A
$W/venv-onnx/bin/python -I scripts/voice_tts/synth_onnx.py $W vosk-tts-0.9-3        # one thread
FLEURS=4 $W/venv-torch/bin/python -I scripts/voice_tts/synth_torch.py $W chatterbox ru   # four threads
$W/venv-onnx/bin/python -I scripts/voice_tts/intelligibility.py $W $A vosk-tts-0.9-3 chatterbox-ru
$W/venv-torch/bin/python -I scripts/voice_tts/utmos.py $W vosk-tts-0.9-3 chatterbox-ru
$W/venv-onnx/bin/python -I scripts/voice_tts/common.py $W
python3 scripts/voice_tts/page.py $W voices.html
```

- `sentences.tsv`: 24 sentences per language.
  - Four for listening: two of the system's own phrases, one with homographs (`замок`, `мукой`; `read`, `lead`), and one FLEURS sentence.
  - Twenty FLEURS test sentences (Google, CC BY 4.0) without digits.
- `synth_onnx.py <work> <system>`: the ONNX voices on one thread. The system names a voice:
  - `piper-<voice>`;
  - `kokoro-<voice>`;
  - `kitten-<nano|mini>-<speaker>`;
  - `vosk-tts-<0.7|0.9>-<speaker>`.

  It writes `out/<system>/<id>.wav` and `timing.json`, with the time per second of speech and the size of the model's files.
- `synth_torch.py <work> <pocket|espeech|qwen3|chatterbox> <lang>`: the PyTorch voices on four threads, into `out/<system>-<lang>/`.
  - `FLEURS=N` keeps the first N FLEURS sentences; the slowest models read four.
  - ESpeech and Qwen3-TTS copy the voice of the FLEURS recording named in `refs.json`.
- `intelligibility.py <work> <asr-work> <system>...`: the best recognizers of 250 transcribe the speech.
  - Russian: Vosk 0.54 int8. English: Parakeet TDT-CTC 110M.
  - It compares the transcript with the text, normalized as in 250, and appends CER and WER to `results/asr.jsonl`.
- `utmos.py <work> <system>...`: the naturalness that UTMOSv2 (fold 0) predicts, from 1 to 5, into `results/utmos.jsonl`.
- `common.py <work>`: CER, WER and UTMOS on the eight sentences every voice read, into `results/common.json`.
- `page.py <work> <out.html>`: a listening page with every voice reading the four listening sentences, and the measurements.
- [`results-2026-10-08.jsonl`](results-2026-10-08.jsonl) holds every measurement of the comparison in 252.
- `vits_reference.py <model.onnx | vosk-model-dir> "text" <out> [--vosk SPEAKER] [--dump]`: onnxruntime's audio of a VITS voice with the noise scales at 0, and its inputs, for `vits_against_onnxruntime` in `tests/nn_host.rs` (MIND_VITS_MODEL, a voice converted with `scripts/voice_dictate/convert.py` as the graph `vits`; MIND_VITS_REFERENCE; MIND_VITS_DUMP for every value). Piper's phonemes come from piper-phonemize (espeak-ng, GPL-3.0, on the host only); Vosk TTS's from vosk-tts.
- `dictionary.py <vosk-model-dir> <out.dic>`: Vosk TTS's Russian dictionary (2 million words, 101 MB) as a MINDDIC1 file of 13 MB for `mind::voice::russian`: per word the vowels stressed and the е that are ё, from which vosk-tts's rules give its phonemes (99 % of the words), else the phonemes; each entry checked against the dictionary.
- `vosk_ids.py <vosk-model-dir> sentences.tsv <out.txt>`: vosk-tts's phoneme ids of the Russian sentences, for `sentences_as_vosk_tts` in `tests/russian_host.rs` (MIND_TTS_DICTIONARY, MIND_TTS_IDS).
- The model disk with the voice: `models.py disk OUT.img tts-ru-vosk-0.7 --add tts-ru-vosk-0.7/voice.bin=<converted model> --add tts-ru-vosk-0.7/russian.dic=<dictionary>`; `speak` uses both.

## Sources and licences

- **Sentences:** the FLEURS sentences and the two reference recordings are from FLEURS (Google, CC BY 4.0); the listening sentences are this project's.
- **Models:** each keeps its own licence, listed in 252. They are downloaded, not stored here.
- **Python packages:**
  - sherpa-onnx, vosk-tts, jiwer, transformers, diffusers, huggingface_hub, qwen-tts, ruaccent: Apache-2.0;
  - piper-phonemize: MIT, with espeak-ng inside (GPL-3.0); used on the host only, for references;
  - f5-tts, chatterbox-tts, pocket-tts, UTMOSv2: MIT;
  - PyTorch, soundfile, numpy: BSD-3-Clause.
