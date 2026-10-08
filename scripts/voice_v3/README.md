# Speech recognition models for V3: how they were compared

These scripts measured the models compared in main task [250](../../issues/250-voice-dictation.md). They run on a host with Python 3.11 and the network. Nothing here is built into MIND Core.

## Steps

```bash
W=/path/to/work                       # about 5 GB: test sets, models, results
python3 -m venv $W/venv
$W/venv/bin/pip install sherpa-onnx==1.13.8 vosk==0.3.45 soundfile==0.14.0 numpy jiwer==4.0.0 onnx==1.23.2
scripts/voice_v3/fetch_all.sh $W      # FLEURS ru/en test sets and every model, each in its own directory
for l in ru en; do $W/venv/bin/python -I scripts/voice_v3/prep.py $W/dl/fleurs-$l $W/data/$l 300; done
PYTHON=$W/venv/bin/python scripts/voice_v3/run_acc.sh $W 3 vosk-zf-ru:ru zf-en-gigaspeech:en   # accuracy, 3 at a time
$W/venv/bin/python -I scripts/voice_v3/evaluate.py $W vosk-zf-ru ru 40                          # speed, alone
$W/venv/bin/python -I scripts/voice_v3/graph.py $W/dl/t-one/model.onnx                          # parameters and operators
```

- `prep.py` keeps one recording per sentence and leaves out sentences whose reference has digits. Of 300 sentences drawn with a fixed seed, 272 Russian and 281 English have audio in the test archive.
- `evaluate.py` knows each model by the name `fetch_all.sh` gives its directory. It decodes on one thread, normalizes both texts the same way (lower case, ё → е, no punctuation) and appends a line to `<work>/results/summary.jsonl`. With a count N it writes `speed.jsonl` instead. `TAG=...` names a probe run, and `LEAD=` sets the silence before a streaming model's input (0.5 s by default).
- [`results-2026-10-08.jsonl`](results-2026-10-08.jsonl) holds every line of the comparison in 250: accuracy, speed and the int8 against fp32 probes.

## Sources and licences

- **Test sets:** FLEURS (Google, CC BY 4.0). They are downloaded, not stored here.
- **Models:** each keeps its own licence, listed in 250. They are downloaded, not stored here.
- **Python packages:**
  - sherpa-onnx, vosk and jiwer: Apache-2.0;
  - soundfile: BSD-3-Clause;
  - numpy: BSD-3-Clause;
  - onnx: Apache-2.0.
