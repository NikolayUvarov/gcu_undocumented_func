#!/usr/bin/env python3
"""Independent vectorized PyTorch f64 and pinned original C f32 decoder/tokenizer oracles."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--check',action='store_true')
    args=parser.parse_args()
    import torch
    torch.set_num_threads(1)
    torch.manual_seed(42)
    root=Path(__file__).resolve().parents[1]/'reference/tinyllamas-260k'
    source=json.loads((root/'source.json').read_text())
    for name,record in source['files'].items():
        assert hashlib.sha256((root/name).read_bytes()).hexdigest()==record['sha256'],name
    raw=(root/'stories260K.bin').read_bytes()
    dim,hidden,layers,heads,kvheads,vocab,context=struct.unpack_from('<7i',raw)
    assert (dim,hidden,layers,heads,kvheads,vocab,context)==(64,172,5,8,4,512,512)
    offset=28
    def read(*shape):
        nonlocal offset
        count=1
        for n in shape: count*=n
        tensor=torch.frombuffer(bytearray(raw[offset:offset+count*4]),dtype=torch.float32).clone().double().reshape(shape)
        offset+=count*4
        return tensor
    embed=read(vocab,dim)
    na=read(layers,dim);q=read(layers,dim,dim);k=read(layers,dim//heads*kvheads,dim);v=read(layers,dim//heads*kvheads,dim)
    o=read(layers,dim,dim);nf=read(layers,dim);gate=read(layers,hidden,dim);down=read(layers,dim,hidden);up=read(layers,hidden,dim)
    final=read(dim);cos=read(context,dim//heads//2);sin=read(context,dim//heads//2)
    assert offset==len(raw)
    def rms(x,w): return x*torch.rsqrt((x*x).mean(-1,keepdim=True)+1e-5)*w
    def rotary(x):
        pair=x.reshape(len(x),x.shape[1],-1,2)
        co=cos[:len(x),None,:];si=sin[:len(x),None,:]
        return torch.stack((pair[...,0]*co-pair[...,1]*si,pair[...,0]*si+pair[...,1]*co),dim=-1).flatten(-2)
    def forward(tokens):
        x=embed[torch.tensor(tokens)]
        traces=[]
        for layer in range(layers):
            y=rms(x,na[layer]);query=rotary((y@q[layer].T).reshape(len(x),heads,-1)).transpose(0,1)
            key=rotary((y@k[layer].T).reshape(len(x),kvheads,-1)).repeat_interleave(heads//kvheads,dim=1).transpose(0,1)
            value=(y@v[layer].T).reshape(len(x),kvheads,-1).repeat_interleave(heads//kvheads,dim=1).transpose(0,1)
            scores=(query@key.transpose(-1,-2))/(dim//heads)**.5
            scores=scores.masked_fill(torch.ones(len(x),len(x),dtype=torch.bool).triu(1),float('-inf'))
            attended=(torch.softmax(scores,-1)@value).transpose(0,1).reshape(len(x),dim)
            x=x+attended@o[layer].T
            incoming=rms(x,nf[layer]);out=(torch.nn.functional.silu(incoming@gate[layer].T)*(incoming@up[layer].T))@down[layer].T
            x=x+out
            traces.append({'input':incoming.flatten().tolist(),'output':out.flatten().tolist()})
        return (rms(x,final)@embed.T).flatten().tolist(),traces
    harness=r'''#include <stdint.h>
#define TESTING
#include "reference-run.c"
int main(int argc,char** argv){
    Transformer m; build_transformer(&m,argv[1]); Tokenizer t; build_tokenizer(&t,argv[2],m.config.vocab_size);
    int *ids=calloc(strlen(argv[3])+8,sizeof(int)),n=0; encode(&t,argv[3],1,0,ids,&n);
    printf("%d",n);for(int i=0;i<n;i++)printf(",%d",ids[i]);printf("\n");
    for(int i=0;i<n;i++){ float* out=forward(&m,ids[i],i);for(int j=0;j<m.config.vocab_size;j++)printf("%a%c",(double)out[j],j+1==m.config.vocab_size?'\n':','); }
    for(int i=1;i<n;i++){char* s=decode(&t,ids[i-1],ids[i]);for(size_t j=0;j<strlen(s);j++)printf("%02x",(unsigned char)s[j]);}printf("\n");
    free(ids);free_tokenizer(&t);free_transformer(&m);return 0;
}'''
    prompts=['Once upon a time','A red fox.','Hi 🦊']
    extra=['','a','  a  b\n','Café e\u0301','Привет 🦊','\t a\n']
    results=[]
    with tempfile.TemporaryDirectory(prefix='nrl-decoder-reference-') as temp:
        temp=Path(temp); (temp/'harness.c').write_text(harness)
        subprocess.run(['cc','-O2','-std=c11','-D_POSIX_C_SOURCE=200809L','-I',str(root),str(temp/'harness.c'),'-lm','-o',str(temp/'reference')],check=True)
        for text in prompts+extra:
            lines=subprocess.check_output([str(temp/'reference'),str(root/'stories260K.bin'),str(root/'tok512.bin'),text],text=True).splitlines()
            row=[int(x) for x in lines[0].split(',')];n=row[0];tokens=row[1:];assert len(tokens)==n
            f32=[float.fromhex(x) for line in lines[1:n+1] for x in line.split(',')]
            entry={'text':text,'tokens':tokens,'decoded_hex':lines[n+1]}
            if text in prompts:
                f64,traces=forward(tokens)
                entry.update({'f64_logits':f64,'original_c_f32_logits':f32,'layers':traces})
            results.append(entry)
    expected={'schema':'nrl.decoder-oracle/0','torch':torch.__version__,'source_revision':source['model_revision'],
              'reference_c_sha256':source['files']['reference-run.c']['sha256'],
              'policy':'cpu-scalar-f64-source-f32-stored-rope/0','cases':results}
    data=(json.dumps(expected,indent=2,sort_keys=True,allow_nan=False)+'\n').encode()
    args.output.parent.mkdir(parents=True,exist_ok=True)
    if args.check:
        if args.output.read_bytes()!=data: parser.error('Decoder oracle differs')
    else: args.output.write_bytes(data)
    print(json.dumps({'sha256':hashlib.sha256(data).hexdigest(),'cases':len(results),'numerical_cases':len(prompts)}))


if __name__=='__main__': main()
