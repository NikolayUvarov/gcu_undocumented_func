#!/usr/bin/env python3
"""Save a tiny five-representation overlay through the verified, unchanged QSL writer."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    archive = Path(__file__).resolve().parents[1] / 'reference/qwen-spline-lab-v0.1.0.zip'
    archive_sha = hashlib.sha256(archive.read_bytes()).hexdigest()
    if archive_sha != '5e27aab03013e16ac447f12e7093b39ca36167fb20b9b74368661ea425a38b25':
        parser.error('Reference archive SHA-256 mismatch')
    with zipfile.ZipFile(archive) as bundle:
        for name in ('__init__.py', 'representations.py', 'artifacts.py', 'utils.py'):
            relative = 'src/qwen_spline_lab/' + name
            if (args.reference_root / relative).read_bytes() != bundle.read('qwen-spline-lab/' + relative):
                parser.error('Reference source mismatch: ' + relative)
    sys.path.insert(0, str(args.reference_root / 'src'))
    import torch
    from qwen_spline_lab.representations import BlockSpec, build_block, REPRESENTATIONS
    from qwen_spline_lab.artifacts import save_overlay, load_overlay
    torch.set_num_threads(1)
    torch.manual_seed(42)
    blocks = {}
    for index, kind in enumerate(REPRESENTATIONS):
        block = build_block(BlockSpec(kind, 3, 4, width=4 if kind == 'dense' else 2, rank=2, knots=4))
        with torch.no_grad():
            for number, parameter in enumerate(block.parameters()):
                parameter.copy_(torch.arange(parameter.numel(), dtype=torch.float32).reshape(parameter.shape) / (17 + number * 3) - .2)
        blocks[index] = block
    base = {'source': 'synthetic://nrl-qsl-import-fixture', 'fingerprint': 'synthetic-nrl-import-π',
            'model_type': 'synthetic-mlp', 'licence': 'MIT; deterministic synthetic fixture, no pretrained weights'}
    with tempfile.TemporaryDirectory(prefix='nrl-qsl-oracle-') as work:
        generated = Path(work) / 'overlay'
        manifest = save_overlay(generated, blocks, base, details={'fixture': True, 'archive_sha256': archive_sha})
        reloaded, _ = load_overlay(generated)
        x = torch.tensor([[.2, -.7, 1.1], [4.2, -3.1, 2.3], [-2.5, .4, -1.2]], dtype=torch.float64)
        expected = {'schema': 'nrl.qsl-oracle/0', 'archive_sha256': archive_sha,
                    'input': x.flatten().tolist(), 'dtype': 'saved-f32-parameters-exactly-widened-to-f64',
                    'outputs': {f'qsl.layer.{index}': block.double()(x).detach().flatten().tolist() for index, block in reloaded.items()},
                    'weights_sha256': manifest['weights_sha256'], 'artifact_id': manifest['artifact_id']}
        (generated / 'expected.json').write_text(json.dumps(expected, indent=2, sort_keys=True, allow_nan=False) + '\n')
        if args.check:
            for name in ('blocks.safetensors', 'expected.json'):
                if (args.output / name).read_bytes() != (generated / name).read_bytes():
                    parser.error('Regenerated fixture differs: ' + name)
            saved_manifest = json.loads((args.output / 'manifest.json').read_text())
            for value in (manifest, saved_manifest):
                value.pop('environment', None)
            if manifest != saved_manifest:
                parser.error('Regenerated core manifest differs')
        else:
            args.output.mkdir()
            for item in generated.iterdir():
                (args.output / item.name).write_bytes(item.read_bytes())
    print(json.dumps({'weights_sha256': expected['weights_sha256'], 'artifact_id': expected['artifact_id'],
                      'representations': len(blocks), 'check': args.check,
                      'environment_check': 'writer environment is recorded; excluded from cross-host regeneration comparison'}, sort_keys=True))


if __name__ == '__main__':
    main()
