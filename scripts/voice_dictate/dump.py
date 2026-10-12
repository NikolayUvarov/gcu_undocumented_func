#!/usr/bin/env python3
"""Every value onnxruntime makes in a graph, for finding where an interpreter departs from it (250, debugging only).

    python3 dump.py model.onnx features.f32 FRAMES OUT_DIR
OUT_DIR gets one raw little-endian file per value and index.txt: name, dtype, shape, file.
"""
import os
import sys

import numpy as np


def main():
    import onnx
    import onnxruntime as ort
    path, feats, frames, out = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
    os.makedirs(out, exist_ok=True)
    model = onnx.shape_inference.infer_shapes(onnx.load(path))
    known = {v.name: v for v in list(model.graph.value_info) + list(model.graph.output)}
    names = []
    for n in model.graph.node:
        if n.op_type == "Constant":
            continue
        for o in n.output:
            if o in known and o not in [x.name for x in model.graph.output]:
                model.graph.output.append(known[o])
                names.append(o)
    options = ort.SessionOptions()
    options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL
    session = ort.InferenceSession(model.SerializeToString(), options, providers=["CPUExecutionProvider"])
    x = np.fromfile(feats, dtype=np.float32).reshape(-1, 80)[:frames]
    results = session.run(None, {"x": x[None], "x_lens": np.array([len(x)], dtype=np.int64)})
    with open(os.path.join(out, "index.txt"), "w") as index:
        for k, (o, value) in enumerate(zip([o.name for o in model.graph.output], results)):
            value = np.asarray(value)
            name = f"v{k}.bin"
            value.tofile(os.path.join(out, name))
            index.write(f"{o}\t{value.dtype}\t{','.join(map(str, value.shape))}\t{name}\n")
    print(len(results), "values")


if __name__ == "__main__":
    main()
