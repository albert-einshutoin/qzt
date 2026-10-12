#!/usr/bin/env python3
"""Source-blind search, context, and original-byte verification recipient."""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_small(path, limit=2 * 1024 * 1024):
    require(path.is_file() and not path.is_symlink(), f"regular file required: {path}")
    data = path.read_bytes()
    require(len(data) <= limit, f"input cap exceeded: {path}")
    return data


def write_new(path, value):
    require(not path.exists() and not path.is_symlink(), f"existing output protected: {path}")
    path.write_bytes((json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode())


class Runner:
    def __init__(self, binary):
        self.binary = binary.resolve()
        self.records = []

    def call(self, *args):
        command = [str(self.binary), *map(str, args)]
        started = time.perf_counter()
        result = subprocess.run(command, cwd=Path.cwd(), capture_output=True, timeout=30, check=False)
        record = {"command": command, "exit_code": result.returncode,
                  "wall_ms": round((time.perf_counter() - started) * 1000, 3),
                  "stdout_utf8": result.stdout.decode("utf-8"), "stderr_utf8": result.stderr.decode("utf-8")}
        self.records.append(record)
        require(result.returncode == 0, f"CLI failed: {args[0]} exit={result.returncode}")
        return result.stdout

    def json(self, *args):
        return json.loads(self.call(*args))


def validate_questions(value):
    require(isinstance(value, dict) and set(value) == {"schema", "questions"}
            and value["schema"] == "qzt-public-history-questions-v1", "question schema")
    require(isinstance(value["questions"], list) and len(value["questions"]) == 7, "seven questions required")
    seen = set()
    for question in value["questions"]:
        require(isinstance(question, dict) and set(question) == {"id", "question", "queries"}, "question fields")
        require(question["id"] not in seen and question["question"], "question identity")
        require(isinstance(question["queries"], list) and question["queries"], "explicit query required")
        require(all(isinstance(query, str) and len(query.encode()) >= 3 for query in question["queries"]), "query text")
        seen.add(question["id"])


def validate_manifest(value):
    require(isinstance(value, dict) and set(value) == {"schema", "container_id", "documents", "selection", "source", "question_binding"}, "manifest fields")
    require(value["schema"] == "qzt-public-history-manifest-v1" and value["question_binding"] == "none; manifest inventory is not an answer oracle", "manifest schema")
    require(re.fullmatch(r"[0-9a-f]{32}", value["container_id"]), "container id")
    docs = value["documents"]; require(isinstance(docs, list) and 1 <= len(docs) <= 20, "document count")
    by_id = {}; offset = 0
    for doc in docs:
        require(isinstance(doc, dict) and set(doc) == {"doc_id", "logical_offset", "byte_length", "checksum", "source"}, "document fields")
        require(isinstance(doc["doc_id"], str) and doc["doc_id"] not in by_id, "document id")
        require(type(doc["logical_offset"]) is int and doc["logical_offset"] == offset, "document offset")
        require(type(doc["byte_length"]) is int and doc["byte_length"] > 0, "document length")
        checksum = doc["checksum"]
        require(isinstance(checksum, dict) and set(checksum) == {"algorithm", "value"} and checksum["algorithm"] == "sha256"
                and re.fullmatch(r"[0-9a-f]{64}", checksum["value"]), "document checksum")
        require(isinstance(doc["source"], dict) and doc["source"].get("url"), "document provenance")
        by_id[doc["doc_id"]] = doc; offset += doc["byte_length"]
    require(value["source"].get("source_sha") == "3d092d1db64fdd3f0744377df98257038eddd32f", "source binding")
    return by_id


def search_status(report):
    require(report.get("capped") is False and report.get("stop_reason") is None
            and report.get("incomplete_reason") is None and report.get("index_coverage_verified") is False,
            "search status is capped, partial, unknown, or coverage was asserted")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--output", default="report.json")
    parser.add_argument("--log", default="commands.json")
    args = parser.parse_args()
    output = Path(args.output); log = Path(args.log)
    runner = None
    try:
        require(output.name == args.output and log.name == args.log and output != log, "output/log must be basenames")
        require(not output.exists() and not output.is_symlink() and not log.exists() and not log.is_symlink(), "fresh output/log required")
        require(re.fullmatch(r"[0-9a-f]{64}", args.manifest_sha256), "manifest SHA required")
        manifest_bytes = read_small(Path("manifest.json")); require(sha(manifest_bytes) == args.manifest_sha256, "manifest SHA mismatch")
        manifest = json.loads(manifest_bytes); docs = validate_manifest(manifest)
        questions = json.loads(read_small(Path("questions.json"))); validate_questions(questions)
        require(Path("summary.txt").is_file() and not Path("summary.txt").is_symlink(), "summary required")
        require(Path("evidence.qzt").is_file() and not Path("evidence.qzt").is_symlink(), "QZT required")
        binary = Path("qzt.exe" if os.name == "nt" else "qzt")
        require(binary.is_file() and not binary.is_symlink(), "recipient binary required")
        runner = Runner(binary)
        info = runner.json("info", "evidence.qzt", "--format", "json")
        require(info.get("container_id") == manifest["container_id"] and info.get("document_index") is True, "container binding")
        stored = runner.json("docs", "evidence.qzt", "--format", "json")["documents"]
        require([(x["doc_id"], x["logical_offset"], x["byte_length"]) for x in stored]
                == [(x["doc_id"], x["logical_offset"], x["byte_length"]) for x in docs.values()], "Document Index mismatch")
        sidecar = Path("evidence.qzi")
        if not sidecar.exists():
            require(not sidecar.is_symlink(), "sidecar symlink refused")
            runner.call("sidecar-rebuild", "evidence.qzt", "--index", "ngram", "--ngram", 3, "-o", "evidence.qzi")
        require(sidecar.is_file() and not sidecar.is_symlink(), "sidecar required")
        inspected = runner.json("inspect-sidecar", "evidence.qzt", "--sidecar", "evidence.qzi", "--format", "json")
        require(inspected.get("index_type") == "ngram" and inspected.get("complete") is True, "invalid sidecar inspection")
        report = {"schema": "qzt-public-history-report-v1", "container_id": manifest["container_id"],
                  "deep_verify_note": "deep verify reads the whole container; search/context precede hit-document restoration", "questions": []}
        cached = {}
        for question in questions["questions"]:
            entry = {"id": question["id"], "question": question["question"], "queries": question["queries"], "hits": []}
            for query in question["queries"]:
                result = runner.json("search", "evidence.qzt", query, "--sidecar", "evidence.qzi", "--max-results", 100, "--format", "json")
                search_status(result)
                query_hits = []
                for hit in result["hits"]:
                    require(hit.get("source") == "verified_original_bytes" and type(hit.get("logical_offset")) is int
                            and type(hit.get("byte_length")) is int and hit["byte_length"] == len(query.encode()), "invalid search hit")
                    hit_start = hit["logical_offset"]; hit_end = hit_start + hit["byte_length"]
                    doc = next((d for d in docs.values() if d["logical_offset"] <= hit_start and hit_end <= d["logical_offset"] + d["byte_length"]), None)
                    require(doc is not None, "hit outside Document Index")
                    if doc["doc_id"] not in cached:
                        data = runner.call("doc", "evidence.qzt", doc["doc_id"])
                        require(len(data) == doc["byte_length"] and sha(data) == doc["checksum"]["value"], "document checksum mismatch")
                        cached[doc["doc_id"]] = data
                    data = cached[doc["doc_id"]]; local = hit_start - doc["logical_offset"]
                    require(data[local:local + hit["byte_length"]] == query.encode(), "query bytes mismatch")
                    context = runner.json("context", "evidence.qzt", "--offset", hit_start, "--length", hit["byte_length"], "--before", 2, "--after", 2, "--format", "json")
                    require(context.get("mapping_status") == "unique" and context["document"]["id"] == doc["doc_id"], "context mapping mismatch")
                    excerpt = context["excerpt"]; start = excerpt["logical_offset"] - doc["logical_offset"]
                    require(bytes.fromhex(excerpt["bytes_hex"]) == data[start:start + excerpt["byte_length"]], "context bytes mismatch")
                    require(context["before"]["stop"] in ("complete", "scope_boundary") and context["after"]["stop"] in ("complete", "scope_boundary"), "partial context")
                    query_hits.append({"query": query, "hit": hit, "doc_id": doc["doc_id"], "context": context, "document_checksum_verified": True})
                entry["hits"].extend(query_hits)
            entry["candidate_status"] = "hits_returned" if entry["hits"] else "no_hit_record_insufficient"
            report["questions"].append(entry)
        verified = runner.json("verify", "evidence.qzt", "--deep", "--format", "json")
        require(verified.get("ok") is True and verified.get("original_checksum_verified") is True and verified.get("document_index_status") == "source_checked", "deep verification incomplete")
        report["deep_verify"] = verified
        write_new(log, runner.records); write_new(output, report)
        return 0
    except (RuntimeError, OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        if runner is not None and not log.exists():
            write_new(log, runner.records)
        print(f"Error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
