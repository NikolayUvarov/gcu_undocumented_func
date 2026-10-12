# Dictation models: conversion and references (250)

Host tools for the dictation engine of main task [250](../../issues/250-voice-dictation.md). Nothing here is built into MIND Core; `convert.py`'s output is the file the system reads.

```bash
python3 -m venv venv && venv/bin/pip install onnx==1.23.2 onnxruntime==1.30.0 numpy kaldi-native-fbank==1.22.3 soundfile
python3 scripts/models.py fetch asr-ru-vosk-0.54            # the cache of 251: $MIND_MODELS, else ~/.cache/mind-models
M=${MIND_MODELS:-~/.cache/mind-models}/asr-ru-vosk-0.54
venv/bin/python -I scripts/voice_dictate/convert.py dictate-ru.bin encoder=$M/am-onnx/encoder.int8.onnx \
  decoder=$M/am-onnx/decoder.int8.onnx joiner=$M/am-onnx/joiner.int8.onnx --tokens $M/lang/tokens.txt
venv/bin/python -I scripts/voice_dictate/reference.py MODEL_DIR clip.wav refs/clip      # one directory per clip
MIND_DICTATE_MODEL=dictate-ru.bin MIND_DICTATE_REFERENCE=refs rustc ... tests/nn_host.rs  # see the test's comments
```

- `fbank_reference.py OUT`: kaldi-native-fbank's features of an integer test signal, for `tests/fbank_reference.txt`.
- `convert.py OUT NAME=model.onnx... [--tokens tokens.txt]`: ONNX graphs to a `MINDNN01` file (the format is in the script's docstring and in `libmind/src/nn/mod.rs`). Version 2 lays out `MatMulInteger`'s weights in panels for `mind::nn::gemm`; files of version 1 must be converted again.
- `toy.py OUT`: a toy transducer with the models' interface and random weights, converted to `tests/dictate_toy.bin` (9.6 KB), so that the host tests and the `tools` suite run `dictate`'s whole chain without the 71 MB model.
- `models.py disk OUT.img asr-ru-vosk-0.54 --add asr-ru-vosk-0.54/dictate.bin=dictate-ru.bin`: the model disk with the converted model, which `dictate` uses by default after checking its SHA-256 against the disk's `MANIFEST.json`.
- `MIND_DICTATE_PROFILE=1` with the model and one clip's directory: `profile_by_operator` in `tests/nn_host.rs` prints the encoder's time by operator and its slowest nodes.
- `reference.py MODEL_DIR clip.wav OUT_DIR`: features, onnxruntime's encoder output, and greedy search's tokens and text.
- `dump.py model.onnx features.f32 FRAMES OUT_DIR`: every value onnxruntime makes, to find where `mind::nn` departs from it (with `MIND_DICTATE_DUMP` in `tests/nn_host.rs`).

## Licences

The tools are used on the host only: onnx and onnxruntime (MIT), kaldi-native-fbank (Apache-2.0), numpy (BSD-3-Clause), soundfile (BSD-3-Clause). The model keeps its own licence (`models/manifest.toml`: Apache-2.0 for `asr-ru-vosk-0.54`).
