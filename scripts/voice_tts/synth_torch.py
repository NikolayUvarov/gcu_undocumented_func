# synth_torch.py <work> <system> <lang>: synthesize the language's sentences with a PyTorch model on 4 threads; out/<system>-<lang>/.
import json, os, sys, time
from pathlib import Path
import numpy as np
import soundfile as sf
import torch

root, system, lang = Path(sys.argv[1]), sys.argv[2], sys.argv[3]


def _load(path, *args, **kwargs):
    """torchaudio.load through soundfile: this torchaudio reads through a torchcodec built for another torch."""
    data, rate = sf.read(str(path), dtype="float32", always_2d=True)
    return torch.from_numpy(data.T.copy()), rate


import torchaudio
torchaudio.load = _load
torch.set_num_threads(4)
dl = root / "dl"
ref = json.load(open(root / "refs.json"))[lang]  # a FLEURS recording (CC BY 4.0) and its text, for models that clone a voice
ref["wav"] = str(Path(os.environ.get("ASR", root)) / ref["wav"])  # relative to the recognition work directory
LANG_NAME = {"ru": "Russian", "en": "English"}


def build():
    if system == "pocket":
        from pocket_tts import TTSModel
        model = TTSModel.load_model()
        state = model.get_state_for_audio_prompt("alba")
        return lambda text: (model.generate_audio(state, text).numpy(), model.sample_rate)
    if system == "espeech":
        from f5_tts.infer.utils_infer import infer_process, load_model, load_vocoder, preprocess_ref_audio_text
        from f5_tts.model import DiT
        from ruaccent import RUAccent
        d = dl / "espeech"
        model = load_model(DiT, dict(dim=1024, depth=22, heads=16, ff_mult=2, text_dim=512, conv_layers=4), str(d / "espeech_tts_rlv2.pt"), vocab_file=str(d / "vocab.txt"), device="cpu")
        vocoder = load_vocoder(device="cpu")
        accent = RUAccent(); accent.load(omograph_model_size="turbo3.1", use_dictionary=True, tiny_mode=False)
        ref_audio, ref_text = preprocess_ref_audio_text(ref["wav"], accent.process_all(ref["text"]))
        def run(text):
            wav, sr, _ = infer_process(ref_audio, ref_text, accent.process_all(text), model, vocoder, device="cpu", nfe_step=32)
            return wav, sr
        return run
    if system == "qwen3":
        from qwen_tts import Qwen3TTSModel
        model = Qwen3TTSModel.from_pretrained(str(dl / "qwen3-base"), device_map="cpu", dtype=torch.float32)
        def run(text):
            wavs, sr = model.generate_voice_clone(text=text, language=LANG_NAME[lang], ref_audio=ref["wav"], ref_text=ref["text"])
            return np.asarray(wavs[0]), sr
        return run
    if system == "chatterbox":
        from chatterbox.mtl_tts import ChatterboxMultilingualTTS
        model = ChatterboxMultilingualTTS.from_local(str(dl / "chatterbox"), device="cpu")
        def run(text):
            wav = model.generate(text, language_id=lang)
            return wav.squeeze().numpy(), model.sr
        return run
    raise SystemExit(f"unknown system {system}")


run = build()
rows = [l.rstrip("\n").split("\t") for l in open(root / "sentences.tsv", encoding="utf-8")]
rows = [r for r in rows if r[1] == lang]
fleurs = int(os.environ.get("FLEURS", "20"))  # the slowest models read fewer FLEURS sentences
rows = [r for r in rows if "-L" in r[0] or int(r[0].split("-F")[1]) <= fleurs]
name = f"{system}-{lang}" + ("-timing" if os.environ.get("TIMING") else "")  # TIMING=1: a timing run apart from the scored one
out = root / "out" / name; out.mkdir(parents=True, exist_ok=True)
if os.environ.get("WARMUP", "1") != "0":  # WARMUP=0 for a model too slow to read a sentence twice
    run(rows[0][2])
spent, audio_s = 0.0, 0.0
for sid, _, text in rows:
    t0 = time.perf_counter(); samples, rate = run(text); spent += time.perf_counter() - t0
    samples = np.asarray(samples, dtype=np.float32).reshape(-1)
    audio_s += len(samples) / rate
    sf.write(out / f"{sid}.wav", samples, rate, subtype="PCM_16")
    print(sid, round(len(samples) / rate, 1), flush=True)
info = {"system": name, "lang": lang, "n": len(rows), "audio_s": round(audio_s, 1), "synth_s": round(spent, 1), "rtf": round(spent / audio_s, 4), "threads": 4}
json.dump(info, open(out / "timing.json", "w"))
print(json.dumps(info))
