#!/usr/bin/env python3
"""Generate small f64 differential tensors using the unchanged Python reference."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    archive = Path(__file__).resolve().parents[1] / "reference/qwen-spline-lab-v0.1.0.zip"
    if hashlib.sha256(archive.read_bytes()).hexdigest() != "5e27aab03013e16ac447f12e7093b39ca36167fb20b9b74368661ea425a38b25":
        parser.error("Reference archive SHA-256 mismatch")
    with zipfile.ZipFile(archive) as bundle:
        for name in ("__init__.py", "representations.py"):
            relative = "src/qwen_spline_lab/" + name
            if (args.reference_root / relative).read_bytes() != bundle.read("qwen-spline-lab/" + relative):
                parser.error("Reference source mismatch: " + relative)
    sys.path.insert(0, str(args.reference_root / "src"))
    import torch
    from qwen_spline_lab.representations import BlockSpec, ChannelSpline, CompactGatedMLP
    torch.set_num_threads(1)
    torch.manual_seed(42)
    cases = []
    for kind in ("dense", "spline-linear"):
        block = CompactGatedMLP(BlockSpec(kind, 3, 4, width=4, knots=4)).double()
        with torch.no_grad():
            for index, layer in enumerate((block.gate_proj, block.up_proj, block.down_proj)):
                layer.weight.copy_(torch.arange(12, dtype=torch.float64).reshape(layer.weight.shape)
                                   / (13 + index * 3) - 0.4)
            if kind == "spline-linear":
                block.activation.low.fill_(-1.0)
                block.activation.high.fill_(1.0)
                block.activation.values.copy_(torch.tensor([
                    [-0.3, -0.1, 0.4, 1.1], [-0.2, 0.0, 0.5, 1.2],
                    [-0.1, 0.1, 0.6, 1.3], [0.0, 0.2, 0.7, 1.4]], dtype=torch.float64))
        x = torch.tensor([[0.2, -0.7, 1.1], [4.2, -3.1, 2.3], [-2.5, 0.4, -1.2]],
                         dtype=torch.float64, requires_grad=True)
        target = torch.tensor([[0.1, 0.2, -0.1], [0.2, -0.3, 0.4], [-0.1, 0.3, 0.2]], dtype=torch.float64)
        output = block(x)
        loss = (output - target).square().mean()
        loss.backward()
        artifact = {"schema": "nrl.block/0", "representation": kind, "shape": [3, 4, 3],
                    "gate": block.gate_proj.weight.detach().flatten().tolist(),
                    "up": block.up_proj.weight.detach().flatten().tolist(),
                    "down": block.down_proj.weight.detach().flatten().tolist(), "spline": None}
        grads = {name: layer.weight.grad.flatten().tolist() for name, layer in
                 (("gate", block.gate_proj), ("up", block.up_proj), ("down", block.down_proj))}
        grads["values"] = []
        if kind == "spline-linear":
            activation = block.activation
            artifact["spline"] = {"knots": 4, "low": activation.low.tolist(), "high": activation.high.tolist(),
                                  "values": activation.values.detach().flatten().tolist()}
            grads["values"] = activation.values.grad.flatten().tolist()
        initial_loss = loss.item()
        with torch.no_grad():
            for parameter in block.parameters():
                parameter.add_(parameter.grad, alpha=-0.05)
            trained_loss = (block(x) - target).square().mean().item()
        cases.append({"artifact": artifact, "input": x.detach().flatten().tolist(),
                      "target": target.flatten().tolist(), "output": output.detach().flatten().tolist(),
                      "input_gradient": x.grad.flatten().tolist(), "gradients": grads,
                      "loss": initial_loss, "learning_rate": 0.05, "trained_loss": trained_loss})
    spline = ChannelSpline(2, 4, "linear").double()
    with torch.no_grad():
        spline.low.copy_(torch.tensor([-1.0, -2.0]))
        spline.high.copy_(torch.tensor([2.0, 1.0]))
        spline.values.copy_(torch.tensor([[0.0, 1.0, 3.0, 6.0], [1.0, -1.0, 2.0, 4.0]]))
    x = torch.tensor([[-2.0, -3.0], [-1.0, -2.0], [0.0, -1.0], [1.0, 0.0],
                      [2.0, 1.0], [3.0, 2.0], [0.4, -0.3]], dtype=torch.float64, requires_grad=True)
    out = spline(x)
    out.sum().backward()
    result = {"schema": "nrl.fixture/0", "archive_sha256":
              "5e27aab03013e16ac447f12e7093b39ca36167fb20b9b74368661ea425a38b25",
              "torch": torch.__version__, "seed": 42, "dtype": "f64", "cases": cases,
              "spline": {"channels": 2, "knots": 4, "low": spline.low.tolist(), "high": spline.high.tolist(),
                         "values": spline.values.detach().flatten().tolist(), "input": x.detach().flatten().tolist(),
                         "output": out.detach().flatten().tolist(), "input_gradient": x.grad.flatten().tolist(),
                         "values_gradient": spline.values.grad.flatten().tolist()}}
    payload = (json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + "\n").encode()
    if args.check:
        if args.output.read_bytes() != payload:
            parser.error("Fixtures differ from regenerated reference data")
    else:
        with args.output.open("xb") as output:
            output.write(payload)
    print(hashlib.sha256(payload).hexdigest())


if __name__ == "__main__":
    main()
