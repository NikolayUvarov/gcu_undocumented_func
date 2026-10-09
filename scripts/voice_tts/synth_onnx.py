# synth_onnx.py <work> <system>: synthesize sentences.tsv of the system's language on one thread; out/<system>/<id>.wav, timing.json.
import json, re, sys, time
from pathlib import Path
import numpy as np
import soundfile as sf

root, system = Path(sys.argv[1]), sys.argv[2]
dl = root / "dl"
KOKORO = {"af_heart": 3, "af_bella": 2, "am_michael": 16, "am_fenrir": 14, "bf_emma": 21, "bm_george": 26}


def sherpa(model_cfg):
    import sherpa_onnx as so
    tts = so.OfflineTts(so.OfflineTtsConfig(model=model_cfg, max_num_sentences=1))
    return tts


def build():
    import sherpa_onnx as so
    if system.startswith("piper-"):
        v = system[len("piper-"):]
        d = dl / f"piper-{v}"
        cfg = so.OfflineTtsModelConfig(vits=so.OfflineTtsVitsModelConfig(model=str(next(d.glob("*.onnx"))), tokens=str(d / "tokens.txt"), data_dir=str(d / "espeak-ng-data")), num_threads=1)
        tts = sherpa(cfg)
        return ("ru" if v.startswith("ru") else "en"), lambda text: tts.generate(text, sid=0, speed=1.0), [next(d.glob("*.onnx"))]
    if system.startswith("kokoro-"):
        d = dl / "kokoro"
        cfg = so.OfflineTtsModelConfig(kokoro=so.OfflineTtsKokoroModelConfig(model=str(d / "model.onnx"), voices=str(d / "voices.bin"), tokens=str(d / "tokens.txt"), data_dir=str(d / "espeak-ng-data"), lexicon=f"{d}/lexicon-us-en.txt"), num_threads=1)
        tts = sherpa(cfg)
        sid = KOKORO[system[len("kokoro-"):]]
        return "en", lambda text: tts.generate(text, sid=sid, speed=1.0), [d / "model.onnx", d / "voices.bin"]
    if system.startswith("kitten-"):
        size, voice = system.split("-")[1], int(system.split("-")[2])
        d = dl / f"kitten-{size}"
        cfg = so.OfflineTtsModelConfig(kitten=so.OfflineTtsKittenModelConfig(model=str(d / "model.fp16.onnx"), voices=str(d / "voices.bin"), tokens=str(d / "tokens.txt"), data_dir=str(d / "espeak-ng-data")), num_threads=1)
        tts = sherpa(cfg)
        return "en", lambda text: tts.generate(text, sid=voice, speed=1.0), [d / "model.fp16.onnx", d / "voices.bin"]
    if system.startswith("vosk-tts-"):
        _, _, version, speaker = system.split("-")
        import vosk_tts
        d = next(p for p in (dl / f"vosk-tts-{version}").iterdir() if p.is_dir())
        model = vosk_tts.Model(model_path=str(d))
        synth = vosk_tts.Synth(model)
        class Audio:  # the shape sherpa-onnx returns
            def __init__(self, samples, rate): self.samples, self.sample_rate = samples, rate
        def run(text):
            # its phonemizer knows "-" but no other dash or hyphen
            a = synth.synth_audio(re.sub("[\u2010-\u2015\u2212]", "-", text), speaker_id=int(speaker))
            a = np.asarray(a).reshape(-1)
            return Audio(a.astype(np.float32) / 32768 if a.dtype == np.int16 else a.astype(np.float32), 22050)
        return "ru", run, [p for p in d.rglob("*") if p.is_file()]
    raise SystemExit(f"unknown system {system}")


lang, run, files = build()
size = sum(Path(p).stat().st_size for p in files)
rows = [l.rstrip("\n").split("\t") for l in open(root / "sentences.tsv", encoding="utf-8")]
rows = [r for r in rows if r[1] == lang]
out = root / "out" / system; out.mkdir(parents=True, exist_ok=True)
run(rows[0][2])  # warm-up
spent, audio_s = 0.0, 0.0
for sid, _, text in rows:
    t0 = time.perf_counter(); a = run(text); spent += time.perf_counter() - t0
    samples = np.asarray(a.samples, dtype=np.float32)
    audio_s += len(samples) / a.sample_rate
    sf.write(out / f"{sid}.wav", samples, a.sample_rate, subtype="PCM_16")
info = {"system": system, "lang": lang, "n": len(rows), "audio_s": round(audio_s, 1), "synth_s": round(spent, 1), "rtf": round(spent / audio_s, 4), "size_mb": round(size / 1e6, 1)}
json.dump(info, open(out / "timing.json", "w"))
print(json.dumps(info))
