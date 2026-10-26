#!/usr/bin/env python3
"""Thin local layout and CLI preparation; no product changes or catalog service."""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
import measure
from corpus import save, sha, require, GIB

ROOT=Path(__file__).resolve().parent
REPO=ROOT.parents[1]
OLD=ROOT.parent/"agent-decision-evaluation"
sys.path.insert(0,str(OLD))
from harness import citation, check_citation, search_state, validate_answer
sys.path.pop(0)

def filehash(path):
    h=hashlib.sha256()
    with Path(path).open("rb") as f:
        for b in iter(lambda:f.read(1048576),b""):h.update(b)
    return h.hexdigest()

def verify_binary(config):
    require(filehash(Path(config['binary']))==config['binary_sha256'],'declared binary SHA256 mismatch')

def verify_freeze(root):
    value=json.loads((root/'main-freeze.json').read_text())
    require(bool(value['files']),'empty main freeze')
    for item in value['files']:
        path=Path(item['path']);require(path.is_file() and path.stat().st_size==item['bytes'] and filehash(path)==item['sha256'],'frozen input changed: '+str(path))

def inventory(corpus):
    return [json.loads(line) for line in (corpus/"inventory.jsonl").read_text().splitlines()]

def selected(rows,size,start=0):
    result=[];total=0
    for d in rows[start:]:
        if total+d["bytes"]>size:break
        result.append(d);total+=d["bytes"]
    require(bool(result),"no whole documents in selection")
    return result

def join(corpus,rows,path):
    h=hashlib.sha256();total=0;start=time.perf_counter()
    with path.open("xb") as out:
        for d in rows:
            data=(corpus/"documents"/d["doc_id"]).read_bytes()
            require(len(data)==d["bytes"] and sha(data)==d["sha256"],"source document changed")
            out.write(data);h.update(data);total+=len(data)
    return {"bytes":total,"sha256":h.hexdigest(),"seconds":time.perf_counter()-start}

def command(root,name,args,seconds=900,deadline=None):
    if deadline is not None:
        seconds=min(seconds,deadline-time.monotonic());require(seconds>0,'total construction deadline')
    result=measure.run(args,root,name,seconds)
    if result["stop_reason"]:raise ValueError("resource stop: "+result["stop_reason"])
    if deadline is not None:require(time.monotonic()<=deadline,'total construction deadline')
    return result

def calibrate(root,corpus,binary):
    rows=inventory(corpus)
    (root/"calibration").mkdir()
    records=[]
    for size,label in [(10*1048576,"10m"),(100*1048576,"100m")]:
        group=selected(rows,size)
        text=root/"calibration"/(label+".txt");join(corpus,group,text)
        qzt=root/"calibration"/(label+".qzt")
        r=command(root,"cal-pack-"+label,[binary,"pack",text,"-o",qzt]);require(r["exit"]==0,"calibration pack failed")
        r=command(root,"cal-verify-"+label,[binary,"verify",qzt,"--deep","--format","json"]);require(r["exit"]==0,"calibration verify failed")
        if label=="10m":
            r=command(root,"cal-index-"+label,[binary,"sidecar-rebuild",qzt,"--index","token","-o",root/"calibration"/(label+".qzi")],120)
            records.append({"condition":label,"index_exit":r["exit"]})
            require(r["exit"]==0,"10 MiB token calibration refused; stop before selecting shards")
    for i,fraction in enumerate([0,.45,.9]):
        group=selected(rows,8*1048576,int(len(rows)*fraction))
        text=root/"calibration"/f"spread-{i}.txt";join(corpus,group,text)
        qzt=text.with_suffix(".qzt")
        r=command(root,f"cal-spread-pack-{i}",[binary,"pack",text,"-o",qzt]);require(r["exit"]==0,"spread pack")
        r=command(root,f"cal-spread-index-{i}",[binary,"sidecar-rebuild",qzt,"--index","token","-o",text.with_suffix(".qzi")],120)
        records.append({"condition":f"spread-{i}","index_exit":r["exit"]})
        require(r["exit"]==0,"8 MiB shard calibration refused; stop, do not raise budgets")
    save(root/"calibration.json",{"records":records,"chosen_index":"token","shard_max_bytes":8*1048576,
                                 "explicit_budget_conditions":[],"subsets_are_not_primary":True})
    print(json.dumps({"calibration":"passed","index":"token","shard_max_bytes":8*1048576}))

