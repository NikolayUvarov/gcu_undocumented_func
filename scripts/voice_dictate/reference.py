#!/usr/bin/env python3
"""The reference for tests/nn_host.rs and the dictation host tests (250): kaldi-native-fbank's features of a WAV,
onnxruntime's encoder output on them, and the transducer's greedy search over onnxruntime's decoder and joiner.

    python3 reference.py MODEL_DIR clip.wav OUT_DIR   (MODEL_DIR: encoder.int8.onnx, decoder.int8.onnx,
                                                       joiner.int8.onnx, tokens.txt; needs onnxruntime,
                                                       kaldi-native-fbank, numpy, soundfile)
OUT_DIR gets features.f32 and encoder_out.f32 (little endian), tokens.txt (the ids found) and text.txt.
"""
import os
import sys

import numpy as np


def features(wav):
    import kaldi_native_fbank as knf
    import soundfile
    samples, rate = soundfile.read(wav, dtype="float32")
    if samples.ndim > 1:
        samples = samples.mean(axis=1)
    assert rate == 16000, rate
    opts = knf.FbankOptions()
    opts.frame_opts.samp_freq = 16000
    opts.frame_opts.dither = 0
    opts.frame_opts.snip_edges = False
    opts.mel_opts.num_bins = 80
    opts.mel_opts.low_freq = 20
    opts.mel_opts.high_freq = -400
    fbank = knf.OnlineFbank(opts)
    fbank.accept_waveform(16000, samples.tolist())
    fbank.input_finished()
    return np.stack([fbank.get_frame(i) for i in range(fbank.num_frames_ready)]).astype(np.float32)


def main():
    import onnxruntime as ort
    model, wav, out = sys.argv[1:4]
    os.makedirs(out, exist_ok=True)
    options = ort.SessionOptions()
    options.intra_op_num_threads = 1
    session = lambda name: ort.InferenceSession(os.path.join(model, name), options, providers=["CPUExecutionProvider"])
    encoder, decoder, joiner = session("encoder.int8.onnx"), session("decoder.int8.onnx"), session("joiner.int8.onnx")
    x = features(wav)
    x.tofile(os.path.join(out, "features.f32"))
    enc, enc_lens = encoder.run(None, {"x": x[None], "x_lens": np.array([len(x)], dtype=np.int64)})
    enc[0, :enc_lens[0]].astype(np.float32).tofile(os.path.join(out, "encoder_out.f32"))
    tokens = [line.rsplit(" ", 1)[0] for line in open(os.path.join(model, "tokens.txt"), encoding="utf-8").read().splitlines()]
    blank, context = 0, [0, 0]
    dec = decoder.run(None, {"y": np.array([context], dtype=np.int64)})[0]
    found = []
    for t in range(enc_lens[0]):
        logit = joiner.run(None, {"encoder_out": enc[:, t], "decoder_out": dec})[0]
        y = int(logit.argmax())
        if y != blank:
            found.append(y)
            context = context[1:] + [y]
            dec = decoder.run(None, {"y": np.array([context], dtype=np.int64)})[0]
    text = "".join(tokens[y] for y in found).replace("▁", " ").strip()
    open(os.path.join(out, "tokens.txt"), "w").write(" ".join(map(str, found)) + "\n")
    open(os.path.join(out, "text.txt"), "w", encoding="utf-8").write(text + "\n")
    print(f"{len(x)} frames, encoder out {enc.shape} ({enc_lens[0]}), {len(found)} tokens: {text}")


if __name__ == "__main__":
    main()
