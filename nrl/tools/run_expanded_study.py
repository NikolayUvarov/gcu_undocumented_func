#!/usr/bin/env python3
"""Run the frozen larger host study through the ordinary Rust CLI; no downloads or Python ML backend."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess
import time

def load(p):return json.loads(p.read_bytes())
def save(p,v):
    with p.open('x',encoding='utf-8')as f:json.dump(v,f,indent=2,sort_keys=True,ensure_ascii=False);f.write('\n')
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--executable',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
    root=Path(__file__).resolve().parents[1];repo=root.parent;exe=args.executable.resolve();out=args.output.resolve();fixture=root/'tests/fixtures/expanded-v0';source=load(fixture/'source.json')
    for name,digest in source['checksums'].items():
        if hashlib.sha256((fixture/name).read_bytes()).hexdigest()!=digest:raise SystemExit('Frozen expanded input checksum mismatch: '+name)
    corpus=load(fixture/'corpus.json');study=load(fixture/'study.json');adaptation=load(fixture/'adaptation.json')
    if Counter(d['split']for d in corpus['documents'])!={'train':24,'validation':12,'test':12}:raise SystemExit('Frozen split contract differs')
    out.mkdir();log=out/'commands.jsonl'
    def run(*argv):
        command=[str(exe)]+[str(x)for x in argv];start=time.monotonic();result=subprocess.run(command,cwd=repo,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        with log.open('a',encoding='utf-8')as f:f.write(json.dumps(dict(argv=command,exit_code=result.returncode,elapsed_seconds=time.monotonic()-start,stderr=result.stderr),ensure_ascii=False)+'\n')
        if result.returncode:raise SystemExit(f'Command failed: {argv[0]}: {result.stderr}')
        print('completed',argv[0],flush=True)
    base=out/'dense-model';ref=root/'reference/tinyllamas-260k';run('import-decoder',ref/'stories260K.bin',ref/'tok512.bin',ref/'decoder-source.json',base)
    search=out/'search';run('search-plans',base,fixture/'corpus.json',fixture/'study.json',search)
    adapted=out/'adaptation';run('compare-adaptation',base,search/(study['recovery']['start_plan']+'-model'),fixture/'corpus.json',fixture/'adaptation.json',adapted)
    measurement=out/'measurement.json';save(measurement,study['measurement'])
    baseline=load(search/'baseline-validation.json');search_record=load(search/'complete.json');validation={'dense':baseline};models={'dense':base};refusals={}
    for c in search_record['candidates']:
        if c['validation'] is not None:validation[c['plan_id']]=c['validation'];models[c['plan_id']]=search/(c['plan_id']+'-model')
        else:refusals[c['plan_id']]=c['refusal']
    for name in ['adamw','sgd']:
        model=adapted/(name+'-model');r=out/(name+'-validation-benchmark.json');run('benchmark-language',base,model,fixture/'corpus.json','validation',measurement,r);validation[name]=load(r);models[name]=model
    assessments=[];eligible=[];dense_bytes=baseline['model_information']['package_bytes'];dense_min=baseline['timing']['minimum_seconds']
    for name,r in validation.items():
        if name=='dense':continue
        delta=r['quality']['value']-baseline['quality']['value'];ratio=r['model_information']['package_bytes']/dense_bytes;speed=r['timing']['p95_seconds']<dense_min
        accepted=delta<=study['maximum_validation_nll_increase']and ratio<=study['maximum_package_ratio']and speed
        assessments.append(dict(candidate=name,validation_nll_increase=delta,actual_package_ratio=ratio,observed_nonoverlapping_timing_pass=speed,eligible=accepted))
        if accepted:eligible.append((r['timing']['median_seconds'],r['quality']['value'],name))
    chosen=min(eligible)[2]if eligible else 'dense'
    save(out/'selection.json',dict(schema='nrl.expanded-selection/0',selected=chosen,assessments=assessments,refusals=refusals,selection_split='validation',test_used_for_selection=False,rule=search_record['selection']['rule']))
    tests={}
    for name,model in models.items():
        path=out/(name+'-test.json');run('evaluate-language',base,model,fixture/'corpus.json','test',path);tests[name]=load(path)
    summary=dict(schema='nrl.expanded-study/0',source=source,study=study,adaptation=adaptation,source_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),executable_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),split_documents=dict(Counter(d['split']for d in corpus['documents'])),selection=load(out/'selection.json'),selected_model=str(models[chosen].relative_to(out)),validation_reports=validation,test_reports=tests,test_used_for_selection=False,native_execution=False,limits=['48 attributed prefixes in the TinyStories genre, not a broad benchmark.','Split independence is by exact source story and normalized prefix; semantic near-duplicates and model pretraining overlap remain unknown.','Training uses capped train trajectories; validation selects; all-candidate test reports are diagnostic only.','Actual packages retain original weights; fewer effective parameters do not mean disk/RSS savings.','Timings apply to the recorded scalar-f64 host/executable/workload; no native/GPU/parallel guarantee.'])
    save(out/'summary.json',summary);print(json.dumps(dict(selected=chosen,documents=48,candidates=len(validation)-1,summary=str(out/'summary.json'))))
if __name__=='__main__':main()
