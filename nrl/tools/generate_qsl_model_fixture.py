#!/usr/bin/env python3
"""Unchanged QSL writer plus independent full decoder math on pinned real weights."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys
import tempfile
import zipfile

def main():
    p=argparse.ArgumentParser();p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--check',action='store_true');args=p.parse_args()
    root=Path(__file__).resolve().parents[1];archive=root/'reference/qwen-spline-lab-v0.1.0.zip'
    assert hashlib.sha256(archive.read_bytes()).hexdigest()=='5e27aab03013e16ac447f12e7093b39ca36167fb20b9b74368661ea425a38b25'
    with zipfile.ZipFile(archive)as z:
        for name in ('__init__.py','representations.py','artifacts.py','utils.py'):
            relative='src/qwen_spline_lab/'+name;assert(args.reference_root/relative).read_bytes()==z.read('qwen-spline-lab/'+relative)
    sys.path.insert(0,str(args.reference_root/'src'))
    import torch
    from qwen_spline_lab.representations import BlockSpec,initialize_block
    from qwen_spline_lab.artifacts import save_overlay,load_overlay
    torch.set_num_threads(1);torch.manual_seed(42)
    source=root/'reference/tinyllamas-260k';raw=(source/'stories260K.bin').read_bytes();assert hashlib.sha256(raw).hexdigest()=='b0a507e7ad0f626624f17112325e66691f9076d622e1d3274d103d00299f2696'
    dim,hidden,layers,heads,kvheads,vocab,context=struct.unpack_from('<7i',raw);offset=28
    def read(*shape):
        nonlocal offset
        count=1
        for n in shape:count*=n
        t=torch.frombuffer(bytearray(raw[offset:offset+count*4]),dtype=torch.float32).clone().double().reshape(shape);offset+=count*4;return t
    embed=read(vocab,dim);na=read(layers,dim);q=read(layers,dim,dim);k=read(layers,dim//heads*kvheads,dim);v=read(layers,dim//heads*kvheads,dim);o=read(layers,dim,dim);nf=read(layers,dim);gate=read(layers,hidden,dim);down=read(layers,dim,hidden);up=read(layers,hidden,dim);final=read(dim);cos=read(context,dim//heads//2);sin=read(context,dim//heads//2);assert offset==len(raw)
    blocks={}
    for layer,kind in [(0,'pruned'),(1,'spline-linear'),(2,'spline-cubic'),(4,'svd')]:
        teacher=torch.nn.Module();teacher.gate_proj=torch.nn.Linear(dim,hidden,bias=False);teacher.up_proj=torch.nn.Linear(dim,hidden,bias=False);teacher.down_proj=torch.nn.Linear(hidden,dim,bias=False)
        with torch.no_grad():
            teacher.gate_proj.weight.copy_(gate[layer]);teacher.up_proj.weight.copy_(up[layer]);teacher.down_proj.weight.copy_(down[layer])
        blocks[layer]=initialize_block(teacher,BlockSpec(kind,dim,hidden,width=16,rank=4,knots=8),svd_method='exact')
    base_id='fde147c5be67eb480c825cc7c01cafc6998d95a0c494072554f61b5b6c3fd1fa'
    base=dict(source='nrl://pinned-tinyllamas-260k',fingerprint=base_id,fingerprint_policy='nrl-decoder-model-id/0',model_type='llama2c-f32-v0/0',licence='MIT; publisher-declared source weights')
    with tempfile.TemporaryDirectory(prefix='nrl-qsl-model-')as temp:
        generated=Path(temp)/'overlay';manifest=save_overlay(generated,blocks,base,details={'fixture':'four transformed FFNs from actual pinned 260K weights; numerical bridge only','reference_archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest()});loaded,_=load_overlay(generated);loaded={i:b.double()for i,b in loaded.items()}
        def rms(x,w):return x*torch.rsqrt((x*x).mean(-1,keepdim=True)+1e-5)*w
        def rotary(x):
            pair=x.reshape(len(x),x.shape[1],-1,2);co=cos[:len(x),None,:];si=sin[:len(x),None,:];return torch.stack((pair[...,0]*co-pair[...,1]*si,pair[...,0]*si+pair[...,1]*co),dim=-1).flatten(-2)
        def forward(tokens):
            x=embed[torch.tensor(tokens)]
            for layer in range(layers):
                y=rms(x,na[layer]);query=rotary((y@q[layer].T).reshape(len(x),heads,-1)).transpose(0,1);key=rotary((y@k[layer].T).reshape(len(x),kvheads,-1)).repeat_interleave(heads//kvheads,dim=1).transpose(0,1);value=(y@v[layer].T).reshape(len(x),kvheads,-1).repeat_interleave(heads//kvheads,dim=1).transpose(0,1)
                scores=(query@key.transpose(-1,-2))/(dim//heads)**.5;scores=scores.masked_fill(torch.ones(len(x),len(x),dtype=torch.bool).triu(1),float('-inf'));attended=(torch.softmax(scores,-1)@value).transpose(0,1).reshape(len(x),dim);x=x+attended@o[layer].T;incoming=rms(x,nf[layer]);out=loaded[layer](incoming)if layer in loaded else(torch.nn.functional.silu(incoming@gate[layer].T)*(incoming@up[layer].T))@down[layer].T;x=x+out
            return rms(x,final)@embed.T
        cases=[]
        for tokens in [[1,403,407,269,261,280],[1,259,260]]:
            logits=forward(tokens).detach().flatten().tolist();sequence=tokens.copy();generated_ids=[]
            for _ in range(4):
                best=forward(sequence)[-1].argmax().item();generated_ids.append(best)
                if best in (1,2):break
                sequence.append(best)
            cases.append(dict(tokens=tokens,logits=logits,generated=generated_ids))
        binding=dict(schema='nrl.qsl-decoder-binding/0',adapter='llama2c-f32-v0/0',base_model_id=base_id,qsl_artifact_id=manifest['artifact_id'],layers={str(i):i for i in blocks},source=dict(identifier='nrl://original-qsl-complete-model-fixture',sha256=None,licence='MIT; derived publisher-declared MIT TinyStories weights and archived QSL writer',notices=['Numerical full-model bridge fixture; no accepted compression or model-quality claim.']))
        extra={'binding.json':binding,'expected.json':dict(schema='nrl.qsl-model-oracle/0',reference='Unchanged QSL save/load plus independent vectorized PyTorch 2.10.0 float64 decoder',base_model_id=base_id,cases=cases)}
        for name,value in extra.items():(generated/name).write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')
        if args.check:
            for name in ['blocks.safetensors','binding.json','expected.json']:assert(args.output/name).read_bytes()==(generated/name).read_bytes(),name
            old=json.loads((args.output/'manifest.json').read_bytes());old.pop('environment');manifest.pop('environment');assert old==manifest
        else:
            args.output.mkdir()
            for f in generated.iterdir():(args.output/f.name).write_bytes(f.read_bytes())
    print('Verified full decoder QSL fixture:',args.output)
if __name__=='__main__':main()
