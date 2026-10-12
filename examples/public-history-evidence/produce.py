#!/usr/bin/env python3
"""Freeze the approved snapshot, then create a source-blind six-file handoff."""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

PRODUCT_SOURCE = "c11b7eb9a025f75d84ea9d3029a0674a606c04c6"
ARCHIVE_COMMIT = "3d092d1db64fdd3f0744377df98257038eddd32f"
CAP = 2 * 1024 * 1024
def sha(data):
    return hashlib.sha256(data).hexdigest()


def require(ok, text):
    if not ok:
        raise RuntimeError(text)


def safe_snapshot_read(root, relative, limit=CAP):
    """Read one snapshot member without following a supplied path or parent symlink."""
    require(isinstance(relative, str), "snapshot path must be text")
    candidate = Path(relative)
    require(not candidate.is_absolute() and ".." not in candidate.parts, "snapshot path escapes root")
    root = root.resolve(strict=True)
    path = root / candidate
    current = root
    for part in candidate.parts:
        current = current / part
        require(not current.is_symlink(), f"snapshot symlink refused: {relative}")
    require(path.resolve(strict=True).is_relative_to(root), "snapshot resolved outside root")
    require(path.is_file() and not path.is_symlink(), f"regular snapshot file required: {relative}")
    with path.open("rb") as f: data = f.read(limit + 1)
    require(len(data) <= limit, f"input cap exceeded: {relative}")
    return data

def read(path, limit=CAP):
    require(path.is_file() and not path.is_symlink(), f"regular file required: {path}")
    with path.open("rb") as f: data = f.read(limit + 1)
    require(len(data) <= limit, f"input cap exceeded: {path}")
    return data
def read_binary(path):
    require(path.is_file() and not path.is_symlink(), f"regular binary required: {path}")
    return path.read_bytes()
def git_blob_sha(data): return hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
def write_new(path, data):
    require(not path.exists() and not path.is_symlink(), f"existing output protected: {path}")
    with path.open("xb") as f: f.write(data)

class Runner:
    def __init__(self, binary, cwd):
        self.binary = binary.resolve()
        self.cwd = cwd
        self.records = []

    def input_hashes(self, command):
        inputs = {}
        for value in command:
            path = Path(value)
            if not path.is_absolute():
                path = self.cwd / path
            if path.is_file() and not path.is_symlink():
                inputs[str(path.resolve())] = sha(path.read_bytes())
        return inputs

    def call(self, *args):
        command = [str(self.binary), *map(str, args)]
        input_sha256 = self.input_hashes(command)
        started = time.perf_counter()
        try:
            result = subprocess.run(command, cwd=self.cwd, capture_output=True, timeout=30, check=False)
        except subprocess.TimeoutExpired as error:
            self.records.append({"command": command, "input_sha256": input_sha256, "timeout": True, "stdout_sha256": sha(error.stdout or b""), "stderr_sha256": sha(error.stderr or b"")})
            raise RuntimeError("CLI timeout") from error
        self.records.append({"command": command, "input_sha256": input_sha256, "exit_code": result.returncode, "wall_ms": round((time.perf_counter() - started) * 1000, 3), "stdout_sha256": sha(result.stdout), "stderr_sha256": sha(result.stderr)})
        require(result.returncode == 0, f"CLI failed: {args[0]} exit={result.returncode}")
        return result.stdout

    def json(self, *args):
        return json.loads(self.call(*args))

def body_objects(record, raw, extracted):
    value = json.loads(raw)
    values = value if isinstance(value, list) else [value]
    require(isinstance(values, list) and all(isinstance(x, dict) for x in values), "snapshot API schema")
    bodies = []
    for index, item in enumerate(values):
        body = item.get("body")
        require(isinstance(body, str), "body must be text")
        pointer = "/body" if not isinstance(value, list) else f"/{index}/body"
        bodies.append((item, body.encode(), pointer))
    aggregate = b"".join(data for _, data, _ in bodies)
    require(aggregate == extracted, "raw/body extraction bytes differ")
    require(sha(aggregate) == record["body"]["sha256"] and len(aggregate) == record["body"]["size"], "API body extraction mismatch")
    return bodies

