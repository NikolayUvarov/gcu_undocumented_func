#!/usr/bin/env python3
"""Independent float64 autograd and contrastive AdamW oracle for the original embedding fixture."""
import argparse
import json
from pathlib import Path
import torch

def main():
    p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);p.add_argument('--check',action='store_true');args=p.parse_args()
    torch.set_num_threads(1);root=Path(__file__).resolve().parents[1]
    definition=json.loads((root/'tests/fixtures/embedding-v0/definition.json').read_bytes());data=json.loads((root/'tests/fixtures/embedding-v0/data.json').read_bytes());cfg=json.loads((root/'tests/fixtures/embedding-training-v0/config.json').read_bytes())
    block=definition['projection']['objects']['body']['data'];affine=definition['projection']['objects']['readout']['data'];table=definition['table'];tokens=table['tokens']
    values={'table':table['values'],'p.body.0':block['gate'],'p.body.1':block['up'],'p.body.2':block['down'],'p.body.3':[],'p.body.4':block['bias']['gate'],'p.body.5':block['bias']['up'],'p.body.6':block['bias']['down'],'p.readout.0':affine['weights'],'p.readout.1':affine['bias']}
    params={k:torch.tensor(v,dtype=torch.float64,requires_grad=True)for k,v in values.items()}
    def encode(text):
        x=params['table'].reshape(-1,3)[[tokens.index(t)for t in text.split()]].mean(0)
        g=params['p.body.0'].reshape(4,3);u=params['p.body.1'].reshape(4,3);down=params['p.body.2'].reshape(3,4)
        z=x+(torch.nn.functional.silu(x@g.T+params['p.body.4'])*(x@u.T+params['p.body.5']))@down.T+params['p.body.6']
        v=z@params['p.readout.0'].reshape(3,3).T+params['p.readout.1'];return v/torch.linalg.vector_norm(v)
    def loss(split):
        docs=torch.stack([encode(d['text'])for d in data['documents']]);rows=[]
        for q in data['queries']:
            if q['split']!=split:continue
            scores=docs@encode(q['text'])/cfg['temperature'];target=torch.tensor([2**q['relevance'].get(d['id'],0)-1 for d in data['documents']],dtype=torch.float64);target/=target.sum();rows.append(-(torch.log_softmax(scores,dim=0)*target).sum())
        return torch.stack(rows).mean()
    def grads():return {k:(v.grad.tolist()if v.grad is not None else [0.0]*v.numel())for k,v in params.items()}
    text='fox fox red';dy=torch.tensor([.2,-.7,.4],dtype=torch.float64);vector=encode(text);(vector@dy).backward();backward=dict(text=text,dy=dy.tolist(),output=vector.detach().tolist(),gradients=grads())
    for v in params.values():v.grad=None
    objective=loss('train');objective.backward();contrastive=dict(loss=objective.item(),gradients=grads())
    for v in params.values():v.grad=None
    c=cfg['optimizer'];opt=torch.optim.AdamW(list(params.values()),lr=c['learning_rate'],betas=(c['beta1'],c['beta2']),eps=c['epsilon'],weight_decay=c['weight_decay'],foreach=False,fused=False)
    history=[]
    for step in range(2):
        opt.zero_grad(set_to_none=True);loss('train').backward();opt.step();history.append(dict(step=step+1,parameters={k:v.detach().tolist()for k,v in params.items()},train_loss=loss('train').item(),validation_loss=loss('validation').item()))
    raw=(json.dumps(dict(schema='nrl.embedding-training-oracle/0',reference='PyTorch 2.10.0 CPU float64 autograd/AdamW',backward=backward,contrastive=contrastive,history=history),indent=2,sort_keys=True)+'\n').encode()
    if args.check:
        if args.output.read_bytes()!=raw:raise SystemExit('embedding training oracle differs')
    else:args.output.write_bytes(raw)
if __name__=='__main__':main()
