#!/usr/bin/env python3
"""Run public-workflow-v1 on an explicitly selected, extracted native binary."""

import argparse
import csv
import importlib.util
import json
import platform
import tempfile
from pathlib import Path


spec = importlib.util.spec_from_file_location(
    "published_verifier", Path(__file__).with_name("verify-published-release.py")
)
published = importlib.util.module_from_spec(spec)
spec.loader.exec_module(published)
candidate = published.candidate
require = candidate.require


def json_command(binary, work, *args):
    return json.loads(candidate.command(binary, work, *args, "--format", "json")[0])


def check_context(report, source, mapping, scope, hit):
    hit_start, hit_length = hit["logical_offset"], hit["byte_length"]
    hit_end = hit_start + hit_length
    coordinates = tuple(report["hit"][key] for key in ("logical_offset", "byte_length", "end"))
    require(all(type(value) is int for value in coordinates) and
            coordinates == (hit_start, hit_length, hit_end),
            "incorrect context hit coordinates")
    require(report["mapping_status"] == mapping and report["scope"]["kind"] == scope,
            "incorrect context mapping/scope")
    excerpt = report["excerpt"]
    start, length = excerpt["logical_offset"], excerpt["byte_length"]
    require(type(start) is int and type(length) is int and
            0 <= start <= hit_start < hit_end <= start + length <= len(source) and
            bytes.fromhex(excerpt["bytes_hex"]) == source[start:start + length],
            "context differs from original bytes")
    verification = report["verification"]
    require(verification["decoded_chunks_verified"] is True and
            all(verification[key] is False for key in
                ("document_checksum_verified", "search_query_verified", "external_provenance_verified")),
            "context overclaims verification")


def context_smoke(binary, work):
    search = json_command(binary, work, "search", "app.qzt", "error", "--sidecar", "app.qzi")
    hit = search["hits"][0]
    args = ("--offset", str(hit["logical_offset"]), "--length", str(hit["byte_length"]))
    report = json_command(binary, work, "context", "app.qzt", *args)
    check_context(report, candidate.SOURCE, "no_document_index", "container", hit)
    require(bytes.fromhex(report["excerpt"]["bytes_hex"]) == candidate.SOURCE,
            "context omitted requested source lines")
    clipped = json_command(binary, work, "context", "app.qzt", *args, "--max-scan-bytes", "5")
    check_context(clipped, candidate.SOURCE, "no_document_index", "container", hit)
    require(clipped["before"]["stop"] == clipped["after"]["stop"] == "budget" and
            clipped["excerpt"]["leading_fragment"] is True and
            clipped["excerpt"]["trailing_fragment"] is True and
            bytes.fromhex(clipped["excerpt"]["bytes_hex"]) == b"error",
            "budget fragment was presented as complete context")
    candidate.command(binary, work, "pack-docs", "app.log", "-o", "docs.qzt")
    document = json_command(binary, work, "context", "docs.qzt", *args, "--before", "100", "--after", "100")
    check_context(document, candidate.SOURCE, "unique", "document", hit)
    require(document["document"]["id"] == "app.log" and
            document["before"]["stop"] == document["after"]["stop"] == "scope_boundary",
            "document boundary was presented as requested complete context")
    zero = json_command(binary, work, "search", "app.qzt", "absent", "--sidecar", "app.qzi")
    require(zero["hits"] == [] and zero["capped"] is False and
            zero["stop_reason"] is None and zero["incomplete_reason"] is None and
            zero["index_coverage_verified"] is False, "zero hits overclaim coverage")
    capped = json_command(binary, work, "search", "app.qzt", "error", "--sidecar", "app.qzi", "--max-results", "0")
    require(capped["hits"] == [] and capped["capped"] is True and
            capped["stop_reason"] == "max_search_results" and
            capped["index_coverage_verified"] is False, "capped zero became a negative finding")
    (work / "broken.qzi").write_bytes((work / "app.qzi").read_bytes()[:-1])
    out, err = candidate.command(binary, work, "search", "app.qzt", "error", "--sidecar", "broken.qzi",
                                 "--format", "json", exit_code=1)
    require(not out and err, "corrupt QZI became a successful search report")
    return {"search_to_context": "passed", "document_scope": "passed",
            "budget_fragment": "passed", "zero_and_capped_zero": "passed", "corrupt_qzi": "passed"}


