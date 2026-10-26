#!/usr/bin/env python3
"""Original bounded mean-embedding family and independent PyTorch retrieval oracle."""
import argparse
import copy
import hashlib
import json
from pathlib import Path


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output-dir',type=Path,required=True)
    p.add_argument('--check',action='store_true')
    args=p.parse_args()
    import torch
    torch.set_num_threads(1)
    root=Path(__file__).resolve().parents[1]
    raw=(root/'tests/fixtures/model-v0/definition.json').read_bytes()
    original=json.loads(raw)
    def source(identifier):return dict(identifier='synthetic://'+identifier,sha256=None,licence='MIT OR Apache-2.0; original NRL fixture',notices=['Original synthetic numerical/retrieval fixture; no pretrained or representative embedding-quality claim.'])
    block=copy.deepcopy(original['objects']['block0']['data'])
    block['down']=[v/20 for v in block['down']]
    block['bias']['down']=[v/20 for v in block['bias']['down']]
    projection=dict(schema='nrl.model-definition/0',descriptor=dict(family='feed-forward',adapter='nrl.feed-forward',adapter_version=0,preprocessor='identity-f64/0',state='stateless',tasks=['regression-vector/0'],tied_objects=[],configuration=dict(schema='nrl.graph/0',input_width=3,nodes=[dict(id='body',op=dict(operator='gated',input='input',object='body')),dict(id='residual',op=dict(operator='add',left='input',right='body')),dict(id='readout',op=dict(operator='affine',input='residual',object='readout'))],output='readout')),execution=original['execution'],sources=[source('nrl-original-embedding-projection-v0')],objects=dict(body=dict(kind='gated',data=block),readout=dict(kind='affine',data=dict(schema='nrl.affine/0',shape=[3,3],weights=[1,.02,-.01,.01,1,.03,-.02,.01,1],bias=[.01,-.02,.005]))))
    vocab={'red':[.9,.02,.05],'fox':[1,.02,.03],'dog':[.95,.01,.04],'small':[.3,.3,.3],'cold':[.03,.95,.01],'rain':[.03,1,.02],'snow':[.01,.9,.08],'white':[.3,.3,.3],'star':[.01,.03,1],'moon':[.05,.01,.95],'sky':[.05,.48,.48],'bright':[.1,.1,.8]}
    tokens=list(vocab);table=dict(schema='nrl.embedding-table/0',tokens=tokens,dimension=3,values=[v for row in vocab.values()for v in row])
    definition=dict(schema='nrl.embedding-definition/0',table=table,projection=projection,max_input_tokens=32,sources=[source('nrl-original-token-embedding-v0')])
    docs=[dict(id=i,text=t)for i,t in [('animal-fox','red fox'),('animal-dog','small dog'),('weather-rain','cold rain'),('weather-snow','white snow'),('space-star','bright star'),('space-moon','moon sky')]]
    queries=[]
    for split,group,items in [('train','animals',[('fox','animal-fox','animal-dog'),('dog','animal-dog','animal-fox')]),('validation','weather',[('rain','weather-rain','weather-snow'),('snow','weather-snow','weather-rain')]),('test','space',[('star','space-star','space-moon'),('sky','space-moon','space-star')])]:
        for text,a,b in items:queries.append(dict(id='query-'+text,group=group,split=split,text=text,relevance={a:3,b:1}))
    data=dict(schema='nrl.retrieval-data/0',source=source('nrl-original-retrieval-data-v0'),top_k=2,documents=docs,queries=queries)
    tensor=lambda v:torch.tensor(v,dtype=torch.float64)
    def encode(text):
        ids=[tokens.index(t)for t in text.split()];x=tensor(table['values']).reshape(-1,3)[ids].mean(0)
        g=tensor(block['gate']).reshape(4,3);u=tensor(block['up']).reshape(4,3);down=tensor(block['down']).reshape(3,4);bias=block['bias']
        z=x+(torch.nn.functional.silu(x@g.T+tensor(bias['gate']))*(x@u.T+tensor(bias['up'])))@down.T+tensor(bias['down'])
        affine=projection['objects']['readout']['data'];v=z@tensor(affine['weights']).reshape(3,3).T+tensor(affine['bias']);v=v/torch.linalg.vector_norm(v)
        return v
    vectors={t:encode(t)for t in [d['text']for d in docs]+[q['text']for q in queries]}
    oracle_queries=[]
    for q in queries:
        ranking=sorted([(d['id'],torch.dot(vectors[d['text']],vectors[q['text']]).item())for d in docs],key=lambda item:(-item[1],item[0]))
        rel=q['relevance'];gain=lambda grade,rank:(2**grade-1)/torch.log2(tensor(rank+2)).item()
        recall=sum(i in rel for i,_ in ranking[:2])/len(rel);rr=1/(next(i for i,(key,_)in enumerate(ranking)if key in rel)+1)
        dcg=sum(gain(rel.get(key,0),i)for i,(key,_)in enumerate(ranking[:2]));idcg=sum(gain(g,i)for i,g in enumerate(sorted(rel.values(),reverse=True)[:2]))
        oracle_queries.append(dict(id=q['id'],split=q['split'],ranking=ranking,recall_at_k=recall,reciprocal_rank=rr,ndcg_at_k=dcg/idcg))
    expected=dict(schema='nrl.embedding-oracle/0',torch=torch.__version__,base_fixture_sha256=hashlib.sha256(raw).hexdigest(),vectors={text:v.tolist()for text,v in vectors.items()},queries=oracle_queries)
    files={'definition.json':definition,'data.json':data,'expected.json':expected,'replacement-source.json':source('nrl-embedding-projection-replacement-v0')}
    args.output_dir.mkdir(parents=True,exist_ok=True)
    for name,value in files.items():
        encoded=(json.dumps(value,indent=2,sort_keys=True)+'\n').encode();target=args.output_dir/name
        if args.check:assert target.read_bytes()==encoded,name
        else:target.write_bytes(encoded)
        print(name,hashlib.sha256(encoded).hexdigest())


if __name__=='__main__':main()
