# graph.py <onnx>...: parameters (weights counted once, int8 and float alike) and the operator set of each graph.
import sys, collections
import onnx
from onnx import numpy_helper
total_ops = collections.Counter()
params = 0
for path in sys.argv[1:]:
    m = onnx.load(path, load_external_data=False)
    seen = set()
    for t in m.graph.initializer:
        n = 1
        for d in t.dims: n *= d
        if t.data_type in (onnx.TensorProto.INT64, onnx.TensorProto.INT32) and n < 64: continue
        if t.name in seen: continue
        seen.add(t.name)
        if n >= 16: params += n
    def walk(g):
        for node in g.node:
            total_ops[node.op_type] += 1
            for a in node.attribute:
                if a.g: walk(a.g)
                for sg in a.graphs: walk(sg)
    walk(m.graph)
print(f"params~{params/1e6:.1f}M ops={len(total_ops)}: " + " ".join(sorted(total_ops)))