def layout(root,corpus,binary):
    require(json.loads((corpus/"corpus.json").read_text())["raw_body_bytes"]>=GIB,"primary corpus size")
    calibration=json.loads((root/"calibration.json").read_text())
    limit=calibration["shard_max_bytes"]
    (root/"material").mkdir();(root/"shards").mkdir();(root/"shard-inputs").mkdir()
    rows=inventory(corpus);shards=[];current=[];size=0
    def finish():
        if not current:return
        sid=len(shards);offset=0
        for d in current:d.update(shard=sid,shard_offset=offset);offset+=d["bytes"]
        record=join(corpus,current,root/"shard-inputs"/f"s{sid:04d}.txt")
        shards.append({"id":sid,**record,"documents":len(current)})
    for d in rows:
        require(d["bytes"]<=limit,"document exceeds shard cap")
        if size+d["bytes"]>limit:finish();current=[];size=0
        current.append(d);size+=d["bytes"]
    finish()
    record=join(corpus,rows,root/"material/corpus.txt")
    source=json.loads((corpus/"corpus.json").read_text())
    require(record["sha256"]==source["concatenated_sha256"],"whole source binding")
    with (root/"inventory.jsonl").open("x") as f:
        for d in rows:f.write(json.dumps(d,ensure_ascii=False,separators=(",",":"))+"\n")
    shutil.copy2(binary,root/"material/qzt");os.chmod(root/"material/qzt",0o555)
    save(root/"configuration.json",{"corpus":str(corpus),"binary":str(root/"material/qzt"),"binary_sha256":filehash(binary),
          "source":"1562940cf9a93e885d32faf0f838a5b288d96497","index":"token","shard_max_bytes":limit,
          "shards":shards,"corpus_bytes":source["body_bytes"],"source_manifest_sha256":filehash(corpus/"corpus.json"),
          "inventory_sha256":filehash(root/"inventory.jsonl"),"document_index":False,
          "routing":"shared deterministic document placement; no oracle routing for search","explicit_limits":[],
          "resource_limits":{"build_single_seconds":900,"build_total_seconds":1800,"child_rss":4*GIB,"artifact_bytes":30*GIB,"free_floor":30*GIB}})
    print(json.dumps({"documents":len(rows),"shards":len(shards),"body_bytes":source["body_bytes"]}))

def prepare(root):
    config=json.loads((root/"configuration.json").read_text());binary=config["binary"]
    require((root/"main-freeze.json").is_file(),"main conditions must be frozen before build")
    verify_freeze(root)
    verify_binary(config)
    start=time.monotonic()
    deadline=start+1800
    qzt=root/"material/whole.qzt"
    for name,args in [("whole-pack",[binary,"pack",root/"material/corpus.txt","-o",qzt]),
                      ("whole-verify",[binary,"verify",qzt,"--deep","--format","json"])]:
        r=command(root,name,args,deadline=deadline);require(r["exit"]==0,name+" failed")
    r=command(root,"whole-index-default",[binary,"sidecar-rebuild",qzt,"--index","token","-o",root/"material/whole.qzi"],300,deadline=deadline)
    whole_exit=r["exit"]
    # This reproduction pins the observed default-budget refusal, not arbitrary errors.
    require(whole_exit==1,'fixed whole-index outcome changed; preserve logs and stop')
    diagnostic=Path(r['stderr']).read_text().splitlines()
    require(diagnostic and diagnostic[0]=='qzt sidecar-rebuild: a resource limit was exceeded','unexpected whole-index failure')
    require(not (root/"material/whole.qzi").exists(),"failed index left successful output")
    results=[]
    for s in config["shards"]:
        require(time.monotonic()-start<1800,"total construction deadline")
        stem=f"s{s['id']:04d}";target=root/"shards"/(stem+".qzt")
        for suffix,args in [("pack",[binary,"pack",root/"shard-inputs"/(stem+".txt"),"-o",target]),
                            ("verify",[binary,"verify",target,"--deep","--format","json"]),
                            ("index",[binary,"sidecar-rebuild",target,"--index","token","-o",target.with_suffix(".qzi")]),
                            ("inspect",[binary,"inspect-sidecar",target,"--sidecar",target.with_suffix(".qzi"),"--format","json"])]:
            r=command(root,stem+"-"+suffix,args,120,deadline=deadline)
            results.append({"shard":s["id"],"operation":suffix,"exit":r["exit"]})
            require(r["exit"]==0,"shard preparation failed; retain failure, do not raise budgets")
        print(json.dumps({"shard_prepared":s["id"],"total":len(config["shards"])}),flush=True)
    verify_binary(config)
    require(time.monotonic()<=deadline,'total construction deadline before success')
    save(root/"preparation.json",{"whole_index_exit":whole_exit,"shards":results,"seconds":time.monotonic()-start,
          "sizes":{"original":config["corpus_bytes"],"whole_qzt":qzt.stat().st_size,
                   "shard_qzt":sum(p.stat().st_size for p in (root/"shards").glob("*.qzt")),
                   "qzi":sum(p.stat().st_size for p in (root/"shards").glob("*.qzi")),
                   "inventory":(root/"inventory.jsonl").stat().st_size}})
    verify_freeze(root)

if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("operation",choices=["calibrate","layout","prepare"])
    parser.add_argument("root",type=Path);parser.add_argument("--corpus",type=Path);parser.add_argument("--binary",type=Path)
    args=parser.parse_args()
    if args.operation=="prepare":prepare(args.root.resolve())
    else:(calibrate if args.operation=="calibrate" else layout)(args.root.resolve(),args.corpus.resolve(),args.binary.resolve())