def snapshot_docs(snapshot):
    import acquire

    index = json.loads(safe_snapshot_read(snapshot, "snapshot.json"))
    policy = json.loads(safe_snapshot_read(snapshot, "acquisition-policy.json"))
    require(index.get("schema") == "qzt-public-history-snapshot-v1" and policy.get("schema") == "qzt-public-history-acquisition-v1", "snapshot schema")
    require(set(policy) == {"schema", "urls", "limits", "saved_before_get", "network"} and policy["urls"] == acquire.URLS and policy["limits"] == acquire.LIMITS and policy["network"] == {"method": "GET", "credentials": False} and isinstance(policy["saved_before_get"], str), "approved acquisition policy")
    records = index.get("records")
    require(isinstance(records, list) and len(records) == len(acquire.URLS) and [record.get("url") for record in records] == acquire.URLS, "approved snapshot records")
    docs, excluded, commit = [], [], None
    response_total = body_total = 0
    for record in records:
        require(isinstance(record, dict) and record.get("status") == 200 and isinstance(record.get("url"), str) and isinstance(record.get("raw"), dict) and isinstance(record.get("body"), dict), "snapshot record schema")
        require(set(record) == acquire.RECORD_FIELDS, "snapshot record fields")
        require(set(record["raw"]) == {"path", "size", "sha256"} and set(record["body"]) == {"path", "size", "sha256", "json_pointer"}, "snapshot file fields")
        raw = safe_snapshot_read(snapshot, record["raw"]["path"])
        require(len(raw) == record["raw"]["size"] and sha(raw) == record["raw"]["sha256"], "raw snapshot mismatch")
        body = safe_snapshot_read(snapshot, record["body"]["path"])
        require(len(body) == record["body"]["size"] and sha(body) == record["body"]["sha256"], "body snapshot mismatch")
        response_total += len(raw)
        body_total += len(body)
        require(len(raw) <= acquire.LIMITS["response_bytes"] and response_total <= acquire.LIMITS["total_response_bytes"], "response cap")
        url = record["url"]
        if "/commits/" in url:
            commit = json.loads(raw)
            require(commit.get("sha") == ARCHIVE_COMMIT, "archive commit mismatch")
            continue
        if url.startswith("https://api.github.com/"):
            value = json.loads(raw)
            require(len(value) <= acquire.LIMITS["collection_items"] if isinstance(value, list) else True, "collection cap")
            require("next" not in record.get("headers", {}).get("Link", ""), "pagination rejected")
            for obj, data, pointer in body_objects(record, raw, body):
                require(type(obj.get("id")) is int and isinstance(obj.get("node_id"), str) and isinstance(obj.get("html_url"), str), "API object identity")
                if not data:
                    excluded.append({"id": obj.get("id"), "node_id": obj.get("node_id"), "json_pointer": pointer, "reason": "empty body excluded"})
                    continue
                if "/issues/344" in url and "comments" not in url:
                    stem = "issue-344-body"
                elif "/issues/344/comments" in url:
                    stem = f"issue-344-comment-{obj['id']}"
                elif "/pulls/345" in url and "comments" not in url and "reviews" not in url:
                    stem = "pr-345-body"
                elif "/pulls/345/comments" in url:
                    stem = f"pr-345-comment-{obj['id']}"
                elif "/pulls/345/reviews" in url:
                    stem = f"pr-345-review-{obj['id']}"
                elif "/issues/345/comments" in url:
                    stem = f"issue-345-comment-{obj['id']}"
                else:
                    continue
                submitted = obj.get("submitted_at", "unavailable") if "/reviews" in url else obj.get("created_at", "unavailable")
                docs.append((stem + ".txt", data, {"acquisition_url": url, "object_url": obj.get("html_url"), "object_id": obj.get("id"), "node_id": obj.get("node_id"), "updated_at": obj.get("updated_at") or "unknown", "submitted_at": submitted or "unknown", "fetched_at": record["fetched_at"], "raw_response": record["raw"], "body_json_pointer": pointer, "body_size": len(data), "body_sha256": sha(data)}))
    require(commit is not None, "commit record missing")
    blobs = {f["filename"]: f["sha"] for f in commit.get("files", [])}
    for record in records:
        if not record["url"].startswith("https://raw.githubusercontent.com/"):
            continue
        data = safe_snapshot_read(snapshot, record["body"]["path"])
        require(data == safe_snapshot_read(snapshot, record["raw"]["path"]) and sha(data) == record["body"]["sha256"], "fixed file raw/body mismatch")
        name = record["url"].rsplit("/", 1)[-1]
        repo_path = "docs/benchmarks/" + name
        require(git_blob_sha(data) == blobs.get(repo_path), "git blob framing mismatch")
        docs.append(("commit-" + name, data, {"acquisition_url": record["url"], "object_url": f"https://github.com/albert-einshutoin/qzt/blob/{ARCHIVE_COMMIT}/{repo_path}", "object_id": ARCHIVE_COMMIT, "updated_at": "unknown", "submitted_at": "unknown", "fetched_at": record["fetched_at"], "commit_committed_at": commit["commit"]["committer"]["date"], "path": repo_path, "blob_sha": blobs[repo_path], "raw_response": record["raw"], "body_json_pointer": None, "body_size": len(data), "body_sha256": sha(data)}))
    require(response_total == index.get("total_response_bytes") and body_total == index.get("total_body_bytes") and body_total <= acquire.LIMITS["body_bytes"], "declared snapshot totals")
    require(1 <= len(docs) <= acquire.LIMITS["documents"] and sum(len(x[1]) for x in docs) <= acquire.LIMITS["body_bytes"], "document cap")
    return docs, excluded, index, policy

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--snapshot", required=True, type=Path)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    require(re.fullmatch(r"[0-9a-f]{40}", args.source_sha), "source SHA must be 40 lowercase hex")
    require(not args.output.exists() and not args.output.is_symlink(), "fresh handoff required")
    receipt = args.output.with_name(args.output.name + "-receipt.json")
    log = args.output.with_name(args.output.name + "-producer.jsonl")
    require(not receipt.exists() and not receipt.is_symlink() and not log.exists() and not log.is_symlink(), "existing producer receipt/log protected")
    snapshot = args.snapshot.resolve()
    docs, excluded, index, policy = snapshot_docs(snapshot)
    questions_path = Path(__file__).with_name("questions.json")
    evaluation_path = Path(__file__).with_name("evaluation.json")
    recipient_path = Path(__file__).with_name("recipient.py")
    questions_bytes = read(questions_path)
    evaluation_bytes = read(evaluation_path)
    recipient_bytes = read(recipient_path)
    binary = read_binary(args.binary)
    questions = json.loads(questions_bytes)
    evaluation = json.loads(evaluation_bytes)
    require(questions.get("schema") == "qzt-public-history-questions-v1" and len(questions.get("questions", [])) == 7, "questions")
    require(evaluation.get("schema") == "qzt-public-history-evaluation-v1" and set(evaluation.get("questions", {})) == {x["id"] for x in questions["questions"]}, "evaluation")
    source_by_id = {"github/" + name: data for name, data, _ in docs}
    for criterion in evaluation["questions"].values():
        require(isinstance(criterion.get("required"), list), "evaluation requirements")
        for required in criterion["required"]:
            require(required.get("doc_id") in source_by_id and required.get("fragment", "").encode() in source_by_id[required["doc_id"]], "evaluation fragment absent from frozen source")
    # Freeze all inputs before pack; this receipt is deliberately outside the six-file handoff.
    frozen = {
        "schema": "qzt-public-history-producer-receipt-v3",
        "stage": "pre_pack",
        "product_source_sha_self_reported": args.source_sha,
        "archive_commit_sha": ARCHIVE_COMMIT,
        "snapshot_sha256": sha(safe_snapshot_read(snapshot, "snapshot.json")),
        "policy_sha256": sha(safe_snapshot_read(snapshot, "acquisition-policy.json")),
        "binary_sha256": sha(binary),
        "producer_sha256": sha(read(Path(__file__))),
        "helper_sha256": sha(recipient_bytes),
        "questions_sha256": sha(questions_bytes),
        "evaluation_sha256": sha(evaluation_bytes),
        "documents": [{"filename": name, "size": len(data), "sha256": sha(data), "source": source} for name, data, source in docs],
        "acquisition_policy": policy,
        "command_policy": {"timeout_seconds": 30, "failure": "record command then fail closed"},
    }
    write_new(receipt, (json.dumps(frozen, ensure_ascii=False, indent=2) + "\n").encode())
    runner = None
    try:
        with tempfile.TemporaryDirectory(prefix="qzt-public-history-stage-") as temporary:
            stage = Path(temporary)
            staged = []
            manifest_docs = []
            offset = 0
            for name, data, source in docs:
                path = stage / name
                write_new(path, data)
                staged.append(path)
                manifest_docs.append({"doc_id": "github/" + name, "logical_offset": offset, "byte_length": len(data), "checksum": {"algorithm": "sha256", "value": sha(data)}, "source": source})
                offset += len(data)
            args.output.mkdir()
            runner = Runner(args.binary, args.output.resolve())
            runner.call("pack-docs", *[path.resolve() for path in staged], "--doc-id-prefix", "github/", "-o", "evidence.qzt")
            info = runner.json("info", "evidence.qzt", "--format", "json")
            require(info.get("document_index") is True, "document index missing")
            manifest = {
                "schema": "qzt-public-history-manifest-v2",
                "container_id": info["container_id"],
                "documents": manifest_docs,
                "selection": {"rule": "all non-empty approved API bodies and both fixed commit files", "excluded": excluded},
                "source": {"product_source_sha_self_reported": args.source_sha, "archive_commit_sha": ARCHIVE_COMMIT, "snapshot_sha256": frozen["snapshot_sha256"], "policy_sha256": frozen["policy_sha256"]},
                "runtime_inventory": {"questions_sha256": sha(questions_bytes), "binary_sha256": sha(binary), "recipient_sha256": sha(recipient_bytes)},
                "question_binding": "none; runtime inventory is not an answer oracle",
            }
            require(runner.call("export", "evidence.qzt") == b"".join(item[1] for item in docs), "prepack/export bytes differ")
            write_new(args.output / "manifest.json", (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode())
            write_new(args.output / "summary.txt", "公開snapshotから固定した非権威的要約。質問への正解や文書対応は含まない。\n".encode())
            write_new(args.output / "questions.json", questions_bytes)
            write_new(args.output / "recipient.py", recipient_bytes)
            executable = args.output / ("qzt.exe" if os.name == "nt" else "qzt")
            write_new(executable, binary)
            os.chmod(executable, args.binary.stat().st_mode & 0o777)
            require(sha(read_binary(executable)) == sha(binary), "binary changed during producer")
            result = {"event": "producer_complete", "pre_pack_receipt_sha256": sha(read(receipt)), "manifest_sha256": sha(read(args.output / "manifest.json")), "records": runner.records}
            write_new(log, (json.dumps(result, ensure_ascii=False) + "\n").encode())
            print(result["manifest_sha256"])
    except Exception:
        if runner is not None and not log.exists():
            write_new(log, (json.dumps({"event": "producer_failed", "records": runner.records}, ensure_ascii=False) + "\n").encode())
        raise


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"Error: {error}", file=sys.stderr)
        sys.exit(1)
