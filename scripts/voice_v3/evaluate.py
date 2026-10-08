# evaluate.py <work> <model> <lang> [N]: decode <work>/data/<lang> on one thread; N alone times the first N sentences.
import json, os, sys, time, unicodedata
from pathlib import Path
import numpy as np
import soundfile as sf
import jiwer

root, model, lang = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
limit = int(sys.argv[4]) if len(sys.argv) > 4 else 0
dl = root / "dl"
LEAD = float(os.environ.get("LEAD", "0.5"))  # silence before a streaming model's input, as a microphone gives
TAG = os.environ.get("TAG", "")  # names a probe run


def files(d, *names):
    return [str(dl / d / n) for n in names]


def offline(make):
    rec = make()
    def run(audio):
        s = rec.create_stream(); s.accept_waveform(16000, audio); rec.decode_stream(s)
        return s.result.text
    return run


def online(make):
    rec = make()
    def run(audio):
        s = rec.create_stream()
        s.accept_waveform(16000, np.zeros(int(LEAD * 16000), dtype=np.float32))  # a microphone stream starts in silence
        s.accept_waveform(16000, audio)
        s.accept_waveform(16000, np.zeros(int(0.8 * 16000), dtype=np.float32)); s.input_finished()
        while rec.is_ready(s):
            rec.decode_stream(s)
        return rec.get_result(s)
    return run


def vosk_kaldi(d):
    import vosk
    vosk.SetLogLevel(-1)
    path = next(p for p in (dl / d).iterdir() if p.is_dir())
    m = vosk.Model(str(path))
    def run(audio):
        r = vosk.KaldiRecognizer(m, 16000)
        r.AcceptWaveform((np.clip(audio, -1, 1) * 32767).astype("<i2").tobytes())
        return json.loads(r.FinalResult())["text"]
    return run, [p for p in path.rglob("*") if p.is_file()]


def build():
    import sherpa_onnx as so
    O, N = so.OfflineRecognizer, so.OnlineRecognizer
    zf = lambda d: files(d, "encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt")
    if model == "vosk-kaldi-small-ru":
        return vosk_kaldi("vosk-kaldi-small-ru")
    if model == "vosk-kaldi-small-en":
        return vosk_kaldi("vosk-kaldi-small-en")
    if model == "vosk-zf-small-ru-fp32":  # the int8 export has a fixed length (2337 frames)
        e, d, j, t = files(model, "encoder.onnx", "decoder.onnx", "joiner.onnx", "tokens.txt")
        return offline(lambda: O.from_transducer(e, d, j, t, num_threads=1)), [e, d, j, t]
    if model in ("vosk-zf-small-ru", "vosk-zf-ru"):
        e, d, j, t = zf(model)
        return offline(lambda: O.from_transducer(e, d, j, t, num_threads=1)), [e, d, j, t]
    if model == "vosk-zf-streaming-ru-fp32":
        e, d, j, t = files(model, "encoder.onnx", "decoder.onnx", "joiner.onnx", "tokens.txt")
        return online(lambda: N.from_transducer(t, e, d, j, num_threads=1)), [e, d, j, t]
    if model in ("vosk-zf-small-streaming-ru", "vosk-zf-streaming-ru"):
        e, d, j, t = zf(model)
        return online(lambda: N.from_transducer(t, e, d, j, num_threads=1)), [e, d, j, t]
    if model == "t-one":
        m, t = files("t-one", "model.onnx", "tokens.txt")
        return online(lambda: N.from_t_one_ctc(t, m, num_threads=1)), [m, t]
    if model.startswith("whisper-"):
        size = model.split("-")[1]
        q = "" if model.endswith("-fp32") else ".int8"
        e, d, t = files(model, f"{size}-encoder{q}.onnx", f"{size}-decoder{q}.onnx", f"{size}-tokens.txt")
        return offline(lambda: O.from_whisper(e, d, t, language=lang, num_threads=1)), [e, d, t]
    if model.startswith("moonshine-"):
        p, e, u, c, t = files(model, "preprocess.onnx", "encode.int8.onnx", "uncached_decode.int8.onnx", "cached_decode.int8.onnx", "tokens.txt")
        return offline(lambda: O.from_moonshine(p, e, u, c, t, num_threads=1)), [p, e, u, c, t]
    if model == "zf-en-20m":
        e, d, j, t = files(model, "encoder-epoch-99-avg-1.int8.onnx", "decoder-epoch-99-avg-1.int8.onnx", "joiner-epoch-99-avg-1.int8.onnx", "tokens.txt")
        return online(lambda: N.from_transducer(t, e, d, j, num_threads=1)), [e, d, j, t]
    if model == "zf-en-libriheavy-small":
        e, d, j, t = files(model, "encoder-epoch-90-avg-20.int8.onnx", "decoder-epoch-90-avg-20.int8.onnx", "joiner-epoch-90-avg-20.int8.onnx", "tokens.txt")
        return offline(lambda: O.from_transducer(e, d, j, t, num_threads=1)), [e, d, j, t]
    if model in ("zf-en-gigaspeech", "zf-en-gigaspeech-int8"):
        q = ".int8" if model.endswith("-int8") else ""
        e, d, j, t = files("zf-en-gigaspeech", f"encoder-epoch-30-avg-1{q}.onnx", f"decoder-epoch-30-avg-1{q}.onnx", f"joiner-epoch-30-avg-1{q}.onnx", "tokens.txt")
        return offline(lambda: O.from_transducer(e, d, j, t, num_threads=1)), [e, d, j, t]
    if model == "kroko-en":
        e, d, j, t = files(model, "encoder.onnx", "decoder.onnx", "joiner.onnx", "tokens.txt")
        return online(lambda: N.from_transducer(t, e, d, j, num_threads=1)), [e, d, j, t]
    if model in ("nemo-ml-fc", "nemo-en-conformer-small", "nemo-en-conformer-medium"):
        m, t = files(model, "model.onnx", "tokens.txt")
        return offline(lambda: O.from_nemo_ctc(m, t, num_threads=1)), [m, t]
    if model == "parakeet-110m":
        m, t = files(model, "model.onnx", "tokens.txt")
        return offline(lambda: O.from_nemo_ctc(m, t, num_threads=1)), [m, t]
    if model == "parakeet-0.6b-v3":
        e, d, j, t = zf(model)
        return offline(lambda: O.from_transducer(e, d, j, t, num_threads=1, model_type="nemo_transducer")), [e, d, j, t]
    raise SystemExit(f"unknown model {model}")


