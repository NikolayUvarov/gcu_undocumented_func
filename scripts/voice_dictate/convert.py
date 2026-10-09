#!/usr/bin/env python3
"""ONNX models to MIND Core's network file (250, `mind::nn`): the graphs as the system runs them, without protobuf.

    python3 convert.py OUT.bin NAME=model.onnx... [--tokens tokens.txt]

e.g. `convert.py dictate-ru.bin encoder=encoder.int8.onnx decoder=decoder.int8.onnx joiner=joiner.int8.onnx
--tokens tokens.txt`. Needs the `onnx` package. Constant nodes become initializers, and an `If` keeps its two branches
as graphs of their own. One table holds every tensor; each top graph has its own names, which its branches share, so a
branch reads what its graph made.

The file, little endian, every section 64-byte aligned:
    magic "MINDNN01", u32 version (2), u32 graphs, u32 tensors, u32 ops (the op names' count)
    op names: per op u8 length, the name
    tensors: per tensor u8 dtype (0 none: a value made at run time; 1 f32, 2 u8, 3 i8, 4 i32, 5 i64, 6 bool; 7 i8 in
        panels, below), u8 rank, u16 name length, u32 0,
        u64 data offset (from the start of the file), u64 data bytes, rank * u64 dims, the name
    graphs: per graph u16 name length, the name, u32 inputs, u32 outputs, u32 nodes, ids of inputs, ids of outputs,
        then per node: u16 op, u8 inputs, u8 outputs, u16 attributes, u16 0, input ids (0xFFFFFFFF: absent),
        output ids, u32 tensors freed after it and their ids, then the attributes: u8 name length, the name,
        u8 kind (1 int, 2 float, 3 ints, 4 floats, 5 graph index), u8 0, u32 count, then count i64 / f64 / u32
    tokens: u32 count, then per token u16 length and its UTF-8 (count 0 without --tokens)
    data: the initializers' bytes
    u32 FNV-1a of everything before it

Version 2 adds code 7: a weight [k, n] that only MatMulInteger reads as its B, with k even and n a multiple of 16, is
written in panels of 16 columns, one after another; a panel holds for each pair of rows p the pairs (B[2p][j],
B[2p+1][j]) of its 16 columns j in order (libmind/src/nn/gemm.rs reads it so).
"""
import struct
import sys

DTYPES = {1: (1, "f32", 4), 2: (2, "u8", 1), 3: (3, "i8", 1), 6: (4, "i32", 4), 7: (5, "i64", 8), 9: (6, "bool", 1)}
KINDS = {"INT": 1, "FLOAT": 2, "INTS": 3, "FLOATS": 4, "GRAPH": 5}


def fnv1a(data):
    h = 0x811C9DC5
    for b in data:
        h = ((h ^ b) * 0x01000193) & 0xFFFFFFFF
    return h


