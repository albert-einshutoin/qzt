#!/usr/bin/env python3
"""Run the published pre.5 CLI over fixed CI logs and replay its handoff."""

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tarfile
import time
from pathlib import Path


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


class Runner:
    def __init__(self, binary: Path, cwd: Path):
        self.binary = binary.resolve()
        self.cwd = cwd
        self.records: list[dict] = []

    def call(self, label: str, *args: str) -> bytes:
        command = [str(self.binary), *map(str, args)]
        started = time.perf_counter()
        result = subprocess.run(command, cwd=self.cwd, capture_output=True, check=False)
        record = {"label": label, "command": [self.binary.name, *map(str, args)],
                  "exit_code": result.returncode, "wall_ms": round((time.perf_counter() - started) * 1000, 3),
                  "stdout_sha256": digest(result.stdout), "stderr_utf8": result.stderr.decode("utf-8", "replace")}
        self.records.append(record)
        require(result.returncode == 0, f"{label} failed: {record}")
        return result.stdout


def check_binary(binary: Path, source: dict) -> None:
    require(digest(binary.read_bytes()) == source["published_cli"]["binary_sha256"], "wrong published binary")
    result = subprocess.run([str(binary), "--version"], capture_output=True, check=True)
    require(result.stdout.strip() == b"qzt 0.1.0-pre.5", "wrong CLI version")


def check_search(report: dict, expected: list[dict], capped: bool = False) -> None:
    require(report["index_complete_declared"] is True, "QZI did not declare complete")
    require(report["index_coverage_verified"] is False, "unexpected coverage claim")
    require(report["incomplete_reason"] is None, "incomplete query")
    if capped:
        require(report["capped"] is True and report["stop_reason"] == "max_search_results", "cap not reported")
        require(len(report["hits"]) == 1, "unexpected capped hit count")
        require(all((h["logical_offset"], h["byte_length"]) in
                    {(e["logical_offset"], e["byte_length"]) for e in expected}
                    for h in report["hits"]), "capped hit is not in oracle")
    else:
        require(report["capped"] is False and report["stop_reason"] is None, "ordinary query was capped")
        actual = sorted((h["logical_offset"], h["byte_length"]) for h in report["hits"])
        wanted = sorted((h["logical_offset"], h["byte_length"]) for h in expected)
        require(actual == wanted, f"search positions differ: {actual} != {wanted}")
    require(all(h["source"] == "verified_original_bytes" for h in report["hits"]), "unverified hit")


def search_and_ranges(runner: Runner, queries: dict, raw: Path, recipient: bool) -> None:
    for index, entry in enumerate(queries["queries"]):
        label = f"query-{index}"
        output = runner.call(label, "search", "evidence.qzt", entry["query"], "--sidecar", "evidence.qzi",
                             "--max-results", "100", "--max-candidates", "10000",
                             "--max-decoded-bytes", "16MiB", "--max-physical-decoded-bytes", "16MiB",
                             "--max-physical-decoded-chunks", "100", "--format", "json")
        (raw / f"{label}.json").write_bytes(output)
        report = json.loads(output)
        check_search(report, entry["hits"])
        for hit_index, hit in enumerate(entry["hits"]):
            start = hit["logical_offset"] - hit["doc_offset"] + hit["context_doc_offset"]
            end = start + hit["context_byte_length"]
            context = runner.call(f"range-{index}-{hit_index}", "range", "evidence.qzt", "--bytes", f"{start}:{end}")
            require(context == hit["context_utf8"].encode("utf-8"), "range differs from independent oracle")
            (raw / f"range-{index}-{hit_index}.txt").write_bytes(context)
    if not recipient:
        entry = queries["queries"][2]
        output = runner.call("capped-query", "search", "evidence.qzt", entry["query"],
                             "--sidecar", "evidence.qzi", "--max-results", "1", "--format", "json")
        (raw / "capped-query.json").write_bytes(output)
        check_search(json.loads(output), entry["hits"], capped=True)


