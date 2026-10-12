#!/usr/bin/env python3
"""The reference for tests/fbank_host.rs: kaldi-native-fbank's features of a test signal, with sherpa-onnx's settings
for the dictation models (250). The signal is integers only, so the Rust test makes the same samples.

Usage: python3 scripts/voice_dictate/fbank_reference.py tests/fbank_reference.txt   (needs kaldi-native-fbank)
"""
import sys

SAMPLES = 5300  # not a multiple of the shift: the last frame reaches past the end


def signal():
    """A triangle wave of 37 samples, a square wave of 113 gated every 1000 samples, and LCG noise."""
    state, out = 12345, []
    for i in range(SAMPLES):
        state = (state * 1103515245 + 12345) % (1 << 31)
        noise = (state >> 16) % 2001 - 1000
        phase = i % 37
        triangle = (phase * 2 * 6000 // 37 - 6000) if phase < 37 // 2 else (6000 - (phase - 37 // 2) * 2 * 6000 // 37)
        square = (3000 if i % 113 < 56 else -3000) if (i // 1000) % 2 == 0 else 0
        out.append(max(-32768, min(32767, triangle + square + noise)))
    return out


def main():
    import kaldi_native_fbank as knf  # only here: the QEMU suite imports signal() without it
    opts = knf.FbankOptions()
    opts.frame_opts.samp_freq = 16000
    opts.frame_opts.dither = 0
    opts.frame_opts.snip_edges = False
    opts.mel_opts.num_bins = 80
    opts.mel_opts.low_freq = 20
    opts.mel_opts.high_freq = -400
    fbank = knf.OnlineFbank(opts)
    fbank.accept_waveform(16000, [s / 32768 for s in signal()])
    fbank.input_finished()
    with open(sys.argv[1], "w") as out:
        out.write(f"# kaldi-native-fbank {knf.__version__ if hasattr(knf, '__version__') else ''}: {SAMPLES} samples, "
                  f"{fbank.num_frames_ready} frames of 80 bands (scripts/voice_dictate/fbank_reference.py)\n")
        for frame in range(fbank.num_frames_ready):
            out.write(" ".join(f"{v:.5f}" for v in fbank.get_frame(frame)) + "\n")


if __name__ == "__main__":
    main()