class Model:
    def __init__(self):
        self.tensors = {}   # (top graph, name) -> id
        self.scope = ""
        self.info = []      # id -> [dtype, dims, raw bytes or None, name]
        self.graphs = []    # (name, inputs, outputs, nodes)
        self.ops = []

    def tensor(self, name):
        # Names are the top graph's own: the encoder's and the decoder's "/Constant_output_0" are two tensors.
        key = (self.scope, name)
        if key not in self.tensors:
            self.tensors[key] = len(self.info)
            self.info.append([0, [], None, name])
        return self.tensors[key]

    def initializer(self, proto, name=None):
        import onnx.numpy_helper as nh
        array = nh.to_array(proto)
        if proto.data_type not in DTYPES:
            raise SystemExit(f"{name or proto.name}: data type {proto.data_type} not supported")
        code, _, _ = DTYPES[proto.data_type]
        t = self.tensor(name or proto.name)
        self.info[t][:3] = [code, list(array.shape), array.astype(array.dtype.newbyteorder("<")).tobytes()]

    def op(self, name):
        if name not in self.ops:
            self.ops.append(name)
        return self.ops.index(name)

    def graph(self, name, g, prefix=""):
        import onnx
        from onnx import helper
        for init in g.initializer:
            self.initializer(init)
        inputs = [self.tensor(i.name) for i in g.input if i.name not in {x.name for x in g.initializer}]
        outputs = [self.tensor(o.name) for o in g.output]
        index = len(self.graphs)
        self.graphs.append(None)
        nodes = []
        for n in g.node:
            if n.op_type == "Constant":
                (attr,) = n.attribute
                if attr.name == "value":
                    self.initializer(attr.t, n.output[0])
                else:
                    value = helper.get_attribute_value(attr)
                    import numpy as np
                    dtype = {"value_float": np.float32, "value_floats": np.float32, "value_int": np.int64, "value_ints": np.int64}[attr.name]
                    self.initializer(onnx.numpy_helper.from_array(np.array(value, dtype=dtype)), n.output[0])
                continue
            attrs = []
            for a in n.attribute:
                kind = onnx.AttributeProto.AttributeType.Name(a.type)
                if kind == "GRAPH":
                    attrs.append((a.name, 5, [self.graph(f"{name}/{n.name}/{a.name}", a.g)]))
                elif kind == "TENSOR":
                    import onnx.numpy_helper as nh
                    array = nh.to_array(a.t).ravel()
                    attrs.append((a.name, 4 if array.dtype.kind == "f" else 3, [x.item() for x in array]))  # ConstantOfShape's value
                elif kind in KINDS:
                    value = helper.get_attribute_value(a)
                    attrs.append((a.name, KINDS[kind], list(value) if isinstance(value, (list, tuple)) else [value]))
                else:
                    raise SystemExit(f"{n.op_type} {n.name}: attribute {a.name} of type {kind}")
            ins = [self.tensor(i) if i else 0xFFFFFFFF for i in n.input]
            outs = [self.tensor(o) for o in n.output]
            nodes.append([self.op(n.op_type), ins, outs, attrs])
        self.graphs[index] = (name, inputs, outputs, nodes)
        return index

    def free_lists(self):
        # A value a graph's nodes made (or a top graph's input) goes after the last node that reads it, there or in a
        # branch of one of its Ifs; graph outputs stay. A branch never frees what its outer graph made.
        keep, branches = set(), set()
        for name, inputs, outputs, nodes in self.graphs:
            keep.update(outputs)
            for node in nodes:
                for attr in node[3]:
                    if attr[1] == 5:
                        branches.update(attr[2])
        for index, (name, inputs, outputs, nodes) in enumerate(self.graphs):
            owned = {t for node in nodes for t in node[2]} | (set() if index in branches else set(inputs))
            last = {}
            for i, (op, ins, outs, attrs) in enumerate(nodes):
                for t in ins:
                    if t != 0xFFFFFFFF:
                        last[t] = i
                for attr in attrs:
                    if attr[1] == 5:
                        for g in attr[2]:
                            for t in self.reads(g):
                                last[t] = i
            for i, node in enumerate(nodes):
                node.append(sorted(t for t, at in last.items() if at == i and t in owned and t not in keep and self.info[t][0] == 0))

    def reads(self, index):
        # Everything a graph and its branches read.
        name, inputs, outputs, nodes = self.graphs[index]
        out = set(outputs)
        for op, ins, outs, attrs, *_ in nodes:
            out.update(t for t in ins if t != 0xFFFFFFFF)
            for attr in attrs:
                if attr[1] == 5:
                    for g in attr[2]:
                        out.update(self.reads(g))
        return out

    def panels(self):
        # MatMulInteger's i8 weights that nothing else reads, in panels (code 7).
        import numpy as np
        uses = {}
        for name, inputs, outputs, nodes in self.graphs:
            for t in outputs:
                uses.setdefault(t, set()).add(("output", 0))
            for op, ins, outs, attrs, *_ in nodes:
                for i, t in enumerate(ins):
                    if t != 0xFFFFFFFF:
                        uses.setdefault(t, set()).add((self.ops[op], i))
        for t, used in uses.items():
            code, dims, raw, name = self.info[t]
            if used == {("MatMulInteger", 1)} and code == 3 and len(dims) == 2 and dims[0] % 2 == 0 and dims[1] % 16 == 0:
                k, n = dims
                b = np.frombuffer(raw, dtype=np.int8).reshape(k // 2, 2, n // 16, 16)
                self.info[t][0] = 7
                self.info[t][2] = np.ascontiguousarray(b.transpose(2, 0, 3, 1)).tobytes()

    def write(self, path, tokens):
        self.free_lists()
        self.panels()
        head = bytearray()
        head += b"MINDNN01" + struct.pack("<IIII", 2, len(self.graphs), len(self.info), len(self.ops))
        for op in self.ops:
            head += struct.pack("<B", len(op)) + op.encode()
        pad = lambda b: b.extend(b"\0" * (-len(b) % 64))
        pad(head)
        # The data offsets are known once the header's size is: lay it out twice.
        for _ in range(2):
            body = bytearray(head)
            data_at = getattr(self, "data_at", 0)
            data = bytearray()
            for code, dims, raw, name in self.info:
                offset = 0
                if raw is not None:
                    pad(data)
                    offset = data_at + len(data)
                    data += raw
                encoded = name.encode()
                body += struct.pack("<BBHI QQ", code, len(dims), len(encoded), 0, offset, len(raw or b""))
                body += struct.pack(f"<{len(dims)}Q", *dims) + encoded
            pad(body)
            for name, inputs, outputs, nodes in self.graphs:
                encoded = name.encode()
                body += struct.pack("<H", len(encoded)) + encoded + struct.pack("<III", len(inputs), len(outputs), len(nodes))
                body += struct.pack(f"<{len(inputs)}I", *inputs) + struct.pack(f"<{len(outputs)}I", *outputs)
                for op, ins, outs, attrs, freed in nodes:
                    body += struct.pack("<HBBHH", op, len(ins), len(outs), len(attrs), 0)
                    body += struct.pack(f"<{len(ins)}I", *ins) + struct.pack(f"<{len(outs)}I", *outs)
                    body += struct.pack("<I", len(freed)) + struct.pack(f"<{len(freed)}I", *freed)
                    for aname, kind, values in attrs:
                        body += struct.pack("<B", len(aname)) + aname.encode() + struct.pack("<BBI", kind, 0, len(values))
                        fmt = {1: "q", 2: "d", 3: "q", 4: "d", 5: "I"}[kind]
                        body += struct.pack(f"<{len(values)}{fmt}", *values)
            pad(body)
            body += struct.pack("<I", len(tokens))
            for token in tokens:
                encoded = token.encode()
                body += struct.pack("<H", len(encoded)) + encoded
            pad(body)
            self.data_at = len(body)
        out = body + data
        out += struct.pack("<I", fnv1a(out))
        with open(path, "wb") as f:
            f.write(out)
        return len(out)


def main():
    import onnx
    args = sys.argv[1:]
    if len(args) < 2:
        raise SystemExit(__doc__)
    out, tokens = args[0], []
    model = Model()
    rest = args[1:]
    if "--tokens" in rest:
        at = rest.index("--tokens")
        with open(rest[at + 1], encoding="utf-8") as f:
            tokens = [line.rsplit(" ", 1)[0] for line in f.read().splitlines() if line.strip()]
        rest = rest[:at] + rest[at + 2:]
    for item in rest:
        name, path = item.split("=", 1)
        model.scope = name
        model.graph(name, onnx.load(path).graph)
    size = model.write(out, tokens)
    print(f"{out}: {len(model.graphs)} graphs, {len(model.info)} tensors, ops {' '.join(model.ops)}; {size} bytes")


if __name__ == "__main__":
    main()
