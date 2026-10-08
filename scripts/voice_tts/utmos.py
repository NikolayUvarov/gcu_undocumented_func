# utmos.py <work> <system>...: predicted MOS (UTMOSv2, fold 0; 1..5) of each system's sentences; results/utmos.jsonl.
import json, sys, warnings
from pathlib import Path
import numpy as np
import soundfile as sf
import torch
import utmosv2

warnings.filterwarnings("ignore")
root = Path(sys.argv[1])
torch.set_num_threads(4)
model = utmosv2.create_model(pretrained=True, device="cpu")
out = root / "results"; out.mkdir(exist_ok=True)
for system in sys.argv[2:]:
    d = root / "out" / system
    scores = {}
    for wav in sorted(d.glob("*.wav")):
        a, r = sf.read(wav, dtype="float32")
        if a.ndim > 1: a = a.mean(axis=1)
        scores[wav.stem] = float(np.asarray(model.predict(data=np.ascontiguousarray(a), sr=r, device="cpu", verbose=False)).reshape(-1)[0])
    lang = json.load(open(d / "timing.json"))["lang"]
    res = {"system": system, "lang": lang, "n": len(scores), "utmos": round(sum(scores.values()) / len(scores), 3)}
    json.dump(scores, open(d / "utmos.json", "w"), indent=0)
    with open(out / "utmos.jsonl", "a", encoding="utf-8") as f: f.write(json.dumps(res) + "\n")
    print(json.dumps(res), flush=True)
