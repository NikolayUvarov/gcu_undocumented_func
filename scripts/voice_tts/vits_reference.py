#!/usr/bin/env python3
"""onnxruntime's audio for a VITS voice (Piper, Vosk TTS 0.7), to compare `mind::nn` with (252, the host only).

    python3 -I vits_reference.py MODEL.onnx "text" OUT_DIR [--dump]                 (a Piper voice)
    python3 -I vits_reference.py VOSK_MODEL_DIR "text" OUT_DIR --vosk SPEAKER [--dump]  (Vosk TTS 0.7)

For Piper the phonemes come from espeak-ng through piper-phonemize (GPL-3.0, used here on the host only), mapped to
ids with the voice's MODEL.onnx.json the way Piper does: "^", each phoneme followed by "_", "$". For Vosk TTS they come
from vosk-tts's own dictionary and rules (vosk_tts.Synth.g2p_noembed), and the speaker's number is an input too. The
noise scales are 0, so the audio does not depend on the random numbers. OUT_DIR gets ids.i64, scales.f32 (noise,
length, noise_w), audio.f32, phonemes.txt and, for Vosk TTS, sid.i64; with --dump also every value onnxruntime makes
(index.txt: name, dtype, shape, file), for finding where `mind::nn` departs from it. Needs onnx, onnxruntime, numpy,
and piper-phonemize or vosk-tts.
"""
import json
import os
import sys

import numpy as np


def main():
    import onnx
    import onnxruntime as ort
    path, text, out = sys.argv[1], sys.argv[2], sys.argv[3]
    rest = sys.argv[4:]
    dump = "--dump" in rest
    speaker = int(rest[rest.index("--vosk") + 1]) if "--vosk" in rest else None
    os.makedirs(out, exist_ok=True)
    if speaker is None:
        from piper_phonemize import phonemize_espeak
        config = json.load(open(path + ".json", encoding="utf-8"))
        table = config["phoneme_id_map"]
        phonemes = [p for sentence in phonemize_espeak(text, config["espeak"]["voice"]) for p in sentence]
        ids = list(table["^"])
        for p in phonemes:
            if p in table:
                ids += table[p] + table["_"]
        ids += table["$"]
        length_scale = config["inference"]["length_scale"]
    else:
        import vosk_tts
        voice = vosk_tts.Model(model_path=path)
        ids = vosk_tts.Synth(voice).g2p_noembed(text)
        phonemes = [str(i) for i in ids]
        length_scale = 1.0 / voice.config["inference"].get("speech_rate", 1.0)
        path = os.path.join(path, "model.onnx")
    model = onnx.load(path)
    names = []
    if dump:
        model = onnx.shape_inference.infer_shapes(model)
        known = {v.name: v for v in list(model.graph.value_info)}
        outputs = {o.name for o in model.graph.output}
        for n in model.graph.node:
            for o in n.output:
                if o in known and o not in outputs:
                    model.graph.output.append(known[o])
                    names.append(o)
    options = ort.SessionOptions()
    options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL
    session = ort.InferenceSession(model.SerializeToString(), options, providers=["CPUExecutionProvider"])
    x = np.array([ids], dtype=np.int64)
    scales = np.array([0.0, length_scale, 0.0], dtype=np.float32)
    inputs = {"input": x, "input_lengths": np.array([len(ids)], dtype=np.int64), "scales": scales}
    if speaker is not None:
        inputs["sid"] = np.array([speaker], dtype=np.int64)
        inputs["sid"].tofile(os.path.join(out, "sid.i64"))
    results = session.run(None, inputs)
    x.tofile(os.path.join(out, "ids.i64"))
    scales.tofile(os.path.join(out, "scales.f32"))
    np.asarray(results[0], dtype=np.float32).ravel().tofile(os.path.join(out, "audio.f32"))
    with open(os.path.join(out, "phonemes.txt"), "w", encoding="utf-8") as f:
        f.write((" " if speaker is not None else "").join(phonemes) + "\n")
    if dump:
        with open(os.path.join(out, "index.txt"), "w") as index:
            for k, (o, value) in enumerate(zip([o.name for o in model.graph.output], results)):
                value = np.asarray(value)
                value.tofile(os.path.join(out, f"v{k}.bin"))
                index.write(f"{o}\t{value.dtype}\t{','.join(map(str, value.shape))}\tv{k}.bin\n")
    print(f"{len(ids)} ids, {np.asarray(results[0]).size} samples")


if __name__ == "__main__":
    main()
