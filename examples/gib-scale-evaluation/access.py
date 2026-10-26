#!/usr/bin/env python3
"""Symmetric bounded access over common inventory; raw commands remain local-only."""
import fcntl
import json
import os
import re
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path
from experiment import filehash, inventory, search_state, validate_answer
from corpus import require, save, sha

def excerpt(data,start):
    for left in range(min(3,len(data))+1):
        for right in range(min(3,len(data)-left)+1):
            fragment=data[left:len(data)-right if right else len(data)]
            try:text=fragment.decode("utf-8")
            except UnicodeDecodeError:continue
            return text,start+left,len(fragment),bool(left),bool(right)
    raise ValueError("invalid internal UTF8")

def validate(value,qids):
    require(set(value)=={"answers","investigation"},"reply schema")
    validate_answer({"answers":value["answers"]},qids)
    require(isinstance(value["investigation"],dict) and set(value["investigation"])==set(qids) and all(x in ("examined","not_examined") for x in value["investigation"].values()),"investigation state")

class Access:
    def __init__(self,root,condition,logs,deadline=None):
        self.root=root;self.condition=condition;self.logs=logs;self.calls=[]
        require(condition in ('B','C'),'unknown comparison condition')
        self.deadline=deadline or time.monotonic()+120;self.plain_read_seconds=0
        load_started=time.perf_counter()
        self.config=json.loads((root/"configuration.json").read_text())
        self.corpus=Path(self.config["corpus"])
        self.rows=[json.loads(line) for line in (root/"inventory.jsonl").read_text().splitlines()]
        self.docs={d["doc_id"]:d for d in self.rows}
        self.by_shard={s["id"]:[] for s in self.config["shards"]}
        for d in self.rows:self.by_shard[d["shard"]].append(d)
        self.inventory_load_seconds=time.perf_counter()-load_started

    def native(self,args):
        n=len(self.calls);prefix=self.logs/f"native-{n:04d}"
        start=time.perf_counter();proc=None;code=None;timed_out=False
        remaining=self.deadline-time.monotonic();require(remaining>0,"operation deadline")
        with prefix.with_suffix(".stdout").open("xb") as out,prefix.with_suffix(".stderr").open("xb") as err:
            proc=subprocess.Popen(["/usr/bin/time","-l",*map(str,args)],stdout=out,stderr=err,cwd=self.corpus,start_new_session=True)
            timeout_state=[]
            def kill():
                if proc.poll() is None:
                    timeout_state.append(True);os.killpg(proc.pid,signal.SIGKILL)
            timer=threading.Timer(min(90,remaining),kill);timer.daemon=True;timer.start()
            try:code=proc.wait()
            finally:timer.cancel();timer.join()
            timed_out=bool(timeout_state)
        stdout=prefix.with_suffix(".stdout");stderr=prefix.with_suffix(".stderr")
        entry={"argv":list(map(str,args)),"seconds":time.perf_counter()-start,"exit":code,"timeout":timed_out,
               "stdout_bytes":stdout.stat().st_size,"stderr_bytes":stderr.stat().st_size,"stdout_sha256":filehash(stdout),
               "stdout_path":str(stdout),"stderr_path":str(stderr)}
        timing=re.search(r'([0-9.]+)\s+real',stderr.read_text(errors='replace'))
        entry['native_real_seconds_rounded']=float(timing.group(1)) if timing else None
        self.calls.append(entry)
        require(not timed_out,"native timeout")
        require(stdout.stat().st_size<=4*1048576 and stderr.stat().st_size<=4*1048576,"native output cap")
        require(code in (0,1) if str(args[0]).endswith("rg") else code==0,"native command error: "+stderr.read_text(errors="replace")[:2000])
        return stdout.read_bytes(),code

    def read(self,docid,start=0,length=8000):
        require(docid in self.docs and type(start) is int and type(length) is int,"document/range identity")
        d=self.docs[docid]
        require(0<=start<d["bytes"] and 0<length<=16384,"read bounds")
        end=min(d["bytes"],start+length)
        if self.condition=="B":
            started=time.perf_counter()
            with (self.corpus/"documents"/docid).open("rb") as f:f.seek(start);data=f.read(end-start)
            self.plain_read_seconds+=time.perf_counter()-started
        else:
            offset=d["shard_offset"]+start;sid=d["shard"]
            data,_=self.native([self.config["binary"],"range",self.root/"shards"/f"s{sid:04d}.qzt","--bytes",f"{offset}:{offset+end-start}"])
        require(len(data)==end-start,"short source read")
        text,actual,length,leading,trailing=excerpt(data,start)
        return {"doc_id":docid,"start":actual,"length":length,"sha256":d["sha256"],"text":text,
                "leading_utf8_fragment":leading,"trailing_utf8_fragment":trailing,"document_bytes":d["bytes"]}

    def find(self,query):
        require(isinstance(query,str) and 0<len(query.encode())<=1024,"inventory query")
        matches=[{"doc_id":d["doc_id"],"title":d["title"],"bytes":d["bytes"],"revision_id":d["revision_id"]}
                 for d in self.rows if query.casefold() in d["title"].casefold()]
        return {"documents":matches[:20],"capped":len(matches)>20,"inventory_only":True}

    def within(self,docid,query):
        require(docid in self.docs and isinstance(query,str) and 0<len(query.encode())<=4096,"scoped search arguments")
        d=self.docs[docid];started=time.perf_counter()
        if self.condition=='B':
            data=(self.corpus/'documents'/docid).read_bytes();self.plain_read_seconds+=time.perf_counter()-started
        else:
            offset=d['shard_offset'];path=self.root/'shards'/f"s{d['shard']:04d}.qzt"
            data,_=self.native([self.config['binary'],'range',path,'--bytes',f"{offset}:{offset+d['bytes']}"])
        require(len(data)==d['bytes'],'scoped document length')
        needle=query.encode();positions=[];at=0
        while len(positions)<4:
            at=data.find(needle,at)
            if at<0:break
            positions.append(at);at+=len(needle)
        hits=[]
        for at in positions[:3]:
            lo=max(0,at-150);text,start,n,left,right=excerpt(data[lo:min(len(data),at+len(needle)+550)],lo)
            hits.append({'doc_id':docid,'start':start,'length':n,'sha256':d['sha256'],'text':text,'leading_utf8_fragment':left,'trailing_utf8_fragment':right,'document_bytes':d['bytes']})
        return {'hits':hits,'capped':len(positions)>3,'coverage_verified':False,'method':'bounded single-document restore then common Python literal search; temporary buffer only, no persistent cache'}

    def search(self,query):
        require(isinstance(query,str) and 0<len(query.encode())<=4096 and not query.startswith("-"),"search query")
        candidates=[];partial=[];metrics=[]
        if self.condition=="B":
            raw,code=self.native([shutil_rg(),"--no-config","--files-with-matches","--fixed-strings","--word-regexp","--sort","path","--threads","1","--",query,"documents"])
            names=raw.decode().splitlines()
            for path in names:
                docid=Path(path).name
                require(docid in self.docs,"rg returned unknown document")
                candidates.append((docid,None))
        else:
            for s in self.config["shards"]:
                sid=s["id"];path=self.root/"shards"/f"s{sid:04d}.qzt"
                raw,_=self.native([self.config["binary"],"search",path,query,"--sidecar",path.with_suffix(".qzi"),"--max-results","8","--format","json"])
                result=json.loads(raw);state=search_state(result);metrics.append(result["metrics"])
                if state=="partial_unknown":partial.append({"shard":sid,"reason":result["stop_reason"] or result["incomplete_reason"] or "incomplete_index"})
                for h in result["hits"]:
                    found=[d for d in self.by_shard[sid] if d["shard_offset"]<=h["logical_offset"] and h["logical_offset"]+h["byte_length"]<=d["shard_offset"]+d["bytes"]]
                    if len(found)!=1:
                        partial.append({"shard":sid,"reason":"cross_document_hit"});continue
                    d=found[0];candidates.append((d["doc_id"],h["logical_offset"]-d["shard_offset"]))
        unique={}
        for docid,offset in candidates:unique.setdefault(docid,offset)
        hits=[]
        for docid,offset in sorted(unique.items())[:12]:
            if offset is None:
                raw,_=self.native([shutil_rg(),'--no-config','--byte-offset','--only-matching','--max-count','1','--fixed-strings','--word-regexp','--',query,self.corpus/'documents'/docid])
                first=raw.splitlines()[0];coordinate,match=first.split(b':',1)
                require(coordinate.isdigit() and match==query.encode(),'rg/source match coordinate')
                offset=int(coordinate)
            hit=self.read(docid,max(0,offset-120),800)
            hits.append(hit)
        summed={k:sum(m.get(k,0) for m in metrics) for k in ["decoded_bytes","physical_decoded_bytes","physical_decoded_chunks","posting_bytes_read","query_time_ms"]}
        return {"hits":hits,"capped":len(unique)>12,"matching_documents_observed":len(unique),"partial_diagnostics":partial[:10],
                "partial_shards":len(partial),"coverage_verified":False,"native_metrics_sum":summed,"fanout":len(self.config["shards"]) if self.condition=="C" else 1,
                "semantics":"rg literal whole word" if self.condition=="B" else "QZT token conjunction per line; coverage unknown"}

