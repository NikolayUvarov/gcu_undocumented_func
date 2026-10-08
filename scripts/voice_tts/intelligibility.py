# intelligibility.py <work> <asr-work> <system>...: recognize each system's sentences, CER/WER against the text; results/asr.jsonl.
import json, sys, unicodedata
from pathlib import Path
import numpy as np
import soundfile as sf
import jiwer
import sherpa_onnx as so

root, asr = Path(sys.argv[1]), Path(sys.argv[2])
systems = sys.argv[3:]
dl = asr / "dl"


def recognizer(lang):
    if lang == "ru":
        d = dl / "vosk-zf-ru"
        return so.OfflineRecognizer.from_transducer(str(d / "encoder.int8.onnx"), str(d / "decoder.int8.onnx"), str(d / "joiner.int8.onnx"), str(d / "tokens.txt"), num_threads=4)
    d = dl / "parakeet-110m"
    return so.OfflineRecognizer.from_nemo_ctc(str(d / "model.onnx"), str(d / "tokens.txt"), num_threads=4)


def norm(text):
    text = unicodedata.normalize("NFC", text).lower().replace("ё", "е")
    text = "".join(c if c.isalpha() or c.isdigit() or c == "'" else " " for c in text)
    return " ".join(w.strip("'") for w in text.split() if w.strip("'"))


texts = {r[0]: (r[1], r[2]) for r in (l.rstrip("\n").split("\t") for l in open(root / "sentences.tsv", encoding="utf-8"))}
recs = {}
out = root / "results"; out.mkdir(exist_ok=True)
for system in systems:
    d = root / "out" / system
    lang = json.load(open(d / "timing.json"))["lang"]
    rec = recs.setdefault(lang, recognizer(lang))
    gold, hyp, per = [], [], {}
    for wav in sorted(d.glob("*.wav")):
        a, r = sf.read(wav, dtype="float32")
        if a.ndim > 1: a = a.mean(axis=1)
        s = rec.create_stream(); s.accept_waveform(r, np.ascontiguousarray(a)); rec.decode_stream(s)
        g, h = norm(texts[wav.stem][1]), norm(s.result.text)
        gold.append(g); hyp.append(h); per[wav.stem] = h
    res = {"system": system, "lang": lang, "n": len(gold), "cer": round(100 * jiwer.cer(gold, hyp), 2), "wer": round(100 * jiwer.wer(gold, hyp), 2)}
    json.dump(per, open(d / "asr.json", "w"), ensure_ascii=False, indent=0)
    with open(out / "asr.jsonl", "a", encoding="utf-8") as f: f.write(json.dumps(res) + "\n")
    print(json.dumps(res), flush=True)
