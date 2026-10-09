#!/usr/bin/env python3
"""A toy transducer for the checks of 250: three tiny ONNX graphs with the dictation models' interface, converted by
convert.py into tests/dictate_toy.bin (a few KB, committed), so that CI runs `dictate` in the system without the
71 MB model. Its weights are random (seed 250); its text means nothing, but host and system must agree on it.

    python3 -I toy.py OUT.bin      (needs onnx and numpy)

encoder(x [1, T, 80], x_lens [1]): DynamicQuantizeLinear, MatMulInteger with an int8 weight (in panels after
conversion), its scales, Relu, a depthwise Conv over time, Tanh -> [1, T, 32]. decoder(y [1, 2]): an embedding,
ReduceMean -> [1, 32]. joiner(e [1, 32], d [1, 32]): Tanh of the sum, MatMul -> 16 logits; token 0 is the blank.
"""
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

D, V = 32, 16


def save(path, nodes, inputs, outputs, weights):
    graph = helper.make_graph(nodes, path.stem, inputs, outputs, [numpy_helper.from_array(w, n) for n, w in weights.items()])
    onnx.save(helper.make_model(graph, opset_imports=[helper.make_opsetid("", 13)]), path)


def main():
    out = Path(sys.argv[1])
    rng = np.random.default_rng(250)
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        f, i64 = TensorProto.FLOAT, TensorProto.INT64
        save(tmp / "encoder.onnx", [
            helper.make_node("DynamicQuantizeLinear", ["x"], ["q", "s", "z"]),
            helper.make_node("MatMulInteger", ["q", "w1", "z"], ["p"]),
            helper.make_node("Cast", ["p"], ["pf"], to=f),
            helper.make_node("Mul", ["pf", "s"], ["ps"]),
            helper.make_node("Mul", ["ps", "w1_scale"], ["h"]),
            helper.make_node("Add", ["h", "b1"], ["hb"]),
            helper.make_node("Relu", ["hb"], ["r"]),
            helper.make_node("Transpose", ["r"], ["rt"], perm=[0, 2, 1]),
            helper.make_node("Conv", ["rt", "dw", "db"], ["c"], group=D, kernel_shape=[3], pads=[1, 1]),
            helper.make_node("Transpose", ["c"], ["ct"], perm=[0, 2, 1]),
            helper.make_node("Tanh", ["ct"], ["encoder_out"]),
            helper.make_node("Identity", ["x_lens"], ["encoder_out_lens"]),
        ], [helper.make_tensor_value_info("x", f, [1, "T", 80]), helper.make_tensor_value_info("x_lens", i64, [1])],
            [helper.make_tensor_value_info("encoder_out", f, [1, "T", D]), helper.make_tensor_value_info("encoder_out_lens", i64, [1])],
            {"w1": rng.integers(-127, 128, (80, D)).astype(np.int8), "w1_scale": np.array(0.0005, np.float32),
             "b1": rng.normal(0, 0.5, D).astype(np.float32), "dw": rng.normal(0, 0.6, (D, 1, 3)).astype(np.float32),
             "db": rng.normal(0, 0.2, D).astype(np.float32)})
        save(tmp / "decoder.onnx", [
            helper.make_node("Gather", ["embedding", "y"], ["g"]),
            helper.make_node("ReduceMean", ["g"], ["decoder_out"], axes=[1], keepdims=0),
        ], [helper.make_tensor_value_info("y", i64, [1, 2])], [helper.make_tensor_value_info("decoder_out", f, [1, D])],
            {"embedding": rng.normal(0, 1, (V, D)).astype(np.float32)})
        w2 = rng.normal(0, 1.5, (D, V)).astype(np.float32)
        w2[:, 0] = 0  # the blank's logit is its bias: some frames give a symbol, others not
        bias = np.zeros(V, np.float32)
        bias[0] = 10.0
        save(tmp / "joiner.onnx", [
            helper.make_node("Add", ["encoder_out", "decoder_out"], ["sum"]),
            helper.make_node("Tanh", ["sum"], ["t"]),
            helper.make_node("MatMul", ["t", "w2"], ["m"]),
            helper.make_node("Add", ["m", "b2"], ["logit"]),
        ], [helper.make_tensor_value_info("encoder_out", f, [1, D]), helper.make_tensor_value_info("decoder_out", f, [1, D])],
            [helper.make_tensor_value_info("logit", f, [1, V])],
            {"w2": w2, "b2": bias})
        tokens = ["<blk>", "▁да", "▁нет", "▁мир", "▁дом", "ик", "а", "о", "▁кот", "▁лес", "ы", "▁и", "▁в", "ом", "е", "▁сад"]
        (tmp / "tokens.txt").write_text("".join(f"{t} {n}\n" for n, t in enumerate(tokens)), encoding="utf-8")
        convert = Path(__file__).with_name("convert.py")
        subprocess.run([sys.executable, "-I", str(convert), str(out), *(f"{g}={tmp / g}.onnx" for g in ("encoder", "decoder", "joiner")),
                        "--tokens", str(tmp / "tokens.txt")], check=True)


if __name__ == "__main__":
    main()
