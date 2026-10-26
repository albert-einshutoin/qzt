#!/usr/bin/env python3
"""Sequential subprocess measurement with sampled stop budgets, not an OS RSS guarantee."""
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

GIB = 1073741824

def stop_reason(elapsed, rss, free, generated, pressure, warning_seconds, seconds):
    if elapsed > seconds:return "time_budget"
    if rss > 4*GIB:return "sampled_rss_budget"
    if free < 30*GIB:return "free_disk_floor"
    if generated > 30*GIB:return "artifact_budget"
    if pressure >= 4 or warning_seconds >= 10:return "memory_pressure"
    return None

def monitor_snapshot(pid, disk, artifact_dirs):
    result = subprocess.run(["ps","-axo","pid=,ppid=,rss="],capture_output=True,text=True,check=True)
    rows=[tuple(map(int,line.split())) for line in result.stdout.splitlines() if len(line.split())==3]
    family={pid}
    while True:
        added={p for p,parent,_ in rows if parent in family}
        if added <= family:break
        family |= added
    rss=sum(kb*1024 for p,_,kb in rows if p in family)
    r=subprocess.run(["sysctl","-n","kern.memorystatus_vm_pressure_level"],capture_output=True,text=True,check=True)
    pressure=int(r.stdout.strip())
    generated=sum(p.stat().st_size for directory in artifact_dirs if directory.exists() for p in directory.glob("*") if p.is_file())
    # Reserve 4 GiB for capped acquisition, its interrupted attempt and calibration artifacts.
    generated += 4*GIB
    return {"rss_bytes":rss,"free_bytes":shutil.disk_usage(disk).free,"generated_bytes":generated,"pressure":pressure}

def run(command, root, name, seconds=900, stdin=None):
    root=Path(root).resolve()
    logs=root/"measurements"
    logs.mkdir(exist_ok=True)
    record_path=logs/(name+".json")
    if any((logs/(name+suffix)).exists() for suffix in [".json",".stdout",".stderr"]):raise ValueError("existing measurement protected")
    started=time.monotonic();peak=0;warning_start=None;samples=[];reason=None
    wrapper=["/usr/bin/time","-l",*map(str,command)]
    proc=None
    try:
        with (logs/(name+".stdout")).open("xb") as out,(logs/(name+".stderr")).open("xb") as err:
            proc=subprocess.Popen(wrapper,cwd=root,stdin=stdin,stdout=out,stderr=err,start_new_session=True)
            while proc.poll() is None:
                snap=monitor_snapshot(proc.pid,root,[root/"material",root/"shards",root/"shard-inputs"])
                elapsed=time.monotonic()-started
                peak=max(peak,snap["rss_bytes"])
                if snap["pressure"] >= 2:
                    if warning_start is None:warning_start=time.monotonic()
                else:warning_start=None
                warning_seconds=0 if warning_start is None else time.monotonic()-warning_start
                reason=stop_reason(elapsed,snap["rss_bytes"],snap["free_bytes"],snap["generated_bytes"],snap["pressure"],warning_seconds,seconds)
                samples.append({"elapsed":round(elapsed,3),**snap})
                if reason:
                    os.killpg(proc.pid,signal.SIGTERM)
                    try:proc.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        os.killpg(proc.pid,signal.SIGKILL);proc.wait()
                    break
                time.sleep(.25)
            code=proc.wait()
    except Exception:
        if proc is not None and proc.poll() is None:
            os.killpg(proc.pid,signal.SIGKILL);proc.wait()
        raise
    finally:
        stderr=(logs/(name+".stderr")).read_text(errors="replace") if (logs/(name+".stderr")).exists() else ""
        match=re.search(r"(\d+)\s+maximum resident set size",stderr)
        native=re.search(r"([0-9.]+)\s+real\s+([0-9.]+)\s+user\s+([0-9.]+)\s+sys",stderr)
        record={"command":list(map(str,command)),"wrapper":wrapper,"started_epoch":time.time()-(time.monotonic()-started),
                "seconds":time.monotonic()-started,"exit":None if proc is None else proc.returncode,"stop_reason":reason,
                "sampled_peak_family_rss":peak,"time_maximum_rss_bytes":int(match.group(1)) if match else None,
                "native_real_seconds":float(native.group(1)) if native else None,
                "instrument_wall_includes_sampling_wait":True,"artifact_accounting":"outputs/shard inputs plus conservative 4 GiB setup reserve",
                "sampling_seconds":.25,"samples":samples,"strict_os_rss_guarantee":False,
                "stdout":str(logs/(name+".stdout")),"stderr":str(logs/(name+".stderr"))}
        with record_path.open("x") as f:json.dump(record,f,indent=2);f.write("\n")
    return record

if __name__=="__main__":
    record=run(sys.argv[4:],Path(sys.argv[1]),sys.argv[2],float(sys.argv[3]))
    print(json.dumps({k:v for k,v in record.items() if k!="samples"}))
    sys.exit(0 if record["exit"]==0 and not record["stop_reason"] else 1)
