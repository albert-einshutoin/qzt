#!/usr/bin/env python3
"""Logged, bounded access to a single trial. No answer-dependent search advice."""
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from harness import budget, citation, require, run_cli, safe_read, save, search_state, sha, validate_answer

def main():
    trial = Path(sys.argv[1]).resolve(strict=True)
    inputs, outputs = trial / "input", trial / "output"
    cfg = json.loads(safe_read(inputs, "config.json"))
    cfg["started_epoch"] = json.loads(safe_read(outputs, "activation.json"))["started_epoch"]
    prompt = json.loads(safe_read(inputs, "prompt.json"))
    op, args = sys.argv[2], sys.argv[3:]
    lock = outputs / "access.lock"
    fd = os.open(lock, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    os.close(fd)
    try:
        logs = sorted((outputs / "access").glob("*.json"))
        if op == "finish":
            require(len(args) == 1, "finish expects JSON argument")
            answer = json.loads(args[0])
            validate_answer(answer, [q["id"] for q in prompt["questions"]])
            # Even a late submission is retained and explicitly classified by the parent.
            save(outputs / "answer.json", answer)
            save(outputs / "submission.json", {"epoch": time.time(), "late": time.time() > cfg["started_epoch"] + 600,
                                               "answer_sha256": sha((outputs / "answer.json").read_bytes())})
            print('{"saved":true}')
            return
        try:
            budget(cfg, len(logs), time.time())
        except ValueError as e:
            denied = outputs / "denied"
            denied.mkdir(exist_ok=True)
            save(denied / f"{len(list(denied.glob('*.json')))+1:03d}.json",
                 {"epoch": time.time(), "operation": op, "args": args, "error": str(e),
                  "admitted_accesses": len(logs)})
            raise
        record = {"operation": op, "args": args, "started_epoch": time.time(), "cli": []}
        def cli(command):
            if cfg["sandbox"]:
                command = [cfg["sandbox"], "-p", "(version 1)(allow default)(deny network*)(deny file-write*)", *command]
            entry = run_cli(command, inputs)
            record["cli"].append(entry)
            require(not entry["timeout"], "CLI timeout; partial stdout/stderr retained")
            return subprocess.CompletedProcess(command, entry["exit"], bytes.fromhex(entry["stdout_hex"]), bytes.fromhex(entry["stderr_hex"]))
        def qzt(*command):
            for name in ("qzt", "evidence.qzt", "evidence.qzi"):
                require(sha(safe_read(inputs, name, 33554432)) == cfg["hashes"][name], "C input integrity")
            result = cli([str(inputs / "qzt"), *map(str, command)])
            require(result.returncode == 0, "QZT command failed: " + result.stderr.decode("utf-8", "replace"))
            return result.stdout
        try:
            if op == "info":
                require(not args, "info arguments")
                result = prompt
            elif op == "search":
                require(cfg["condition"] in ("B", "C") and len(args) == 1 and 0 < len(args[0].encode()) <= 4096, "search arguments/permission")
                if cfg["condition"] == "B":
                    r = cli([cfg["rg"], "--no-config", "-n", "-F", "--", args[0], "body"])
                    require(r.returncode in (0, 1), "rg error")
                    result = {"matches": r.stdout.decode(), "note": "literal local rg; zero matches is not general absence"}
                else:
                    result = json.loads(qzt("search", "evidence.qzt", args[0], "--sidecar", "evidence.qzi", "--max-results", 30, "--format", "json"))
                    record["search_state"] = search_state(result)
            elif op == "context":
                require(cfg["condition"] == "C" and len(args) == 2 and all(a.isdecimal() for a in args), "context arguments/permission")
                hits = [hit for p in logs for hit in json.loads(p.read_text()).get("result", {}).get("hits", [])]
                require(any(hit["logical_offset"] == int(args[0]) and hit["byte_length"] == int(args[1]) for hit in hits), "context must use an observed search hit")
                result = json.loads(qzt("context", "evidence.qzt", "--offset", args[0], "--length", args[1], "--before", 2, "--after", 2, "--format", "json"))
                require(result["mapping_status"] == "unique", "context not uniquely mapped")
                manifest = json.loads(safe_read(inputs, "manifest.json"))
                d = next(d for d in manifest["documents"] if d["doc_id"] == result["document"]["id"])
                lo, hi = result["excerpt"]["logical_offset"], result["excerpt"]["logical_offset"] + result["excerpt"]["byte_length"]
                require(d["logical_offset"] <= lo <= int(args[0]) and int(args[0])+int(args[1]) <= hi <= d["logical_offset"]+d["bytes"], "context ranges")
                excerpt = bytes.fromhex(result["excerpt"]["bytes_hex"])
                require(len(excerpt) == result["excerpt"]["byte_length"], "context bytes")
                record["context_doc"] = d["doc_id"]
                record["partial"] = result["excerpt"]["leading_fragment"] or result["excerpt"]["trailing_fragment"]
            elif op == "doc":
                require(cfg["condition"] in ("B", "C") and len(args) == 1, "doc arguments/permission")
                manifest = json.loads(safe_read(inputs, "manifest.json"))
                d = next(d for d in manifest["documents"] if d["doc_id"] == args[0])
                if cfg["condition"] == "C":
                    require(any(json.loads(p.read_text()).get("context_doc") == args[0] for p in logs), "C doc requires prior search/context")
                    data = qzt("doc", "evidence.qzt", args[0])
                else:
                    data = safe_read(inputs, "body/" + args[0])
                require(len(data) == d["bytes"] and sha(data) == d["sha256"], "document checksum")
                cached = outputs / "documents" / args[0]
                if not cached.exists():
                    cached.write_bytes(data)
                result = {"doc_id": args[0], "text": data.decode(), "sha256": sha(data)}
            elif op == "cite":
                require(len(args) == 2, "cite arguments")
                if args[0] == "summary":
                    data = prompt["summary"].encode()
                else:
                    require(cfg["condition"] != "A", "A has no originals")
                    data = safe_read(outputs, "documents/" + args[0])
                result = citation(args[0], data, args[1])
            else:
                raise ValueError("unsupported operation")
            returned = (json.dumps(result, ensure_ascii=False) + "\n").encode()
            require(len(returned) <= 2097152, "output budget")
            record.update(result=result, returned_bytes=len(returned), ok=True)
        except Exception as e:
            error = f"{type(e).__name__}: {e}"
            record.update(ok=False, error=error, returned_bytes=len(("Error: " + error + "\n").encode()))
            raise
        finally:
            record["finished_epoch"] = time.time()
            save(outputs / "access" / f"{len(logs)+1:02d}.json", record)
        sys.stdout.buffer.write(returned)
    finally:
        lock.unlink()

if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"Error: {type(e).__name__}: {e}", file=sys.stderr)
        sys.exit(1)
