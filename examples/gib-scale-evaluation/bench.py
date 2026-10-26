#!/usr/bin/env python3
"""Fixed serial discovery queries and 1/10/100 known-reference sequences."""
import json
import random
import statistics
import subprocess
import sys
import time
from pathlib import Path
from corpus import save,require,sha
from access import Access
from experiment import verify_freeze

def main(root):
    require((root/'main-freeze.json').exists(),'unfrozen benchmark')
    verify_freeze(root)
    queries=json.loads((root/'machine-queries.json').read_text());refs=json.loads((root/'reference-sequences.json').read_text())
    output=root/'benchmark';output.mkdir();records=[]
    readers={}
    for c in 'BC':
        p=output/c;p.mkdir();readers[c]=Access(root,c,p,deadline=time.monotonic()+3600)
    for repetition in range(queries['repetitions']):
        for query in queries['queries']:
            for c in ('BC' if repetition%2==0 else 'CB'):
                reader=readers[c];reader.deadline=time.monotonic()+120;before=len(reader.calls);start=time.perf_counter()
                result=reader.search(query)
                records.append({'kind':'search','condition':c,'repetition':repetition,'query':query,'seconds':time.perf_counter()-start,
                    'calls':len(reader.calls)-before,'returned_bytes':len(json.dumps(result,ensure_ascii=False).encode()),'hits':len(result['hits']),
                    'capped':result['capped'],'partial_shards':result['partial_shards'],'coverage_verified':False,'native_metrics_sum':result['native_metrics_sum']})
    for kind in ['same_point','distributed']:
        for n in refs['counts']:
            for c in 'BC':
                reader=readers[c];before=len(reader.calls);start=time.perf_counter();checks=[]
                for ref in refs[kind][:n]:
                    reader.deadline=time.monotonic()+120;result=reader.read(ref['doc_id'],ref['start'],ref['length']);checks.append(sha(result['text'].encode())==ref['fragment_sha256'])
                require(all(checks),'known reference correspondence')
                records.append({'kind':kind,'condition':c,'accesses':n,'seconds':time.perf_counter()-start,'calls':len(reader.calls)-before,'all_bytes_match':True})
    # A single QZT can serve known references even if its search index was refused.
    config=readers['C'].config
    for kind in ['same_point','distributed']:
        for n in refs['counts']:
            reader=readers['C'];before=len(reader.calls);start=time.perf_counter();checks=[]
            for ref in refs[kind][:n]:
                d=reader.docs[ref['doc_id']];offset=d['global_offset']+ref['start'];reader.deadline=time.monotonic()+120
                data,_=reader.native([config['binary'],'range',root/'material/whole.qzt','--bytes',f"{offset}:{offset+ref['length']}"])
                checks.append(sha(data)==ref['fragment_sha256'])
            require(all(checks),'single container reference bytes')
            records.append({'kind':kind,'condition':'C_single_without_index','accesses':n,'seconds':time.perf_counter()-start,'calls':len(reader.calls)-before,'all_bytes_match':True})
    save(root/'machine-results.json',{'records':records,'cache':'warm/unknown','sequential':True,'concurrent_agent_work':False,
             'inventory_load_seconds':{c:r.inventory_load_seconds for c,r in readers.items()},
             'native_calls':{c:r.calls for c,r in readers.items()},'actual_os_io_bytes':'unknown; only exposed native decoded/posting counters are measured'})
    print(json.dumps({'records':len(records),'all_known_references_match':True}))

if __name__=='__main__':main(Path(sys.argv[1]).resolve())
