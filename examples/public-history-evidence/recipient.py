#!/usr/bin/env python3
"""Source-blind search and byte-correspondence report for one frozen handoff."""
import argparse
import hashlib
import json
import math
import os
import re
import subprocess
import sys
import time
from pathlib import Path

CAP = 2 * 1024 * 1024
REPORT_FIELDS = {"schema", "container_id", "manifest_sha256", "questions_sha256", "deep_verify_note", "deep_verify", "verified_documents", "questions"}
SEARCH_FIELDS = {"hits", "metrics", "capped", "stop_reason", "index_complete_declared", "index_coverage_verified", "incomplete_reason"}
SEARCH_METRIC_FIELDS = {"query", "index_kind", "posting_granularity", "index_size_bytes", "source_size_bytes", "index_size_ratio", "term_lookups", "posting_bytes_read", "candidate_granules", "candidate_chunks", "decoded_bytes", "physical_decoded_bytes", "physical_decoded_chunks", "verified_matches", "query_time_ms"}
SEARCH_HIT_FIELDS = {"logical_offset", "byte_length", "chunk_start", "chunk_end", "source"}
VERIFY_FIELDS = {"ok", "level", "checked_chunks", "compressed_checksum_chunks", "decoded_chunks", "decoded_bytes", "original_checksum_verified", "container_checksum_status", "dense_line_index_status", "document_index_status"}
INFO_FIELDS = {"format", "profile", "original_size", "compressed_size", "chunk_count", "line_count", "newline_mode", "zstd_level", "target_chunk_size", "max_chunk_size", "original_checksum", "dense_line_index", "document_index", "document_count", "container_id"}
SIDECAR_FIELDS = {"index_type", "ngram_n", "complete", "high_df_per_million", "source_size_bytes", "index_size_bytes", "granule_count", "term_count", "postings_size_bytes"}
KNOWN_CAP_REASONS = {"max_candidate_granules", "max_decoded_bytes", "max_physical_decoded_bytes", "max_physical_decoded_chunks", "max_search_results"}
KNOWN_INCOMPLETE_REASONS = {"query_shorter_than_ngram_n", "query_has_no_indexable_tokens", "missing_required_key_in_incomplete_index"}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def exact(value, keys, label):
    require(isinstance(value, dict) and set(value) == set(keys), label)


def read_small(path, limit=CAP):
    require(path.is_file() and not path.is_symlink(), f"regular file required: {path}")
    with path.open("rb") as handle:
        data = handle.read(limit + 1)
    require(len(data) <= limit, f"input cap exceeded: {path}")
    return data