def shutil_rg():
    import shutil
    result=shutil.which("rg");require(result is not None,"rg required");return result

def main():
    trial=Path(sys.argv[1]).resolve();op=sys.argv[2];args=sys.argv[3:]
    cfg=json.loads((trial/"input.json").read_text());root=Path(cfg["root"])
    out=trial/"output";start=time.perf_counter();activation=json.loads((out/"activation.json").read_text())
    if op=="finish":
        value=json.loads(args[0]);validate(value,[q["id"] for q in cfg["questions"]]);save(out/"answer.json",value)
        save(out/"submission.json",{"epoch":time.time(),"late":time.time()>activation["started_epoch"]+cfg["seconds"],"answer_sha256":filehash(out/"answer.json")})
        print('{"saved":true}');return
    events=sorted((out/"events").glob("*.json"));used=sum(json.loads(p.read_text())["returned_bytes"] for p in events)
    if not (len(events)<cfg["accesses"] and time.time()<=activation["started_epoch"]+cfg["seconds"] and used<cfg["total_returned_bytes"]):
        denied=out/"denied";denied.mkdir(exist_ok=True)
        save(denied/f"{len(list(denied.glob('*.json')))+1:03d}.json",{"operation":op,"args":args,"epoch":time.time(),"reason":"trial_budget","admitted":len(events),"returned_bytes_so_far":used})
        raise ValueError("trial budget exhausted; preserve unfinished response with finish")
    logdir=out/f"op-{len(events)+1:03d}";logdir.mkdir()
    record={"operation":op,"args":args,"epoch":time.time(),"native":[],"returned_bytes":0};reader=None
    try:
        lock=(root/"cli.lock").open("a+")
        queued=time.perf_counter();fcntl.flock(lock,fcntl.LOCK_EX);record["queue_seconds"]=time.perf_counter()-queued
        try:
            require(time.time()<=activation["started_epoch"]+cfg["seconds"],"deadline after queue wait")
            if op=="info":result={k:cfg[k] for k in ["summary","questions","usage","seconds","accesses","total_returned_bytes"]}
            elif op=="cite":
                require(len(args)==2 and bool(args[1]),"citation arguments")
                successful=[json.loads(p.read_text()) for p in events if json.loads(p.read_text()).get('ok') is True]
                fragments=[f for event in successful for f in event.get("fragments",[]) if f["doc_id"]==args[0]]
                found=next((f for f in fragments if args[1] in f["text"]),None);require(found is not None,"quote not in prior returned bytes")
                data=found["text"].encode();needle=args[1].encode();offset=found["start"]+data.index(needle)
                result={"doc_id":args[0],"start":offset,"length":len(needle),"sha256":found["sha256"],"quote":args[1]}
            else:
                remaining=activation["started_epoch"]+cfg["seconds"]-time.time()
                reader=Access(root,cfg["condition"],logdir,time.monotonic()+min(120,remaining))
                if op=="find" and len(args)==1:result=reader.find(args[0])
                elif op=="search" and len(args)==1:result=reader.search(args[0])
                elif op=="within" and len(args)==2:result=reader.within(args[0],args[1])
                elif op=="read" and 1<=len(args)<=3:result=reader.read(args[0],int(args[1]) if len(args)>1 else 0,int(args[2]) if len(args)>2 else 8000)
                else:raise ValueError("operation/arguments")
            encoded=(json.dumps(result,ensure_ascii=False)+"\n").encode()
            require(len(encoded)<=65536 and used+len(encoded)<=cfg["total_returned_bytes"],"returned byte budget")
            record["fragments"]=result.get("hits",[]) if op in ("search","within") else [result] if op=="read" else []
            record.update(ok=True,returned_bytes=len(encoded),result_sha256=sha(encoded));sys.stdout.buffer.write(encoded)
        finally:fcntl.flock(lock,fcntl.LOCK_UN);lock.close()
    except Exception as e:
        record.update(ok=False,error=str(e),returned_bytes=len(("Error: "+str(e)+"\n").encode()));raise
    finally:
        if reader:
            record["native"]=reader.calls;record["inventory_load_seconds"]=reader.inventory_load_seconds;record["plain_read_seconds"]=reader.plain_read_seconds
        record["seconds"]=time.perf_counter()-start;save(out/"events"/f"{len(events)+1:03d}.json",record)

if __name__=="__main__":
    try:main()
    except Exception as e:print("Error: "+str(e),file=sys.stderr);sys.exit(1)
