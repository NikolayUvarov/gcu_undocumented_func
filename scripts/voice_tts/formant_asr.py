# formant_asr.py <tts_host> <model-dir> <out-dir> [label]: tts's formant voice (tests/tts_host.rs) reads the 24 Russian
# sentences of sentences.tsv, and a recognizer transcribes each: Vosk's small Kaldi model (vosk-model-small-ru-0.22,
# the `vosk` package), or, for a directory holding encoder.int8.onnx, Vosk 0.54's transducer as 250 runs it
# (alphacep/vosk-model-ru am-onnx, the `sherpa-onnx` package). CER and WER against the text normalized as in 250, and
# the onsets that click (252-APP-0041): after at least 10 ms of silence, a rise past 2000 within 3 samples. One JSON
# line on stdout, the transcripts in <out-dir>/asr.tsv. Run with -I; both packages are Apache-2.0.
import json, subprocess, sys, unicodedata, wave
from pathlib import Path

tts, model_dir, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
label = sys.argv[4] if len(sys.argv) > 4 else ""
here = Path(__file__).resolve().parent
QUIET, SILENCE, LOUD, RISE = 100, 160, 2000, 3  # |x| under QUIET for SILENCE samples, then past LOUD within RISE


def norm(text):
    text = unicodedata.normalize("NFC", text).lower().replace("ё", "е")
    text = "".join(c if c.isalpha() or c.isdigit() or c == "'" else " " for c in text)
    return " ".join(w.strip("'") for w in text.split() if w.strip("'"))


def edits(a, b):
    row = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        prev, row[0] = row[0], i
        for j, y in enumerate(b, 1):
            prev, row[j] = row[j], min(row[j] + 1, row[j - 1] + 1, prev + (x != y))
    return row[-1]


def clicks(samples):
    count, quiet = 0, 0
    for i, s in enumerate(samples):
        if abs(s) < QUIET:
            quiet += 1
            continue
        if quiet >= SILENCE and any(abs(v) > LOUD for v in samples[i:i + RISE]):
            count += 1
        quiet = 0
    return count


if (model_dir / "encoder.int8.onnx").exists():
    import numpy as np, sherpa_onnx
    zf = sherpa_onnx.OfflineRecognizer.from_transducer(*(str(model_dir / f) for f in ("encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt")), num_threads=1)

    def transcribe(raw):
        stream = zf.create_stream()
        stream.accept_waveform(16000, np.frombuffer(raw, dtype="<i2").astype("float32") / 32768)
        zf.decode_stream(stream)
        return stream.result.text
else:
    import vosk
    vosk.SetLogLevel(-1)
    model = vosk.Model(str(model_dir))

    def transcribe(raw):
        rec = vosk.KaldiRecognizer(model, 16000)
        rec.AcceptWaveform(raw)
        return json.loads(rec.FinalResult()).get("text", "")
out.mkdir(parents=True, exist_ok=True)
rows = [l.rstrip("\n").split("\t") for l in open(here / "sentences.tsv", encoding="utf-8")]
chars = char_errors = words = word_errors = onset_clicks = 0
lines = []
for ident, lang, text in rows:
    if lang != "ru":
        continue
    path = out / f"{ident}.wav"
    subprocess.run([str(tts), text, str(path)], check=True, capture_output=True)
    with wave.open(str(path)) as w:
        assert w.getframerate() == 16000 and w.getnchannels() == 1 and w.getsampwidth() == 2
        raw = w.readframes(w.getnframes())
    samples = [int.from_bytes(raw[i:i + 2], "little", signed=True) for i in range(0, len(raw), 2)]
    heard = transcribe(raw)
    gold, hyp = norm(text), norm(heard)
    chars += len(gold); char_errors += edits(gold, hyp)
    words += len(gold.split()); word_errors += edits(gold.split(), hyp.split())
    onset_clicks += clicks(samples)
    lines.append(f"{ident}\t{gold}\t{hyp}")
(out / "asr.tsv").write_text("\n".join(lines) + "\n", encoding="utf-8")
print(json.dumps({"label": label, "sentences": len(lines), "cer": round(100 * char_errors / chars, 1),
                  "wer": round(100 * word_errors / words, 1), "onset_clicks": onset_clicks}, ensure_ascii=False))
