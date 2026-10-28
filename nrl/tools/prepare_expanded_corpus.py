#!/usr/bin/env python3
"""Reproduce a frozen, attributed CDLA-Sharing-1.0 TinyStories prefix evaluation corpus."""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request
REVISION='f54c09fd23315a6f9c86f9dc80f725de7d8f9c64'
RAW_SHA256='94e431816c4cce81ff71e4408ff8d3bda9a42e8d2663986697c3954288cb38b4'
URL=f'https://huggingface.co/datasets/roneneldan/TinyStories/resolve/{REVISION}/TinyStories-valid.txt'

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--source',type=Path,required=True);p.add_argument('--download',action='store_true');p.add_argument('--output',type=Path,required=True);p.add_argument('--check',action='store_true');args=p.parse_args()
    if args.download:
        if args.source.exists():raise SystemExit('Download destination already exists')
        with urllib.request.urlopen(URL,timeout=60)as r:raw=r.read((32<<20)+1)
        if len(raw)>(32<<20)or hashlib.sha256(raw).hexdigest()!=RAW_SHA256:raise SystemExit('Pinned source checksum/size mismatch')
        args.source.write_bytes(raw)
    raw=args.source.read_bytes()
    if len(raw)!=19447282 or hashlib.sha256(raw).hexdigest()!=RAW_SHA256:raise SystemExit('Pinned source checksum/size mismatch')
    candidates=[]
    for index,story in enumerate(raw.decode('utf-8').split('<|endoftext|>')):
        story=story.strip()
        if not story:continue
        digest=hashlib.sha256(story.encode()).hexdigest()
        prefix=story.encode()[:384].decode('utf-8','ignore')
        if len(prefix.encode())<192:continue
        candidates.append((digest,index,prefix))
    candidates.sort();selected=[];seen=set()
    for digest,index,prefix in candidates:
        normalized=' '.join(prefix.casefold().split())
        if normalized in seen:continue
        seen.add(normalized);selected.append((digest,index,prefix))
        if len(selected)==48:break
    if len(selected)!=48:raise SystemExit('Insufficient independent source stories')
    notices=['TinyStories by Ronen Eldan and Yuanzhi Li; source dataset roneneldan/TinyStories.',URL,'MODIFIED DATA: selected 48 source validation stories by ascending full-story SHA-256; retained up to first 384 UTF-8 bytes on character boundaries; removed duplicate normalized prefixes; assigned 24/12/12 train/validation/test in hash order.','Data and this selection/arrangement are CDLA-Sharing-1.0; https://cdla.dev/sharing-1-0/ . Full agreement accompanies corpus.json.','Story IDs preserve source identity; related-story and pretraining overlap beyond exact-prefix checks are unknown. No unseen-pretraining or broad language-model-quality claim.']
    corpus=dict(schema='nrl.language-data/0',source=dict(identifier=f'https://huggingface.co/datasets/roneneldan/TinyStories/tree/{REVISION}',sha256=RAW_SHA256,licence='CDLA-Sharing-1.0 (TinyStories data and modified selection)',notices=notices),max_tokens_per_document=512,documents=[dict(id=f'tinystories-valid-{index}',group='story-'+digest[:16],split='train'if i<24 else 'validation'if i<36 else 'test',text=prefix)for i,(digest,index,prefix)in enumerate(selected)])
    data=(json.dumps(corpus,indent=2,sort_keys=True,ensure_ascii=False)+'\n').encode()
    if args.check:
        if args.output.read_bytes()!=data:raise SystemExit('Expanded corpus differs')
    else:
        if args.output.exists():raise SystemExit('Corpus output already exists')
        args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_bytes(data)
    print(json.dumps(dict(schema='nrl.corpus-preparation/0',revision=REVISION,raw_source_sha256=RAW_SHA256,corpus_file_sha256=hashlib.sha256(data).hexdigest(),corpus_id=hashlib.sha256(json.dumps(corpus,separators=(',',':'),sort_keys=True,ensure_ascii=False).encode()).hexdigest(),documents=48,prefix_bytes=384)))
if __name__=='__main__':main()
