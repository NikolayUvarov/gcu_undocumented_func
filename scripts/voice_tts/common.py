# common.py <work>: CER and UTMOS of every system on the sentences all of them read (L1-L4, F01-F04; L1-L4 for the
# slowest), into results/common.json.
import json, sys, unicodedata
from pathlib import Path
import jiwer

root = Path(sys.argv[1])
COMMON = [f"L{i}" for i in range(1, 5)] + [f"F{i:02d}" for i in range(1, 5)]
texts = {r[0]: r[2] for r in (l.rstrip("\n").split("\t") for l in open(root / "sentences.tsv", encoding="utf-8"))}


def norm(text):
    text = unicodedata.normalize("NFC", text).lower().replace("ё", "е")
    text = "".join(c if c.isalpha() or c.isdigit() or c == "'" else " " for c in text)
    return " ".join(w.strip("'") for w in text.split() if w.strip("'"))


res = {}
for d in sorted((root / "out").iterdir()):
    if not (d / "asr.json").exists():
        continue
    lang = json.load(open(d / "timing.json"))["lang"]
    hyp, mos = json.load(open(d / "asr.json")), json.load(open(d / "utmos.json")) if (d / "utmos.json").exists() else {}
    ids = [f"{lang}-{c}" for c in COMMON]
    if not all(i in hyp for i in ids):
        ids = ids[:4]  # a model too slow for more read the four listening sentences only
        if not all(i in hyp for i in ids):
            print("incomplete", d.name); continue
    gold = [norm(texts[i]) for i in ids]
    r = {"lang": lang, "n": len(ids), "cer8": round(100 * jiwer.cer(gold, [hyp[i] for i in ids]), 2), "wer8": round(100 * jiwer.wer(gold, [hyp[i] for i in ids]), 2)}
    if all(i in mos for i in ids):
        r["utmos8"] = round(sum(mos[i] for i in ids) / len(ids), 3)
    res[d.name] = r
json.dump(res, open(root / "results" / "common.json", "w"), indent=1)
for k, v in res.items(): print(k, v)
