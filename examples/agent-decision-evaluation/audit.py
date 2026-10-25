#!/usr/bin/env python3
"""Verify sealed inputs and answer citations against the original frozen bytes."""
import json
import sys
from pathlib import Path
from harness import check_citation, require, save, sha, snapshot, submission

ROOT = Path(__file__).resolve().parent

def audit(runtime):
    freeze = json.loads((runtime / "freeze.json").read_text())
    for item in freeze["files"]:
        path = Path(item["path"])
        require(path.is_file() and not path.is_symlink(), "frozen file missing/symlink")
        data = path.read_bytes()
        require(len(data) == item["bytes"] and sha(data) == item["sha256"], "frozen input changed: " + str(path))
    manifest, docs = snapshot(ROOT / "snapshot")
    full = b"".join(docs.values())
    offsets, offset = {}, 0
    for docid, data in docs.items():
        offsets[docid] = offset
        offset += len(data)
    prompt = json.loads((ROOT / "questions.json").read_text())
    docs["summary"] = prompt["summary"].encode()
    qids = [q["id"] for q in prompt["questions"]]
    results = []
    plan = json.loads((runtime / "run-plan.json").read_text())
    for p in plan["trials"]:
        out = runtime / "trials" / p["trial"] / "output"
        state = submission(out, qids)
        answer = state["answer"]
        activation = json.loads((out / "activation.json").read_text())
        receipt = state["receipt"]
        events = [json.loads(path.read_text()) for path in sorted((out/"access").glob("*.json"))]
        require(len(events) <= 40, "access cap")
        for event in events:
            require(event["started_epoch"] <= activation["started_epoch"] + 600, "late access")
            if not event["ok"]:
                continue
            if event["operation"] == "context":
                result = event["result"]
                start = result["excerpt"]["logical_offset"]
                data = bytes.fromhex(result["excerpt"]["bytes_hex"])
                require(full[start:start+len(data)] == data, "context/original mismatch")
                d = result["document"]
                require(d["logical_offset"] == offsets[d["id"]] and d["byte_length"] == len(docs[d["id"]]), "context document identity")
            if event["operation"] == "doc":
                d = event["result"]
                require(d["text"].encode() == docs[d["doc_id"]] and d["sha256"] == sha(docs[d["doc_id"]]), "doc/original mismatch")
            if event["operation"] == "search" and p["condition"] == "C":
                for hit in event["result"]["hits"]:
                    start, length = hit["logical_offset"], hit["byte_length"]
                    require(full[start:start+length] == event["args"][0].encode(), "search hit/original mismatch")
        cite_checks = [{"id": a["id"], "citations": [check_citation(c, docs) for c in a["citations"]]} for a in answer["answers"]]
        denials = [json.loads(path.read_text()) for path in sorted((out/"denied").glob("*.json"))]
        results.append({"trial": p["trial"], "condition": p["condition"], "answer_sha256": state["answer_sha256"],
                        "trial_status": state["trial_status"], "submission_failure": state["failure"],
                        "missing_questions": [a["id"] for a in answer["answers"] if a["status"] == "missing"],
                        "seconds_to_submission": receipt["epoch"] - activation["started_epoch"] if receipt else None,
                        "late": receipt["late"] if receipt else None,
                        "accesses": len(events), "failed_accesses": sum(not e["ok"] for e in events),
                        "denied_attempts": denials, "total_access_attempts": len(events)+len(denials),
                        "searches": sum(e["operation"] == "search" for e in events),
                        "cli_calls": sum(len(e["cli"]) for e in events),
                        "returned_bytes": sum(e["returned_bytes"] for e in events),
                        "cli_seconds": sum(c["seconds"] for e in events for c in e["cli"]),
                        "citation_checks": cite_checks,
                        "tool_scope_audit": "pending trace review; gateway log is not complete agent isolation"})
    return {"schema": "qzt-agent-mechanical-audit-v1", "freeze_sha256": sha((runtime/"freeze.json").read_bytes()), "trials": results}

if __name__ == "__main__":
    save(Path(sys.argv[2]), audit(Path(sys.argv[1])))