def norm(text):
    text = unicodedata.normalize("NFC", text).lower().replace("ё", "е")
    text = "".join(c if c.isalpha() or c.isdigit() or c == "'" else " " for c in text)
    return " ".join(w.strip("'") for w in text.split() if w.strip("'"))


run, used = build()
size = sum(Path(p).stat().st_size for p in used)
data = root / "data" / lang
refs = [l.rstrip("\n").split("\t") for l in open(data / "refs.tsv", encoding="utf-8")]
if limit:
    refs = refs[:limit]
run(sf.read(data / refs[0][0], dtype="float32")[0])  # warm-up
out = root / "results"; out.mkdir(exist_ok=True)
hyps, gold, spent, audio_s = [], [], 0.0, 0.0
with open(out / f"{model}{TAG}.{lang}{'.speed' if limit else ''}.tsv", "w", encoding="utf-8") as f:
    for name, dur, raw in refs:
        audio = sf.read(data / name, dtype="float32")[0]
        t0 = time.perf_counter(); text = run(audio); spent += time.perf_counter() - t0
        audio_s += float(dur)
        r, h = norm(raw), norm(text)
        gold.append(r); hyps.append(h)
        f.write(f"{name}\t{r}\t{h}\n")
summary = {"model": model + TAG, "lang": lang, "n": len(refs), "audio_s": round(audio_s, 1), "decode_s": round(spent, 1),
           "rtf": round(spent / audio_s, 4), "wer": round(100 * jiwer.wer(gold, hyps), 2), "cer": round(100 * jiwer.cer(gold, hyps), 2),
           "size_mb": round(size / 1e6, 1)}
print(json.dumps(summary, ensure_ascii=False), flush=True)
with open(out / ("speed.jsonl" if limit and not TAG else "probe.jsonl" if TAG else "summary.jsonl"), "a", encoding="utf-8") as f:
    f.write(json.dumps(summary, ensure_ascii=False) + "\n")
