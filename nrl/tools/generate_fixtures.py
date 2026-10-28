#!/usr/bin/env python3
"""Generate f64 block oracles from verified QSL and an explicit affine test extension."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import zipfile

ARCHIVE_SHA256 = "5e27aab03013e16ac447f12e7093b39ca36167fb20b9b74368661ea425a38b25"
DESCRIPTOR = {"kind": "gated-feed-forward/0", "source_activation": "silu", "dtype": "f64",
              "layout": "row-major", "state": "stateless", "parameter_sharing": "independent"}
KINDS = ("dense", "spline-linear", "pruned", "spline-cubic", "svd")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    archive = Path(__file__).resolve().parents[1] / "reference/qwen-spline-lab-v0.1.0.zip"
    if hashlib.sha256(archive.read_bytes()).hexdigest() != ARCHIVE_SHA256:
        parser.error("Reference archive SHA-256 mismatch")
    with zipfile.ZipFile(archive) as bundle:
        for name in ("__init__.py", "representations.py"):
            relative = "src/qwen_spline_lab/" + name
            if (args.reference_root / relative).read_bytes() != bundle.read("qwen-spline-lab/" + relative):
                parser.error("Reference source mismatch: " + relative)
    sys.path.insert(0, str(args.reference_root / "src"))
    import torch
    from qwen_spline_lab.representations import BlockSpec, ChannelSpline, CompactGatedMLP, SVDGatedMLP, LowRankLinear, select_channels
    torch.set_num_threads(1)
    torch.manual_seed(42)

    def flat(tensor):
        return tensor.detach().flatten().tolist()

    def layers(block):
        return (("gate", block.gate_proj), ("up", block.up_proj), ("down", block.down_proj))

    def artifact(block, kind, selection=None):
        if kind == "svd":
            shape = [block.gate_proj.right.weight.shape[1], block.gate_proj.left.weight.shape[0], block.down_proj.left.weight.shape[0]]
        else:
            shape = [block.gate_proj.weight.shape[1], block.gate_proj.weight.shape[0], block.down_proj.weight.shape[0]]
        result = {"schema": "nrl.block/5", "descriptor": DESCRIPTOR, "representation": kind,
                  "shape": shape,
                  "gate": [], "up": [], "down": [], "spline": None, "factors": None,
                  "selection": selection, "bias": None}
        for name, layer in layers(block):
            if kind == "svd":
                if result["factors"] is None:
                    result["factors"] = {"rank": 2}
                for side in ("left", "right"):
                    result["factors"][name + "_" + side] = flat(getattr(layer, side).weight)
                projection = layer.left
            else:
                result[name] = flat(layer.weight)
                projection = layer
            if projection.bias is not None:
                if result["bias"] is None:
                    result["bias"] = {"gate": [], "up": [], "down": []}
                result["bias"][name] = flat(projection.bias)
        if kind.startswith("spline-"):
            a = block.activation
            result["spline"] = {"knots": a.knots, "low": flat(a.low), "high": flat(a.high), "values": flat(a.values)}
        return result

    def capture(block, kind, x, target, selection, rate, oracle):
        output = block(x)
        loss = (output - target).square().mean()
        loss.backward()
        stored = artifact(block, kind, selection)
        grads = {"gate": [], "up": [], "down": [], "values": [], "factors": None, "bias": None}
        for name, layer in layers(block):
            if kind == "svd":
                if grads["factors"] is None:
                    grads["factors"] = {"rank": 2}
                for side in ("left", "right"):
                    grads["factors"][name + "_" + side] = flat(getattr(layer, side).weight.grad)
                projection = layer.left
            else:
                grads[name] = flat(layer.weight.grad)
                projection = layer
            if projection.bias is not None:
                if grads["bias"] is None:
                    grads["bias"] = {"gate": [], "up": [], "down": []}
                grads["bias"][name] = flat(projection.bias.grad)
        if kind.startswith("spline-"):
            grads["values"] = flat(block.activation.values.grad)
        with torch.no_grad():
            for parameter in block.parameters():
                parameter.add_(parameter.grad, alpha=-rate)
            trained_loss = (block(x) - target).square().mean().item()
        return {"artifact": stored, "input": flat(x), "target": flat(target), "output": flat(output),
                      "input_gradient": flat(x.grad), "gradients": grads, "loss": loss.item(),
                      "learning_rate": rate, "trained_loss": trained_loss,
                      "oracle": oracle}


    sources = []
    selection_fixtures = []
    calibration = torch.tensor([[1., 0, 0], [-1., 0, 0], [2., 0, 0]], dtype=torch.float64)
    for biased in (False, True):
        source = CompactGatedMLP(BlockSpec("dense", 3, 4, width=4)).double()
        with torch.no_grad():
            source.gate_proj.weight.copy_(torch.tensor([[1, 0, 0], [0, 2, 0], [0, 0, 3], [1, 1, 1]]))
            source.up_proj.weight.copy_(source.gate_proj.weight)
            source.down_proj.weight.copy_(torch.tensor([[1, 1, 1, 1], [1, -1, 1, -1], [.5, -.5, .5, -.5]]))
            if biased:
                for (_, layer), bias in zip(layers(source), ([.2, -.3, .1, .4], [-.1, .5, .2, -.2], [.7, -.4, .2])):
                    layer.bias = torch.nn.Parameter(torch.tensor(bias, dtype=torch.float64))
        energy = source.down_proj.weight.detach().square().sum(0)
        g = source.gate_proj.weight.detach().square().sum(1)
        u = source.up_proj.weight.detach().square().sum(1)
        if biased:
            g = g + source.gate_proj.bias.detach().square()
            u = u + source.up_proj.bias.detach().square()
        weight_scores = g * u * energy
        calibrated_scores = source.features(calibration).detach().square().mean(0) * energy
        if biased:
            weight_indices = weight_scores.argsort(descending=True, stable=True)[:2].sort().values.tolist()
            indices = calibrated_scores.argsort(descending=True, stable=True)[:2].sort().values.tolist()
        else:
            weight_indices = select_channels(source, 2, None).tolist()
            indices = select_channels(source, 2, calibration).tolist()
        selection_fixtures.append({"source": artifact(source, "dense"), "input": flat(calibration),
                                   "weight_indices": weight_indices, "calibration_indices": indices,
                                   "weight_scores_f64": flat(weight_scores), "calibration_scores_f64": flat(calibrated_scores),
                                   "reference_ranking_dtype": "f64-affine-extension" if biased else "f32",
                                   "rust_ranking_dtype": "f64"})
        sources.append(source)

    cases = []
    for biased in (False, True):
        for kind in KINDS:
            block = (SVDGatedMLP(BlockSpec(kind, 3, 4, rank=2)) if kind == "svd" else
                     CompactGatedMLP(BlockSpec(kind, 3, 4, width=2 if kind == "pruned" else 4, knots=4))).double()
            indices = selection_fixtures[int(biased)]["calibration_indices"]
            with torch.no_grad():
                for index, (name, layer) in enumerate(layers(block)):
                    if kind == "svd":
                        for side_index, factor in enumerate((layer.left, layer.right)):
                            factor.weight.copy_(torch.arange(factor.weight.numel(), dtype=torch.float64).reshape(factor.weight.shape)
                                                / (11 + index * 3 + side_index * 2) - 0.3)
                        projection = layer.left
                    else:
                        projection = layer
                        if kind == "pruned":
                            original = dict(layers(sources[int(biased)]))[name].weight
                            layer.weight.copy_(original[:, indices] if index == 2 else original[indices])
                        else:
                            layer.weight.copy_(torch.arange(12, dtype=torch.float64).reshape(layer.weight.shape) / (13 + index * 3) - 0.4)
                    if biased:
                        if kind == "pruned":
                            original = dict(layers(sources[1]))[name].bias
                            bias = original if index == 2 else original[indices]
                        else:
                            bias = torch.arange(projection.weight.shape[0], dtype=torch.float64) / (11 + index) - 0.1
                        projection.bias = torch.nn.Parameter(bias.detach().clone())
                if kind.startswith("spline-"):
                    block.activation.low.fill_(-1.0)
                    block.activation.high.fill_(1.0)
                    block.activation.values.copy_(torch.tensor([
                        [-0.3, -0.1, 0.4, 1.1], [-0.2, 0.0, 0.5, 1.2],
                        [-0.1, 0.1, 0.6, 1.3], [0.0, 0.2, 0.7, 1.4]], dtype=torch.float64))
            x = torch.tensor([[0.2, -0.7, 1.1], [4.2, -3.1, 2.3], [-2.5, 0.4, -1.2]], dtype=torch.float64, requires_grad=True)
            target = torch.tensor([[0.1, 0.2, -0.1], [0.2, -0.3, 0.4], [-0.1, 0.3, 0.2]], dtype=torch.float64)
            selection = {"mapping": "known", "original_channels": 4, "indices": indices, "method": "activation-energy-f64/1"} if kind == "pruned" else None
            cases.append(capture(block, kind, x, target, selection, 1e-6 if kind == "pruned" else 0.05,
                                 "pytorch-affine-extension/0" if biased else "qsl-unmodified/0"))

    class Rectangular(torch.nn.Module):
        def __init__(self, factored):
            super().__init__()
            self.gate_proj = LowRankLinear(2, 3, 2) if factored else torch.nn.Linear(2, 3, bias=False)
            self.up_proj = LowRankLinear(2, 3, 2) if factored else torch.nn.Linear(2, 3, bias=False)
            self.down_proj = LowRankLinear(3, 4, 2) if factored else torch.nn.Linear(3, 4, bias=False)

        def forward(self, x):
            return self.down_proj(torch.nn.functional.silu(self.gate_proj(x)) * self.up_proj(x))

    for factored in (False, True):
        block = Rectangular(factored).double()
        with torch.no_grad():
            for index, (_, layer) in enumerate(layers(block)):
                matrices = (layer.left, layer.right) if factored else (layer,)
                for j, matrix in enumerate(matrices):
                    matrix.weight.copy_(torch.arange(matrix.weight.numel(), dtype=torch.float64).reshape(matrix.weight.shape) / (19 + index * 3 + j) - .2)
                projection = layer.left if factored else layer
                projection.bias = torch.nn.Parameter(torch.arange(projection.weight.shape[0], dtype=torch.float64) / 13 - .1)
        x = torch.tensor([[.2, -.3], [.7, .4]], dtype=torch.float64, requires_grad=True)
        target = torch.tensor([[.1, .2, -.1, .3], [-.2, .4, .3, -.1]], dtype=torch.float64)
        cases.append(capture(block, "svd" if factored else "dense", x, target, None, .05, "pytorch-rectangular-affine/0"))

    standalone = {}
    for kind in ("linear", "cubic"):
        spline = ChannelSpline(2, 4, kind).double()
        with torch.no_grad():
            spline.low.copy_(torch.tensor([-1.0, -2.0]))
            spline.high.copy_(torch.tensor([2.0, 1.0]))
            spline.values.copy_(torch.tensor([[0.0, 1.0, 3.0, 6.0], [1.0, -1.0, 2.0, 4.0]]))
        x = torch.tensor([[-2.0, -3.0], [-1.0, -2.0], [0.0, -1.0], [1.0, 0.0], [2.0, 1.0], [3.0, 2.0], [0.4, -0.3]], dtype=torch.float64, requires_grad=True)
        output = spline(x)
        output.sum().backward()
        standalone["spline" if kind == "linear" else "cubic"] = {
            "channels": 2, "knots": 4, "low": flat(spline.low), "high": flat(spline.high), "values": flat(spline.values),
            "input": flat(x), "output": flat(output), "input_gradient": flat(x.grad), "values_gradient": flat(spline.values.grad)}
    svd_cases = []
    matrices = [torch.tensor([[3., 1., 0.], [0., 2., 1.], [1., 0., 1.]], dtype=torch.float64),
                torch.tensor([[1., 2., 3.], [4., -1., 2.], [2., 3., 0.], [-1., 1., 4.]], dtype=torch.float64),
                torch.tensor([[1., 2., 3., 4.], [0., 2., -1., 1.], [2., 0., 1., -2.]], dtype=torch.float64),
                torch.tensor([[1., 2., 3.], [2., 4., 6.], [0., 0., 0.]], dtype=torch.float64),
                torch.zeros(2, 3, dtype=torch.float64), torch.eye(3, dtype=torch.float64) * 2,
                torch.tensor([[1., 2.], [3., 4.], [-1., 0.]], dtype=torch.float64) * 1e150,
                torch.tensor([[1., 2.], [3., 4.], [-1., 0.]], dtype=torch.float64) * 1e-150,
                torch.randn(6, 5, dtype=torch.float64)]
    for matrix in matrices:
        singular = torch.linalg.svdvals(matrix)
        svd_cases.append({"inputs": matrix.shape[1], "outputs": matrix.shape[0], "weights": flat(matrix), "singular_values": flat(singular)})
    result = {"schema": "nrl.fixture/5", "archive_sha256": ARCHIVE_SHA256, "torch": torch.__version__,
              "seed": 42, "dtype": "f64", "cases": cases, "svd_initialization": svd_cases,
              "channel_selection": selection_fixtures[0], "affine_channel_selection": selection_fixtures[1],
              "affine_oracle": "Test-only output biases added to nn.Linear module instances; archive sources unchanged. Original QSL conversion rejects biases; this fixture independently checks the new Rust extension.", **standalone}
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