def restore_documents(runner: Runner, source: dict, raw: Path, originals: Path | None) -> None:
    documents = json.loads(runner.call("docs", "docs", "evidence.qzt", "--format", "json"))
    write_json(raw / "docs.json", documents)
    actual = {entry["doc_id"]: entry for entry in documents["documents"]}
    expected_start = 0
    for item in source["input_order"]:
        entry = actual[item["doc_id"]]
        require(entry["logical_offset"] == expected_start and entry["byte_length"] == item["bytes"],
                "Document Index differs from source-length offsets")
        expected_start += item["bytes"]
        restored = runner.call("doc-" + item["file"], "doc", "evidence.qzt", item["doc_id"])
        require(len(restored) == item["bytes"] and digest(restored) == item["sha256"], "document hash differs")
        if originals is not None:
            require(restored == (originals / item["file"]).read_bytes(), "document differs from original")
        (raw / ("restored-" + item["file"])).write_bytes(restored)
    require(len(actual) == len(source["input_order"]), "unexpected document")


def verify_tar(archive: Path, source: dict, originals: Path) -> None:
    with tarfile.open(archive, "r:gz") as handle:
        members = handle.getmembers()
        require([member.name for member in members] == [item["file"] for item in source["input_order"]],
                "tar members differ")
        for member, item in zip(members, source["input_order"]):
            extracted = handle.extractfile(member).read()
            require(extracted == (originals / item["file"]).read_bytes(), "tar extraction differs")


def produce(binary: Path, work: Path) -> None:
    base = Path(__file__).resolve().parent
    source = json.loads((base / "source.json").read_text())
    queries = json.loads((base / "queries.json").read_text())
    check_binary(binary, source)
    require(not work.exists(), "work directory already exists")
    work.mkdir(parents=True)
    bundle = work / "bundle"
    raw = work / "raw"
    bundle.mkdir()
    raw.mkdir()
    for name in ("source.json", "queries.json"):
        shutil.copy2(base / name, bundle / name)
    originals = base / "inputs"
    expected = b""
    for item in source["input_order"]:
        data = (originals / item["file"]).read_bytes()
        require(len(data) == item["bytes"] and digest(data) == item["sha256"], "input differs from manifest")
        data.decode("utf-8")
        expected += data
    # The oracle is fixed before pack and is checked from source bytes alone.
    from oracle import build
    require(build(source, base) == queries, "fixed oracle differs from original logs")
    runner = Runner(binary, bundle)
    staged = work / "inputs"
    staged.mkdir()
    paths = []
    for item in source["input_order"]:
        (staged / item["file"]).symlink_to((originals / item["file"]).resolve())
        paths.append("../inputs/" + item["file"])
    runner.call("pack-docs", "pack-docs", *paths, "--doc-id-prefix", "ci/", "-o", "evidence.qzt")
    verified = json.loads(runner.call("deep-verify", "verify", "evidence.qzt", "--deep", "--format", "json"))
    write_json(raw / "deep-verify.json", verified)
    require(verified["ok"] and verified["original_checksum_verified"] and
            verified["document_index_status"] == "source_checked", "deep verify incomplete")
    attestation = runner.call("attest", "attest", "evidence.qzt", "--level", "deep")
    (bundle / "evidence.attest.json").write_bytes(attestation)
    require(json.loads(attestation)["attestation_schema"] == "qzt-attestation-v1", "attestation schema")
    restore_documents(runner, source, raw, originals)
    exported = runner.call("export", "export", "evidence.qzt")
    require(exported == expected, "whole export differs from ordered concatenation")
    write_json(raw / "export.json", {"bytes": len(exported), "sha256": digest(exported), "byte_equal": True})
    runner.call("sidecar-rebuild", "sidecar-rebuild", "evidence.qzt", "--index", "ngram", "--ngram", "3", "-o", "evidence.qzi")
    inspect = json.loads(runner.call("inspect-sidecar", "inspect-sidecar", "evidence.qzt", "--sidecar", "evidence.qzi", "--format", "json"))
    write_json(raw / "inspect-sidecar.json", inspect)
    require(inspect["index_type"] == "ngram" and inspect["ngram_n"] == 3, "wrong QZI")
    search_and_ranges(runner, queries, raw, False)
    archive = work / "plain.tar.gz"
    start = time.perf_counter()
    with tarfile.open(archive, "w:gz") as handle:
        for item in source["input_order"]:
            handle.add(originals / item["file"], arcname=item["file"])
    tar_ms = round((time.perf_counter() - start) * 1000, 3)
    verify_tar(archive, source, originals)
    sizes = {"source_manifest": (base / "source.json").stat().st_size,
             "query_manifest_extra": (base / "queries.json").stat().st_size,
             "logs": sum(item["bytes"] for item in source["input_order"]),
             "tar_gz": archive.stat().st_size, "qzt": (bundle / "evidence.qzt").stat().st_size,
             "attestation": (bundle / "evidence.attest.json").stat().st_size,
             "qzi": (bundle / "evidence.qzi").stat().st_size}
    sizes.update({"plain_total": sizes["logs"] + sizes["source_manifest"],
                  "tar_total": sizes["tar_gz"] + sizes["source_manifest"],
                  "qzt_total": sizes["qzt"] + sizes["attestation"] + sizes["source_manifest"],
                  "qzt_qzi_total": sizes["qzt"] + sizes["attestation"] + sizes["source_manifest"] + sizes["qzi"]})
    write_json(raw / "measurements.json", {"environment": "macOS arm64, Apple M4, one process at a time",
               "published_binary_sha256": source["published_cli"]["binary_sha256"], "sizes_bytes": sizes,
               "tar_create_wall_ms": tar_ms, "commands": runner.records})
    print("producer: PASS", json.dumps(sizes))