def golden_smoke(binary, work, vectors_dir):
    diagnostics = {
        "invalid_magic": b"invalid magic",
        "invalid_footer_trailer": b"fixed footer trailer is malformed",
        "footer_checksum_mismatch": b"footer payload checksum mismatch",
        "non_canonical_cbor": b"CBOR is not in the deterministic canonical form",
        "compressed_chunk_checksum_mismatch": b"compressed chunk checksum mismatch",
    }
    names = []
    with (vectors_dir / "manifest.tsv").open(encoding="utf-8", newline="") as manifest:
        for row in csv.DictReader(manifest, delimiter="\t"):
            name = row["name"]
            container = work / f"{name}.qzt"
            container.write_bytes(bytes.fromhex((vectors_dir / f"{name}.qzt.hex").read_text()))
            out, err = candidate.command(binary, work, "info", str(container), "--format", "json",
                                         exit_code=0 if row["expect_open"] == "ok" else 1)
            if row["expect_open"] == "err":
                require(not out and diagnostics[row["expect_error"]] in err,
                        f"wrong golden open failure: {name}")
            else:
                out, err = candidate.command(binary, work, "verify", str(container), "--deep",
                                             "--format", "json",
                                             exit_code=0 if row["expect_deep_verify"] == "ok" else 1)
                if row["expect_deep_verify"] == "err":
                    failure = json.loads(out)
                    require(failure["ok"] is False and
                            diagnostics[row["expect_error"]].decode() in failure["error"],
                            f"wrong golden deep failure: {name}")
                else:
                    require(json.loads(out)["ok"] is True and
                            json.loads(out)["original_checksum_verified"] is True,
                            f"golden Deep verify failed: {name}")
                    # The committed kit's escape grammar is the JSON string subset.
                    expected = json.loads('"' + row["expect_export_text"].replace('"', '\\"') + '"').encode()
                    restored = work / f"{name}.txt"
                    candidate.command(binary, work, "export", str(container), "-o", str(restored))
                    require(restored.read_bytes() == expected, f"golden export changed bytes: {name}")
            names.append(name)
    unsupported = bytearray((work / "valid_c1.qzt").read_bytes())
    unsupported[10:12] = (2).to_bytes(2, "little")
    (work / "unsupported.qzt").write_bytes(unsupported)
    out, err = candidate.command(binary, work, "verify", "unsupported.qzt", "--deep",
                                 "--format", "json", exit_code=1)
    failure = json.loads(out)
    require(failure["ok"] is False and "unsupported QZT format version" in failure["error"],
            "unsupported format version became success")
    return {"core_vectors": names, "unsupported_version": "rejected"}


def verify(binary, expected_sha256, expected_tag, target, vectors_dir):
    require((platform.system(), platform.machine()) == candidate.TARGETS[target], "wrong native target")
    require(candidate.sha256(binary) == expected_sha256, "wrong binary SHA-256")
    vectors = {path.name: candidate.sha256(path) for path in
               [vectors_dir / "manifest.tsv", *sorted(vectors_dir.glob("*.qzt.hex"))]}
    with tempfile.TemporaryDirectory(prefix="qzt-public-workflow-") as directory:
        root = Path(directory)
        work = root / "work"
        work.mkdir()
        with published.isolated_binary_env(root):
            legacy = candidate.smoke(binary, work, vectors_dir, target, expected_tag)
            context = context_smoke(binary, work)
            goldens = golden_smoke(binary, work, vectors_dir)
    require(candidate.sha256(binary) == expected_sha256, "binary changed during smoke")
    require(vectors == {name: candidate.sha256(vectors_dir / name) for name in vectors},
            "vectors changed during smoke")
    return {"ok": True, "profile": "public-workflow-v1", "binary_sha256": expected_sha256,
            "binary_version": candidate.expected_version(expected_tag), "target": target,
            "verifier_sha256": candidate.sha256(Path(__file__)),
            "shared_smoke_sha256": candidate.sha256(Path(candidate.__file__)),
            "isolation_verifier_sha256": candidate.sha256(Path(published.__file__)),
            "vector_sha256": vectors, "workflow": legacy, "context": context, "compatibility": goldens}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--expected-tag", required=True)
    parser.add_argument("--target", required=True, choices=candidate.TARGETS)
    parser.add_argument("--vectors-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    require(args.binary.is_absolute(), "binary path must be absolute")
    require(not args.output.exists(), "output already exists; choose a fresh evidence path")
    report = verify(args.binary.resolve(), args.binary_sha256, args.expected_tag,
                    args.target, args.vectors_dir.resolve())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8") as output:
        output.write(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
