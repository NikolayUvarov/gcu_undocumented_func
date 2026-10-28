#!/usr/bin/env python3
"""Independent next-token metrics from the pinned PyTorch decoder logits."""
import argparse
import hashlib
import json
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--check', action='store_true')
    args = p.parse_args()
    import torch
    torch.set_num_threads(1)
    raw = (Path(__file__).resolve().parents[1] / 'tests/fixtures/decoder-v0.json').read_bytes()
    oracle = json.loads(raw)
    results = []
    for case in oracle['cases'][:3]:
        logits = torch.tensor(case['f64_logits'], dtype=torch.float64).reshape(-1, 512)[:-1]
        targets = torch.tensor(case['tokens'][1:], dtype=torch.long)
        loss = torch.nn.functional.cross_entropy(logits, targets, reduction='none')
        results.append(dict(text=case['text'], predicted_tokens=len(targets), mean_nll=loss.mean().item(), perplexity=loss.mean().exp().item(), top1_accuracy=(logits.argmax(1) == targets).double().mean().item()))
    data = dict(schema='nrl.evaluation-oracle/0', torch=torch.__version__, decoder_oracle_sha256=hashlib.sha256(raw).hexdigest(), cases=results)
    encoded = (json.dumps(data, indent=2, sort_keys=True) + '\n').encode()
    if args.check:
        assert args.output.read_bytes() == encoded
    else:
        args.output.write_bytes(encoded)
    print(hashlib.sha256(encoded).hexdigest())


if __name__ == '__main__':
    main()
