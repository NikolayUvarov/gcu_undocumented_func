#!/usr/bin/env python3
"""Generate independent full-graph gradients and a complete synthetic regression oracle."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import sys
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    archive = root / 'reference/qwen-spline-lab-v0.1.0.zip'
    if hashlib.sha256(archive.read_bytes()).hexdigest() != '5e27aab03013e16ac447f12e7093b39ca36167fb20b9b74368661ea425a38b25':
        parser.error('Reference archive hash mismatch')
    with zipfile.ZipFile(archive) as bundle:
        for name in ('__init__.py', 'representations.py'):
            path = 'src/qwen_spline_lab/' + name
            if (args.reference_root / path).read_bytes() != bundle.read('qwen-spline-lab/' + path):
                parser.error('Reference source mismatch: ' + path)
    sys.path.insert(0, str(args.reference_root / 'src'))
    import torch
    from qwen_spline_lab.representations import BlockSpec, build_block
    torch.set_num_threads(1)
    torch.manual_seed(42)
    source_files = {name: (root / 'tests/fixtures/model-v0' / name).read_bytes()
                    for name in ('definition.json', 'plain.json', 'input.json')}
    definition = json.loads(source_files['definition.json'])
    plain = json.loads(source_files['plain.json'])

    def linear(data):
        module = torch.nn.Linear(*data['shape'], bias=bool(data['bias'])).double()
        with torch.no_grad():
            module.weight.copy_(torch.tensor(data['weights'], dtype=torch.float64).reshape(module.weight.shape))
            if data['bias']:
                module.bias.copy_(torch.tensor(data['bias'], dtype=torch.float64))
        return module

    def gated(data):
        kind = data['representation']
        n, w, _ = data['shape']
        original = data['selection']['original_channels'] if data['selection'] else w
        module = build_block(BlockSpec(kind, n, max(w, original), width=w,
                                     rank=data['factors']['rank'] if data['factors'] else 0,
                                     knots=data['spline']['knots'] if data['spline'] else 4)).double()
        with torch.no_grad():
            for name, layer in (('gate', module.gate_proj), ('up', module.up_proj), ('down', module.down_proj)):
                projection = layer.left if kind == 'svd' else layer
                if kind == 'svd':
                    for side in ('left', 'right'):
                        weight = getattr(layer, side).weight
                        weight.copy_(torch.tensor(data['factors'][name + '_' + side], dtype=torch.float64).reshape(weight.shape))
                else:
                    layer.weight.copy_(torch.tensor(data[name], dtype=torch.float64).reshape(layer.weight.shape))
                if data['bias'] and data['bias'][name]:
                    projection.bias = torch.nn.Parameter(torch.tensor(data['bias'][name], dtype=torch.float64))
            if data['spline']:
                for field in ('low', 'high', 'values'):
                    target = getattr(module.activation, field)
                    target.copy_(torch.tensor(data['spline'][field], dtype=torch.float64).reshape(target.shape))
        return module

    def modules(objects):
        return torch.nn.ModuleDict({key: linear(obj['data']) if obj['kind'] == 'affine' else gated(obj['data'])
                                   for key, obj in objects.items()})

    def explicit_full(m, x):
        features = torch.nn.functional.silu(m['embed'](x)) + torch.relu(m['embed'](x))
        values = [m[f'block{i}'](features) for i in range(5)]
        total = values[0]
        for value in values[1:]:
            total = total + value
        return m['head'](total)

    def vector(t):
        return t.detach().flatten().tolist()

    def parameter_record(obj, module, gradient=False):
        data = obj['data']
        def values(t):
            return vector(t.grad if gradient else t)
        if obj['kind'] == 'affine':
            return {'kind': 'affine', 'data': {'weights': values(module.weight),
                                             'bias': values(module.bias) if data['bias'] else []}}
        record = {'gate': [], 'up': [], 'down': [], 'values': [], 'factors': None,
                  'bias': {name: [] for name in ('gate', 'up', 'down')} if data['bias'] else None}
        if data['factors']:
            record['factors'] = {'rank': data['factors']['rank']}
        for name, layer in (('gate', module.gate_proj), ('up', module.up_proj), ('down', module.down_proj)):
            projection = layer.left if data['factors'] else layer
            if data['factors']:
                for side in ('left', 'right'):
                    record['factors'][name + '_' + side] = values(getattr(layer, side).weight)
            else:
                record[name] = values(layer.weight)
            if data['bias'] and data['bias'][name]:
                record['bias'][name] = values(projection.bias)
        if data['spline']:
            record['values'] = values(module.activation.values)
        return {'kind': 'gated', 'data': record}

    m = modules(definition['objects'])
    x = torch.tensor(json.loads(source_files['input.json']), dtype=torch.float64).reshape(-1, 3).requires_grad_()
    dy = torch.tensor([[.2, -.1], [-.03, .07], [.15, .04]], dtype=torch.float64)
    y = explicit_full(m, x)
    (y * dy).sum().backward()
    gradients = {key: parameter_record(obj, m[key], True) for key, obj in definition['objects'].items()}
    rate = .001
    with torch.no_grad():
        for parameter in m.parameters():
            parameter -= rate * parameter.grad
    full = {'input': vector(x), 'dy': vector(dy), 'output': vector(y), 'input_gradient': vector(x.grad),
            'gradients': gradients, 'learning_rate': rate, 'after_output': vector(explicit_full(m, x))}

    def source(name):
        return {'identifier': 'synthetic://nrl-' + name, 'sha256': None,
                'licence': 'MIT; deterministic synthetic fixture, no pretrained weights',
                'notices': ['Generated by NRL fixture tools.', 'No model quality or optimization claim.']}

    samples = []
    for i in range(48):
        first = ((i * 17) % 53 - 26) / 13
        second = ((i * 29) % 59 - 29) / 14.5
        samples.append({'id': f'sample-{i:03}', 'group': f'group-{i:03}',
                        'split': 'train' if i < 32 else 'validation' if i < 40 else 'test',
                        'input': [first, second], 'target': [.7 * first - .4 * second + .3]})
    dataset = {'schema': 'nrl.regression-data/0', 'source': source('regression-data-v0'),
               'input_width': 2, 'output_width': 1, 'samples': samples}
    config = {'schema': 'nrl.regression-run/0', 'optimizer': 'full-batch-sgd/0',
              'selection': 'validation-mse-earliest/0', 'epochs': 128, 'learning_rate': .05,
              'max_training_values': 65536, 'max_forward_rows': 16384,
              'output_source': source('regression-training-v0')}
    m = modules(plain['objects'])
    def predict(x):
        return m['readout'](torch.relu(m['hidden'](x)))
    def batch(split):
        rows = [row for row in samples if row['split'] == split]
        return (torch.tensor([row['input'] for row in rows], dtype=torch.float64),
                torch.tensor([row['target'] for row in rows], dtype=torch.float64))
    train_x, train_y = batch('train')
    val_x, val_y = batch('validation')
    optimizer = torch.optim.SGD(m.parameters(), lr=config['learning_rate'])
    history = []
    selected_epoch = 0
    best_loss = float('inf')
    best = None
    for epoch in range(config['epochs'] + 1):
        if epoch:
            optimizer.zero_grad()
            loss = ((predict(train_x) - train_y) ** 2).mean()
            loss.backward()
            optimizer.step()
        with torch.no_grad():
            train_loss = ((predict(train_x) - train_y) ** 2).mean().item()
            val_loss = ((predict(val_x) - val_y) ** 2).mean().item()
        history.append({'epoch': epoch, 'train_mse': train_loss, 'validation_mse': val_loss})
        if val_loss < best_loss:
            best_loss = val_loss
            selected_epoch = epoch
            best = copy.deepcopy(m.state_dict())
    m.load_state_dict(best)
    test_x, _ = batch('test')
    expected = {'schema': 'nrl.training-oracle/0', 'torch': torch.__version__, 'seed': 42,
                'source_sha256': {name: hashlib.sha256(data).hexdigest() for name, data in source_files.items()},
                'full_graph': full,
                'regression': {'history': history, 'selected_epoch': selected_epoch,
                               'selected_test_output': vector(predict(test_x)),
                               'selected_parameters': {key: parameter_record(obj, m[key]) for key, obj in plain['objects'].items()}}}
    files = {'data.json': dataset, 'run.json': config, 'expected.json': expected}
    args.output.mkdir(parents=True, exist_ok=True)
    for name, value in files.items():
        data = (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + '\n').encode()
        path = args.output / name
        if args.check:
            if path.read_bytes() != data:
                parser.error('Fixture differs: ' + name)
        else:
            path.write_bytes(data)
    print(json.dumps({'fixture_sha256': hashlib.sha256((args.output / 'expected.json').read_bytes()).hexdigest(),
                      'selected_epoch': selected_epoch, 'validation_mse': best_loss}))


if __name__ == '__main__':
    main()
