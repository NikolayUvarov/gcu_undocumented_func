#!/usr/bin/env python3
"""Generate an independent PyTorch float64 AdamW continuation reference."""
import argparse
import json
from pathlib import Path
import torch

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--check',action='store_true')
    args=parser.parse_args()
    torch.set_num_threads(1)
    cfg=dict(schema='nrl.optimizer/0',algorithm='adamw',learning_rate=0.003,beta1=0.8,beta2=0.95,epsilon=1e-7,weight_decay=0.02,max_steps=4)
    initial={'body.0':[0.3,-0.8,0.0],'head.0':[-0.4,1.1]}
    params={k:torch.tensor(v,dtype=torch.float64,requires_grad=True) for k,v in initial.items()}
    opt=torch.optim.AdamW(list(params.values()),lr=cfg['learning_rate'],betas=(cfg['beta1'],cfg['beta2']),eps=cfg['epsilon'],weight_decay=cfg['weight_decay'],foreach=False,fused=False)
    rows=[]
    for step in range(4):
        gradients={k:[((-1)**(i+step))*(step+1)*(i+1)*0.07 for i in range(len(v))] for k,v in initial.items()}
        for k,p in params.items(): p.grad=torch.tensor(gradients[k],dtype=torch.float64)
        opt.step()
        rows.append(dict(gradients=gradients,parameters={k:p.detach().tolist() for k,p in params.items()},first={k:opt.state[p]['exp_avg'].tolist() for k,p in params.items()},second={k:opt.state[p]['exp_avg_sq'].tolist() for k,p in params.items()}))
    raw=(json.dumps(dict(schema='nrl.optimizer-oracle/0',reference='PyTorch 2.10.0 CPU float64 AdamW foreach=false fused=false',config=cfg,initial=initial,steps=rows),indent=2)+'\n').encode()
    if args.check:
        if args.output.read_bytes()!=raw: raise SystemExit('optimizer oracle differs')
    else: args.output.write_bytes(raw)
if __name__=='__main__': main()