def write_new(path, value):
    require(not path.exists() and not path.is_symlink(), f"existing output protected: {path}")
    with path.open("xb") as handle:
        handle.write((json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode())


class Runner:
    def __init__(self, binary):
        self.binary = binary.resolve()
        self.records = []

    def input_hashes(self, command):
        inputs = {}
        for value in command:
            path = Path(value)
            if not path.is_absolute():
                path = Path.cwd() / path
            if path.is_file() and not path.is_symlink():
                inputs[str(path.resolve())] = sha(path.read_bytes())
        return inputs

    def call(self, *args):
        command = [str(self.binary), *map(str, args)]
        input_sha256 = self.input_hashes(command)
        started = time.perf_counter()
        try:
            result = subprocess.run(command, cwd=Path.cwd(), capture_output=True, timeout=30, check=False)
        except subprocess.TimeoutExpired as error:
            self.records.append({"command": command, "input_sha256": input_sha256, "timeout": True,
                                 "stdout_sha256": sha(error.stdout or b""), "stderr_sha256": sha(error.stderr or b"")})
            raise RuntimeError("CLI timeout") from error
        self.records.append({"command": command, "input_sha256": input_sha256, "exit_code": result.returncode,
                             "wall_ms": round((time.perf_counter() - started) * 1000, 3),
                             "stdout_sha256": sha(result.stdout), "stderr_sha256": sha(result.stderr)})
        require(result.returncode == 0, f"CLI failed: {args[0]} exit={result.returncode}")
        return result.stdout

    def json(self, *args):
        return json.loads(self.call(*args))


def validate_questions(value):
    exact(value, ["schema", "questions"], "question schema")
    require(value["schema"] == "qzt-public-history-questions-v1" and isinstance(value["questions"], list) and len(value["questions"]) == 7, "seven questions")
    ids = []
    for question in value["questions"]:
        exact(question, ["id", "question", "queries"], "question fields")
        require(isinstance(question["id"], str) and isinstance(question["question"], str) and question["question"], "question identity")
        require(isinstance(question["queries"], list) and question["queries"] and all(isinstance(query, str) and len(query.encode()) >= 3 for query in question["queries"]), "query text")
        ids.append(question["id"])
    require(len(set(ids)) == 7, "unique question ids")


def validate_deep_verify(value):
    exact(value, VERIFY_FIELDS, "deep verify schema")
    integer_fields = {"checked_chunks", "compressed_checksum_chunks", "decoded_chunks", "decoded_bytes"}
    require(all(type(value[name]) is int and value[name] >= 0 for name in integer_fields), "deep verify metrics")
    require(value["ok"] is True and value["level"] == "deep" and value["original_checksum_verified"] is True and value["container_checksum_status"] == "verified" and value["document_index_status"] == "source_checked", "deep verification incomplete")


def validate_info(value, manifest):
    exact(value, INFO_FIELDS, "info schema")
    require(value["container_id"] == manifest["container_id"] and value["document_index"] is True and type(value["document_count"]) is int and value["document_count"] == len(manifest["documents"]), "container binding")


def validate_sidecar(value, source_size):
    exact(value, SIDECAR_FIELDS, "sidecar schema")
    integer_fields = SIDECAR_FIELDS - {"index_type", "complete"}
    require(all(type(value[name]) is int and value[name] >= 0 for name in integer_fields) and type(value["complete"]) is bool, "sidecar types")
    require(value["index_type"] == "ngram" and value["ngram_n"] == 3 and value["source_size_bytes"] == source_size, "invalid sidecar")


def validate_manifest(value, qbytes, binary, helper):
    exact(value, ["schema", "container_id", "documents", "selection", "source", "runtime_inventory", "question_binding"], "manifest fields")
    require(value["schema"] == "qzt-public-history-manifest-v2" and re.fullmatch(r"[0-9a-f]{32}", value["container_id"]), "manifest identity")
    exact(value["source"], ["product_source_sha_self_reported", "archive_commit_sha", "snapshot_sha256", "policy_sha256"], "source fields")
    exact(value["runtime_inventory"], ["questions_sha256", "binary_sha256", "recipient_sha256"], "runtime inventory")
    require(value["runtime_inventory"] == {"questions_sha256": sha(qbytes), "binary_sha256": sha(binary), "recipient_sha256": sha(helper)}, "runtime inventory mismatch")
    require(value["question_binding"] == "none; runtime inventory is not an answer oracle", "answer binding")
    docs, offset = {}, 0
    for doc in value["documents"]:
        exact(doc, ["doc_id", "logical_offset", "byte_length", "checksum", "source"], "document fields")
        exact(doc["checksum"], ["algorithm", "value"], "checksum fields")
        require(isinstance(doc["doc_id"], str) and doc["doc_id"] not in docs and type(doc["logical_offset"]) is int and doc["logical_offset"] == offset and type(doc["byte_length"]) is int and doc["byte_length"] > 0, "document coordinates")
        require(doc["checksum"]["algorithm"] == "sha256" and re.fullmatch(r"[0-9a-f]{64}", doc["checksum"]["value"]), "document checksum")
        require(isinstance(doc["source"], dict) and isinstance(doc["source"].get("object_url"), str), "document provenance")
        docs[doc["doc_id"]], offset = doc, offset + doc["byte_length"]
    require(1 <= len(docs) <= 20, "document count")
    return docs


def search_status(value, query):
    exact(value, SEARCH_FIELDS, "search schema")
    require(isinstance(value["hits"], list) and type(value["capped"]) is bool and type(value["index_complete_declared"]) is bool and value["index_coverage_verified"] is False, "search types")
    metrics = value["metrics"]
    exact(metrics, SEARCH_METRIC_FIELDS, "search metrics schema")
    require(metrics["query"] == query and metrics["index_kind"] == "ngram" and metrics["posting_granularity"] == "line", "search metrics identity")
    integer_metrics = SEARCH_METRIC_FIELDS - {"query", "index_kind", "posting_granularity", "index_size_ratio", "query_time_ms"}
    require(all(type(metrics[name]) is int and metrics[name] >= 0 for name in integer_metrics), "search integer metrics")
    require(all(type(metrics[name]) in (int, float) and math.isfinite(metrics[name]) and metrics[name] >= 0 for name in ("index_size_ratio", "query_time_ms")), "search float metrics")
    for hit in value["hits"]:
        exact(hit, SEARCH_HIT_FIELDS, "search hit schema")
        require(type(hit["logical_offset"]) is int and hit["logical_offset"] >= 0 and type(hit["byte_length"]) is int and hit["byte_length"] > 0, "search hit range")
        require(type(hit["chunk_start"]) is int and type(hit["chunk_end"]) is int and 0 <= hit["chunk_start"] < hit["chunk_end"] and hit["source"] == "verified_original_bytes", "search hit source")
    require(metrics["verified_matches"] == len(value["hits"]), "search verified hit count")
    reason = value["incomplete_reason"]
    stop = value["stop_reason"]
    require(reason is None or reason in KNOWN_INCOMPLETE_REASONS, "unknown incomplete reason")
    require(stop is None or stop in KNOWN_CAP_REASONS, "unknown stop reason")
    require(value["capped"] == (stop is not None), "capped/stop mismatch")
    if value["capped"] or stop is not None or reason is not None or value["index_complete_declared"] is False:
        return {"kind": "partial_unknown", "reason": reason or stop or "index_complete_declared=false", "query": query, "raw_metadata": value}
    return None


def validate_context(context, doc, start, end, data):
    exact(context, ["hit", "mapping_status", "scope", "document", "candidates", "excerpt", "before", "after", "verification"], "context schema")
    exact(context["hit"], ["logical_offset", "byte_length", "end"], "context hit schema")
    require(context["hit"] == {"logical_offset": start, "byte_length": end - start, "end": end}, "context hit binding")
    exact(context["scope"], ["kind", "logical_offset", "end"], "context scope schema")
    doc_end = doc["logical_offset"] + doc["byte_length"]
    require(context["scope"] == {"kind": "document", "logical_offset": doc["logical_offset"], "end": doc_end}, "context scope")
    exact(context["document"], ["id", "logical_offset", "byte_length", "local_offset", "local_end"], "context document schema")
    require(context["mapping_status"] == "unique" and context["document"] == {"id": doc["doc_id"], "logical_offset": doc["logical_offset"], "byte_length": doc["byte_length"], "local_offset": start - doc["logical_offset"], "local_end": end - doc["logical_offset"]}, "context mapping")
    require(isinstance(context["candidates"], list) and len(context["candidates"]) == 1, "context candidates")
    exact(context["candidates"][0], ["id", "logical_offset", "end"], "context candidate schema")
    require(context["candidates"][0] == {"id": doc["doc_id"], "logical_offset": doc["logical_offset"], "end": doc_end}, "context candidate binding")
    excerpt = context["excerpt"]
    exact(excerpt, ["logical_offset", "byte_length", "bytes_hex", "text_escaped", "leading_fragment", "trailing_fragment"], "excerpt schema")
    require(type(excerpt["logical_offset"]) is int and type(excerpt["byte_length"]) is int and excerpt["byte_length"] > 0 and isinstance(excerpt["bytes_hex"], str) and isinstance(excerpt["text_escaped"], str) and type(excerpt["leading_fragment"]) is bool and type(excerpt["trailing_fragment"]) is bool and len(excerpt["bytes_hex"]) == 2 * excerpt["byte_length"], "excerpt types")
    local = excerpt["logical_offset"] - doc["logical_offset"]
    require(0 <= local <= start - doc["logical_offset"] and end - doc["logical_offset"] <= local + excerpt["byte_length"] <= len(data), "context range")
    try:
        excerpt_bytes = bytes.fromhex(excerpt["bytes_hex"])
        visible_text = excerpt_bytes.decode().replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t")
    except ValueError as error:
        raise RuntimeError("context bytes encoding") from error
    require(excerpt_bytes == data[local:local + excerpt["byte_length"]], "context bytes")
    require(excerpt["text_escaped"] == visible_text, "context displayed text")
    hit_in_excerpt = start - excerpt["logical_offset"]
    require(excerpt_bytes[hit_in_excerpt:hit_in_excerpt + end - start] == data[start - doc["logical_offset"]:end - doc["logical_offset"]], "context hit bytes")
    partial = []
    for side in ("before", "after"):
        value = context[side]
        exact(value, ["requested", "returned", "stop"], f"context {side} schema")
        require(type(value["requested"]) is int and type(value["returned"]) is int and 0 <= value["returned"] <= value["requested"] and value["stop"] in ("complete", "scope_boundary", "budget"), f"context {side} values")
        if value["stop"] == "budget":
            partial.append(f"{side}_budget")
    require(not excerpt["leading_fragment"] or context["before"]["stop"] == "budget", "leading fragment without budget")
    require(not excerpt["trailing_fragment"] or context["after"]["stop"] == "budget", "trailing fragment without budget")
    if excerpt["leading_fragment"]:
        partial.append("leading_fragment")
    if excerpt["trailing_fragment"]:
        partial.append("trailing_fragment")
    verification = context["verification"]
    exact(verification, ["decoded_chunks_verified", "document_index_block_verified", "document_checksum_verified", "search_query_verified", "external_provenance_verified"], "context verification schema")
    require(verification == {"decoded_chunks_verified": True, "document_index_block_verified": True, "document_checksum_verified": False, "search_query_verified": False, "external_provenance_verified": False}, "context verification")
    return partial


def validate_report(report, manifest, manifest_sha256, questions):
    exact(report, REPORT_FIELDS, "report schema")
    require(report["schema"] == "qzt-public-history-report-v4" and report["container_id"] == manifest["container_id"] and report["manifest_sha256"] == manifest_sha256, "report binding")
    require(report["questions_sha256"] == sha(questions), "report questions binding")
    require(report["deep_verify_note"] == "deep verify and index construction read the whole container; only hit documents are restored afterwards", "deep verify disclosure")
    validate_deep_verify(report["deep_verify"])
    expected_questions = json.loads(questions)["questions"]
    require(isinstance(report["questions"], list) and len(report["questions"]) == len(expected_questions), "report question count")
    result = {}
    docs = {doc["doc_id"]: doc for doc in manifest["documents"]}
    verified_documents = {}
    require(isinstance(report["verified_documents"], list), "verified documents")
    for verified in report["verified_documents"]:
        exact(verified, ["doc_id", "byte_length", "sha256", "bytes_hex"], "verified document schema")
        doc = docs.get(verified["doc_id"])
        require(doc is not None and verified["doc_id"] not in verified_documents, "verified document identity")
        require(type(verified["byte_length"]) is int and verified["byte_length"] == doc["byte_length"] and verified["sha256"] == doc["checksum"]["value"] and isinstance(verified["bytes_hex"], str) and len(verified["bytes_hex"]) == 2 * verified["byte_length"], "verified document metadata")
        try:
            data = bytes.fromhex(verified["bytes_hex"])
        except ValueError as error:
            raise RuntimeError("verified document bytes encoding") from error
        require(len(data) == verified["byte_length"] and sha(data) == verified["sha256"], "verified document bytes")
        verified_documents[verified["doc_id"]] = data
    referenced_documents = set()
    for expected, entry in zip(expected_questions, report["questions"]):
        exact(entry, ["id", "question", "queries", "searches", "hits", "candidate_status"], "report question fields")
        require(entry["id"] == expected["id"] and entry["id"] not in result and entry["question"] == expected["question"] and entry["queries"] == expected["queries"], "report question binding")
        require(isinstance(entry["searches"], list) and len(entry["searches"]) == len(entry["queries"]), "report search count")
        raw_hits = []
        partial = False
        for query, item in zip(entry["queries"], entry["searches"]):
            exact(item, ["query", "classification", "reason", "raw_metadata"], "report search fields")
            require(item["query"] == query and isinstance(item["raw_metadata"], dict), "report search query")
            status = search_status(item["raw_metadata"], query)
            classification = "partial_unknown" if status else "complete"
            reason = status["reason"] if status else None
            require(item["classification"] == classification and item["reason"] == reason, "report search classification")
            partial = partial or status is not None
            raw_hits.extend((query, hit) for hit in item["raw_metadata"]["hits"])
        require(isinstance(entry["hits"], list) and len(entry["hits"]) == len(raw_hits), "report hit count")
        candidate_hits = []
        for hit in entry["hits"]:
            exact(hit, ["query", "hit", "doc_id", "source_url", "document_sha256", "global_range", "local_range", "context", "document_checksum_verified", "coverage_verified"], "report hit fields")
            require(isinstance(hit["query"], str), "report hit query")
            exact(hit["hit"], SEARCH_HIT_FIELDS, "report hit metadata")
            candidate_hits.append((hit["query"], hit["hit"]))
            doc = docs.get(hit["doc_id"])
            require(doc is not None and hit["source_url"] == doc["source"]["object_url"] and hit["document_sha256"] == doc["checksum"]["value"] and hit["document_checksum_verified"] is True and hit["coverage_verified"] is False, "report hit source")
            require(hit["doc_id"] in verified_documents, "missing verified hit document")
            referenced_documents.add(hit["doc_id"])
            require(type(hit["global_range"]) is list and type(hit["local_range"]) is list and len(hit["global_range"]) == len(hit["local_range"]) == 2, "report ranges")
            start, end = hit["global_range"]
            require(type(start) is int and type(end) is int and doc["logical_offset"] <= start < end <= doc["logical_offset"] + doc["byte_length"] and hit["local_range"] == [start - doc["logical_offset"], end - doc["logical_offset"]], "report range binding")
            require(hit["hit"] == {"logical_offset": start, "byte_length": end - start, "chunk_start": hit["hit"]["chunk_start"], "chunk_end": hit["hit"]["chunk_end"], "source": "verified_original_bytes"}, "report hit range")
            data = verified_documents[hit["doc_id"]]
            require(data[hit["local_range"][0]:hit["local_range"][1]] == hit["query"].encode(), "report query bytes")
            partial = bool(validate_context(hit["context"], doc, start, end, data)) or partial
        require(candidate_hits == raw_hits, "raw/candidate hit mismatch")
        require(entry["candidate_status"] == ("partial_unknown" if partial else "hits_returned" if entry["hits"] else "no_hit_record_insufficient"), "report candidate status")
        result[entry["id"]] = entry
    require(referenced_documents == set(verified_documents), "unreferenced verified document")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--output", default="report.json")
    parser.add_argument("--log", default="commands.json")
    args = parser.parse_args()
    output, log, runner = Path(args.output), Path(args.log), None
    try:
        require(output.name == args.output and log.name == args.log and output != log, "output/log must be distinct basenames")
        require(not output.exists() and not output.is_symlink() and not log.exists() and not log.is_symlink(), "existing output/log protected")
        require(re.fullmatch(r"[0-9a-f]{64}", args.manifest_sha256), "manifest SHA required")
        manifest_bytes, questions = read_small(Path("manifest.json")), read_small(Path("questions.json"))
        require(sha(manifest_bytes) == args.manifest_sha256, "manifest external seal mismatch")
        validate_questions(json.loads(questions))
        binary, helper = Path("qzt.exe" if os.name == "nt" else "qzt"), read_small(Path("recipient.py"))
        require(binary.is_file() and not binary.is_symlink(), "recipient binary required")
        manifest, docs = json.loads(manifest_bytes), validate_manifest(json.loads(manifest_bytes), questions, binary.read_bytes(), helper)
        require(Path("summary.txt").is_file() and not Path("summary.txt").is_symlink() and Path("evidence.qzt").is_file() and not Path("evidence.qzt").is_symlink(), "handoff input missing")
        runner = Runner(binary)
        verified = runner.json("verify", "evidence.qzt", "--deep", "--format", "json")
        validate_deep_verify(verified)
        info = runner.json("info", "evidence.qzt", "--format", "json")
        validate_info(info, manifest)
        stored = runner.json("docs", "evidence.qzt", "--format", "json")["documents"]
        require([(x["doc_id"], x["logical_offset"], x["byte_length"]) for x in stored] == [(x["doc_id"], x["logical_offset"], x["byte_length"]) for x in docs.values()], "Document Index mismatch")
        sidecar = Path("evidence.qzi")
        require(not sidecar.is_symlink(), "sidecar symlink refused")
        if not sidecar.exists():
            runner.call("sidecar-rebuild", "evidence.qzt", "--index", "ngram", "--ngram", 3, "-o", "evidence.qzi")
        inspected = runner.json("inspect-sidecar", "evidence.qzt", "--sidecar", "evidence.qzi", "--format", "json")
        validate_sidecar(inspected, sum(doc["byte_length"] for doc in docs.values()))
        report = {"schema": "qzt-public-history-report-v4", "container_id": info["container_id"], "manifest_sha256": args.manifest_sha256, "questions_sha256": sha(questions), "deep_verify_note": "deep verify and index construction read the whole container; only hit documents are restored afterwards", "deep_verify": verified, "verified_documents": [], "questions": []}
        cached = {}
        for question in json.loads(questions)["questions"]:
            entry = {"id": question["id"], "question": question["question"], "queries": question["queries"], "searches": [], "hits": []}
            context_partial = False
            for query in question["queries"]:
                result = runner.json("search", "evidence.qzt", query, "--sidecar", "evidence.qzi", "--max-results", 100, "--format", "json")
                partial = search_status(result, query)
                entry["searches"].append({"query": query, "classification": "partial_unknown" if partial else "complete", "reason": None if not partial else partial["reason"], "raw_metadata": result})
                for hit in result["hits"]:
                    start, end = hit["logical_offset"], hit["logical_offset"] + hit["byte_length"]
                    context = runner.json("context", "evidence.qzt", "--offset", start, "--length", hit["byte_length"], "--before", 12, "--after", 12, "--format", "json")
                    require(context.get("mapping_status") == "unique" and isinstance(context.get("document"), dict), "context did not resolve one document")
                    doc = docs.get(context["document"].get("id"))
                    require(doc is not None, "context document absent from manifest")
                    if doc["doc_id"] not in cached:
                        cached[doc["doc_id"]] = runner.call("doc", "evidence.qzt", doc["doc_id"])
                        require(len(cached[doc["doc_id"]]) == doc["byte_length"] and sha(cached[doc["doc_id"]]) == doc["checksum"]["value"], "hit document checksum")
                    data = cached[doc["doc_id"]]
                    require(data[start - doc["logical_offset"]:end - doc["logical_offset"]] == query.encode(), "query bytes mismatch")
                    context_partial = bool(validate_context(context, doc, start, end, data)) or context_partial
                    entry["hits"].append({"query": query, "hit": hit, "doc_id": doc["doc_id"], "source_url": doc["source"]["object_url"], "document_sha256": doc["checksum"]["value"], "global_range": [start, end], "local_range": [start - doc["logical_offset"], end - doc["logical_offset"]], "context": context, "document_checksum_verified": True, "coverage_verified": False})
            entry["candidate_status"] = "partial_unknown" if context_partial or any(s["classification"] == "partial_unknown" for s in entry["searches"]) else "hits_returned" if entry["hits"] else "no_hit_record_insufficient"
            report["questions"].append(entry)
        report["verified_documents"] = [{"doc_id": doc_id, "byte_length": len(data), "sha256": sha(data), "bytes_hex": data.hex()} for doc_id, data in cached.items()]
        validate_report(report, manifest, args.manifest_sha256, questions)
        write_new(log, runner.records)
        write_new(output, report)
        return 0
    except (RuntimeError, OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        if runner is not None and not log.exists():
            write_new(log, runner.records)
        print(f"Error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
