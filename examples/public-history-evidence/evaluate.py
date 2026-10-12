#!/usr/bin/env python3
"""Apply frozen criteria to a structurally validated recipient report."""
import argparse
import hashlib
import importlib.util
import json
import sys
from pathlib import Path


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def read(path):
    require(path.is_file() and not path.is_symlink(), f"regular file required: {path}")
    return path.read_bytes()


def recipient_module():
    path = Path(__file__).with_name("recipient.py")
    spec = importlib.util.spec_from_file_location("public_history_recipient", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--evaluation", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report-sha256")
    parser.add_argument("--manifest-sha256")
    args = parser.parse_args()
    require(not args.output.exists() and not args.output.is_symlink(), "fresh evaluation output required")
    report_bytes, manifest_bytes, evaluation_bytes, receipt_bytes = read(args.report), read(args.manifest), read(args.evaluation), read(args.receipt)
    report_sha, manifest_sha = hashlib.sha256(report_bytes).hexdigest(), hashlib.sha256(manifest_bytes).hexdigest()
    require(args.report_sha256 is None or args.report_sha256 == report_sha, "external report seal mismatch")
    require(args.manifest_sha256 is None or args.manifest_sha256 == manifest_sha, "external manifest seal mismatch")
    report, manifest, evaluation, receipt = json.loads(report_bytes), json.loads(manifest_bytes), json.loads(evaluation_bytes), json.loads(receipt_bytes)
    require(receipt.get("schema") == "qzt-public-history-producer-receipt-v3" and receipt.get("evaluation_sha256") == hashlib.sha256(evaluation_bytes).hexdigest(), "receipt/evaluation binding")
    questions_bytes = read(args.manifest.with_name("questions.json"))
    require(receipt.get("questions_sha256") == hashlib.sha256(questions_bytes).hexdigest(), "receipt/questions binding")
    require(receipt.get("product_source_sha_self_reported") == manifest.get("source", {}).get("product_source_sha_self_reported") and receipt.get("archive_commit_sha") == manifest.get("source", {}).get("archive_commit_sha") and receipt.get("snapshot_sha256") == manifest.get("source", {}).get("snapshot_sha256") and receipt.get("policy_sha256") == manifest.get("source", {}).get("policy_sha256"), "receipt/manifest source binding")
    require(receipt.get("binary_sha256") == manifest.get("runtime_inventory", {}).get("binary_sha256") and receipt.get("helper_sha256") == manifest.get("runtime_inventory", {}).get("recipient_sha256"), "receipt/runtime binding")
    receipt_documents = receipt.get("documents")
    require(isinstance(receipt_documents, list) and len(receipt_documents) == len(manifest.get("documents", [])), "receipt document count")
    for frozen, document in zip(receipt_documents, manifest["documents"]):
        require(frozen.get("filename") == document["doc_id"].removeprefix("github/") and frozen.get("size") == document["byte_length"] and frozen.get("sha256") == document["checksum"]["value"] and frozen.get("source") == document["source"], "receipt document binding")
    recipient = recipient_module()
    recipient.validate_questions(json.loads(questions_bytes))
    recipient.validate_manifest(manifest, questions_bytes, read(args.manifest.with_name("qzt.exe" if sys.platform == "win32" else "qzt")), read(args.manifest.with_name("recipient.py")))
    entries = recipient.validate_report(report, manifest, manifest_sha, questions_bytes)
    require(evaluation.get("schema") == "qzt-public-history-evaluation-v1" and isinstance(evaluation.get("questions"), dict) and set(evaluation["questions"]) == set(entries), "evaluation identity")
    results = []
    for qid, criterion in evaluation["questions"].items():
        require(set(criterion) == {"expected", "required", "classification"} and isinstance(criterion["expected"], str) and criterion["expected"] and criterion["classification"] in ("acquired", "record_insufficient") and isinstance(criterion["required"], list), "criterion schema")
        require((criterion["classification"] == "record_insufficient") == (criterion["required"] == []), "criterion classification")
        entry = entries[qid]
        matched = []
        for required in criterion["required"]:
            require(set(required) == {"doc_id", "fragment"} and isinstance(required["fragment"], str) and required["fragment"], "required source")
            candidates = [hit for hit in entry["hits"] if hit["doc_id"] == required["doc_id"] and required["fragment"].encode() in bytes.fromhex(hit["context"]["excerpt"]["bytes_hex"])]
            matched.append({"doc_id": required["doc_id"], "fragment": required["fragment"], "matched": bool(candidates), "hit_count": len(candidates)})
        if entry["candidate_status"] == "partial_unknown":
            outcome = "partial_unknown"
        else:
            outcome = "record_insufficient" if not criterion["required"] else "acquired" if all(item["matched"] for item in matched) else "missing"
        diagnostics = [{"query": search["query"], "reason": search["reason"]} for search in entry["searches"] if search["classification"] == "partial_unknown"]
        if entry["candidate_status"] == "partial_unknown" and not diagnostics:
            diagnostics.append({"query": None, "reason": "context_partial"})
        results.append({"id": qid, "outcome": outcome, "required": matched, "candidate_status": entry["candidate_status"], "diagnostics": diagnostics, "coverage_verified": False})
    output = {"schema": "qzt-public-history-evaluation-report-v4", "report_sha256": report_sha, "manifest_sha256": manifest_sha, "receipt_sha256": hashlib.sha256(receipt_bytes).hexdigest(), "questions": results, "note": "acquired proves only validated report bytes match the manifest SHA-256 and fixed fragment correspondence. Report/input SHA-256 values are optional external integrity expectations, not source authentication, authorship, meaning, or consent."}
    with args.output.open("xb") as handle:
        handle.write((json.dumps(output, ensure_ascii=False, indent=2) + "\n").encode())


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, KeyError, TypeError) as error:
        print(f"Error: {error}", file=sys.stderr)
        sys.exit(1)