def receive(binary: Path, bundle: Path) -> None:
    source = json.loads((bundle / "source.json").read_text())
    queries = json.loads((bundle / "queries.json").read_text())
    check_binary(binary, source)
    require(not (bundle / "evidence.qzi").exists(), "recipient must rebuild QZI")
    require(all(not (bundle / item["file"]).exists() for item in source["input_order"]),
            "recipient must not use original logs")
    raw = bundle / "received-record"
    require(not raw.exists(), "received-record already exists")
    raw.mkdir()
    runner = Runner(binary, bundle)
    verified = json.loads(runner.call("deep-verify", "verify", "evidence.qzt", "--deep", "--format", "json"))
    write_json(raw / "deep-verify.json", verified)
    require(verified["ok"] and verified["original_checksum_verified"] and
            verified["document_index_status"] == "source_checked", "receiver deep verify incomplete")
    regenerated = runner.call("attest", "attest", "evidence.qzt", "--level", "deep")
    require(regenerated == (bundle / "evidence.attest.json").read_bytes(), "attestation bytes differ")
    restore_documents(runner, source, raw, None)
    runner.call("sidecar-rebuild", "sidecar-rebuild", "evidence.qzt", "--index", "ngram", "--ngram", "3", "-o", "evidence.qzi")
    search_and_ranges(runner, queries, raw, True)
    write_json(raw / "measurements.json", {"attestation_byte_equal": True, "commands": runner.records})
    print("recipient: PASS")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("produce", "receive"))
    parser.add_argument("--binary", type=Path, required=True, help="extracted published pre.5 executable")
    parser.add_argument("--work", type=Path, help="new producer work directory")
    parser.add_argument("--bundle", type=Path, help="recipient directory containing the handoff")
    args = parser.parse_args()
    if args.mode == "produce":
        require(args.work is not None and args.bundle is None, "provide --work only")
        produce(args.binary.resolve(), args.work.resolve())
    else:
        require(args.bundle is not None and args.work is None, "provide --bundle only")
        receive(args.binary.resolve(), args.bundle.resolve())


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, RuntimeError, AssertionError) as error:
        print(f"case study failed: {error}", file=sys.stderr)
        raise SystemExit(1)
