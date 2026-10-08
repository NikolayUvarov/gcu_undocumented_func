# prep.py <dl/fleurs-xx> <out-dir> <count>: one recording per sentence, no digits in the reference, 16 kHz mono WAV.
import csv, io, random, re, sys, tarfile
from pathlib import Path
import numpy as np
import soundfile as sf

src, out, count = Path(sys.argv[1]), Path(sys.argv[2]), int(sys.argv[3])
out.mkdir(parents=True, exist_ok=True)
rows = {}
with open(src / "test.tsv", encoding="utf-8") as f:
    for r in csv.reader(f, delimiter="\t", quoting=csv.QUOTE_NONE):
        sid, name, raw = r[0], r[1], r[2]
        if re.search(r"\d", raw) or sid in rows:
            continue
        rows[sid] = (name, raw)
picked = sorted(rows.items())
random.Random(20261008).shuffle(picked)
picked = dict((v[0], (k, v[1])) for k, v in picked[:count])
found = 0
with tarfile.open(src / "test.tar.gz") as tar, open(out / "refs.tsv", "w", encoding="utf-8") as refs:
    for m in tar:
        name = m.name.rsplit("/", 1)[-1]
        if name not in picked:
            continue
        audio, rate = sf.read(io.BytesIO(tar.extractfile(m).read()), dtype="float32")
        if audio.ndim > 1:
            audio = audio.mean(axis=1)
        assert rate == 16000, rate
        sf.write(out / name, audio, rate, subtype="PCM_16")
        refs.write(f"{name}\t{len(audio) / rate:.3f}\t{picked[name][1]}\n")
        found += 1
print(f"{found} utterances, {sum(1 for _ in open(out / 'refs.tsv'))} refs")
