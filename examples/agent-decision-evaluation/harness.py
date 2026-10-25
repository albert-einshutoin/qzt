#!/usr/bin/env python3
"""Small offline evidence checks shared by preparation and result audit."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
import acquire

def require(ok, message):
    if not ok:
        raise ValueError(message)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def save(path, value):
    with Path(path).open("x", encoding="utf-8") as f:
        json.dump(value, f, ensure_ascii=False, indent=2)
        f.write("\n")

def safe_read(root, relative, cap=2097152):
    root = Path(root).resolve(strict=True)
    rel = Path(relative)
    require(not rel.is_absolute() and ".." not in rel.parts, "path escapes root")
    current = root
    for part in rel.parts:
        current /= part
        require(not current.is_symlink(), "symlink refused")
    require(current.is_file() and current.resolve().is_relative_to(root), "regular contained file required")
    with current.open("rb") as f:
        data = f.read(cap + 1)
    require(len(data) <= cap, "file budget")
    return data

def snapshot(root):
    policy = json.loads(safe_read(root, "policy.json"))
    require(policy["urls"] == acquire.URLS and policy["limits"] == acquire.LIMITS
            and policy["credentials"] is False and policy["redirects"] is False, "acquisition policy")
    manifest = json.loads(safe_read(root, "manifest.json"))
    require(manifest["schema"] == "qzt-agent-snapshot-v1", "snapshot schema")
    responses = manifest["responses"]
    require([r["url"] for r in responses] == acquire.URLS, "response allowlist")
    expected, docs, total = [], {}, 0
    for i, r in enumerate(responses):
        raw = safe_read(root, r["path"])
        require(r["status"] == 200 and r["bytes"] == len(raw) and r["sha256"] == sha(raw), "raw integrity")
        require('rel="next"' not in r["headers"].get("Link", ""), "pagination")
        total += len(raw)
        if r["url"].startswith(acquire.REPO):
            value = json.loads(raw)
            items = list(enumerate(value)) if isinstance(value, list) else [(None, value)]
            require(len(items) <= 30, "collection budget")
            for j, obj in items:
                body = obj.get("body") or ""
                if body:
                    expected.append((i, "/body" if j is None else f"/{j}/body", body.encode(), obj))
        else:
            expected.append((i, None, raw, None))
    require(total <= 10485760 and len(expected) == len(manifest["documents"]) <= 20, "snapshot budgets")
    for n, (d, (i, pointer, data, obj)) in enumerate(zip(manifest["documents"], expected), 1):
        require(d["doc_id"] == f"d{n:02d}.txt" and d["response"] == i and d["source"]["pointer"] == pointer,
                "document identity")
        require(safe_read(root, d["path"]) == data and d["bytes"] == len(data) and d["sha256"] == sha(data), "body integrity")
        if obj is not None:
            require(d["source"]["object_id"] == obj["id"] and d["source"]["node_id"] == obj["node_id"]
                    and d["source"]["url"] == obj["html_url"], "object identity")
        else:
            require(d["source"]["url"] == responses[i]["url"] and d["source"]["commit"] in (acquire.MAIN, acquire.HEAD), "fixed source")
        docs[d["doc_id"]] = data
    require(sum(map(len, docs.values())) <= 1048576, "body budget")
    return manifest, docs

def citation(docid, data, quote):
    needle = quote.encode("utf-8")
    require(bool(needle) and needle in data, "quote absent")
    return {"doc_id": docid, "start": data.index(needle), "length": len(needle),
            "sha256": sha(data), "quote": quote}

def check_citation(cite, docs):
    try:
        require(set(cite) == {"doc_id", "start", "length", "sha256", "quote"}, "citation fields")
        require(cite["doc_id"] in docs, "unknown document")
        data = docs[cite["doc_id"]]
        start, length = cite["start"], cite["length"]
        require(type(start) is int and type(length) is int and start >= 0 and length > 0
                and start + length <= len(data), "citation bounds")
        require(cite["sha256"] == sha(data), "document checksum")
        require(isinstance(cite["quote"], str) and data[start:start+length] == cite["quote"].encode(), "quote byte mismatch")
        return {"valid": True, "source": "summary" if cite["doc_id"] == "summary" else "original"}
    except (ValueError, KeyError, TypeError) as e:
        return {"valid": False, "error": str(e)}

def search_state(value):
    require(type(value["capped"]) is bool and type(value["index_complete_declared"]) is bool
            and value["index_coverage_verified"] is False and isinstance(value["hits"], list), "search types/coverage")
    stops = {"max_candidate_granules", "max_decoded_bytes", "max_physical_decoded_bytes",
             "max_physical_decoded_chunks", "max_search_results"}
    reasons = {"query_shorter_than_ngram_n", "query_has_no_indexable_tokens", "missing_required_key_in_incomplete_index"}
    require(value["stop_reason"] is None or value["stop_reason"] in stops, "unknown stop reason")
    require(value["incomplete_reason"] is None or value["incomplete_reason"] in reasons, "unknown incomplete reason")
    require(value["capped"] == (value["stop_reason"] is not None), "cap mismatch")
    if value["capped"] or value["incomplete_reason"] or not value["index_complete_declared"]:
        return "partial_unknown"
    return "coverage_unknown"

def budget(config, accesses, now):
    require(now <= config["started_epoch"] + config["seconds"], "600 second deadline")
    require(accesses < config["accesses"], "40 access limit")

def validate_answer(value, qids):
    require(isinstance(value, dict) and set(value) == {"answers"} and isinstance(value["answers"], list), "answer schema")
    require(len(value["answers"]) == len(qids) and {a["id"] for a in value["answers"]} == set(qids), "question inventory")
    for a in value["answers"]:
        require(set(a) == {"id", "status", "answer", "citations"}, "answer fields")
        require(a["status"] in ("answered", "unknown", "partial") and isinstance(a["answer"], str)
                and isinstance(a["citations"], list), "answer types")

def submission(out, qids):
    """Keep missing/invalid submissions distinct from an actor's explicit unknown."""
    answer_path, receipt_path = out / "answer.json", out / "submission.json"
    result = {"trial_status": "missing", "failure": None, "answer_sha256": None, "receipt": None,
              "answer": {"answers": [{"id": qid, "status": "missing", "answer": "", "citations": []} for qid in qids]}}
    try:
        raw = safe_read(out, "answer.json")
        result["answer_sha256"] = sha(raw)
        value = json.loads(raw)
        validate_answer(value, qids)
        result["answer"] = value
        receipt = json.loads(safe_read(out, "submission.json"))
        require(receipt["answer_sha256"] == sha(raw), "submission seal")
        require(type(receipt["epoch"]) in (int, float) and type(receipt["late"]) is bool, "receipt types")
        result.update(trial_status="late" if receipt["late"] else "submitted", receipt=receipt)
    except (OSError, ValueError, KeyError, TypeError) as e:
        result["trial_status"] = "invalid" if answer_path.exists() else "missing"
        result["failure"] = f"{type(e).__name__}: {e}"
    return result

def run_cli(command, cwd, timeout=30):
    start = time.perf_counter()
    try:
        r = subprocess.run(command, cwd=cwd, capture_output=True, timeout=timeout, check=False,
                           env={"PATH": "/usr/bin:/bin:/opt/homebrew/bin", "LC_ALL": "en_US.UTF-8"})
        code, stdout, stderr, timed_out = r.returncode, r.stdout, r.stderr, False
    except subprocess.TimeoutExpired as e:
        code, stdout, stderr, timed_out = None, e.stdout or b"", e.stderr or b"", True
    return {"argv": command, "exit": code, "timeout": timed_out, "seconds": time.perf_counter()-start,
            "stdout_hex": stdout.hex(), "stderr_hex": stderr.hex(), "stderr": stderr.decode("utf-8", "replace")}
