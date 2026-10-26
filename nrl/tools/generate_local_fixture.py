#!/usr/bin/env python3
"""Real-model local-pruning/SGD oracle from independent decoder traces and PyTorch."""
import argparse
import hashlib
import json
from pathlib import Path
import struct


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--check',action='store_true')
    args=parser.parse_args()
    import torch
    torch.set_num_threads(1)
    root=Path(__file__).resolve().parents[1]
    raw=(root/'reference/tinyllamas-260k/stories260K.bin').read_bytes()
    assert hashlib.sha256(raw).hexdigest()=='b0a507e7ad0f626624f17112325e66691f9076d622e1d3274d103d00299f2696'
    oracle_bytes=(root/'tests/fixtures/decoder-v0.json').read_bytes()
    oracle=json.loads(oracle_bytes)
    n,w,l,h,kv,vocab,context=struct.unpack_from('<7i',raw)
    offset=28+4*(vocab*n+l*n+l*n*n+2*l*(n//h*kv)*n+l*n*n+l*n)
    def matrix(start,rows,cols):
        return torch.frombuffer(bytearray(raw[start:start+rows*cols*4]),dtype=torch.float32).clone().double().reshape(rows,cols)
    gate=matrix(offset,w,n);down=matrix(offset+l*w*n*4,n,w);up=matrix(offset+2*l*w*n*4,w,n)
    train=oracle['cases'][0];val=oracle['cases'][1]
    def batch(case):
        count=len(case['tokens'])-1
        return (torch.tensor(case['layers'][0]['input'][:count*n],dtype=torch.float64).reshape(-1,n),
                torch.tensor(case['layers'][0]['output'][:count*n],dtype=torch.float64).reshape(-1,n))
    x,y=batch(train);vx,vy=batch(val)
    features=torch.nn.functional.silu(x@gate.T)*(x@up.T)
    scores=(features*features).mean(0)*(down*down).sum(0)
    indices=sorted(sorted(range(w),key=lambda c:(-scores[c].item(),c))[:8])
    g=torch.nn.Parameter(gate[indices].clone());u=torch.nn.Parameter(up[indices].clone());d=torch.nn.Parameter(down[:,indices].clone())
    optimizer=torch.optim.SGD([g,u,d],lr=.002)
    def forward(x):return (torch.nn.functional.silu(x@g.T)*(x@u.T))@d.T
    history=[];best_loss=float('inf');best_step=0;best=None
    for step in range(5):
        if step:
            optimizer.zero_grad();((forward(x)-y)**2).mean().backward();optimizer.step()
        with torch.no_grad():tr=((forward(x)-y)**2).mean().item();validation=((forward(vx)-vy)**2).mean().item()
        history.append({'step':step,'train_mse':tr,'validation_mse':validation})
        if validation<best_loss:best_loss=validation;best_step=step;best={name:t.detach().flatten().tolist()for name,t in [('gate',g),('up',u),('down',d)]}
    expected={'schema':'nrl.local-oracle/0','torch':torch.__version__,'decoder_oracle_sha256':hashlib.sha256(oracle_bytes).hexdigest(),'source_weight_sha256':hashlib.sha256(raw).hexdigest(),'retained_indices':indices,'history':history,'selected_step':best_step,'selected':best}
    data=(json.dumps(expected,sort_keys=True,indent=2,allow_nan=False)+'\n').encode()
    args.output.parent.mkdir(parents=True,exist_ok=True)
    if args.check:
        if args.output.read_bytes()!=data:parser.error('Local oracle differs')
    else:args.output.write_bytes(data)
    print(json.dumps({'sha256':hashlib.sha256(data).hexdigest(),'selected_step':best_step,'validation_mse':best_loss}))


if __name__=='__main__':main()
