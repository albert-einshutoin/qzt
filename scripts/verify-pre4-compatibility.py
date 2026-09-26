#!/usr/bin/env python3
"""Compare published pre.3 and unpublished pre.4 on the same fixed QZT/QZI inputs."""

import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path


SOURCE = b"echo echo echo\r\necho echo\n"
POSITIONS = [0, 5, 10, 16, 21]


def sha256(path):
    return hashlib.file_digest(path.open("rb"), "sha256").hexdigest()


def command(binary, work, *args):
    return subprocess.check_output([str(binary), *args], cwd=work)


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def check_search(binary, work, container, sidecar):
    report = json.loads(command(binary, work, "search", container, "echo",
                                "--sidecar", sidecar, "--format", "json"))
    hits = report["hits"]
    require([hit["logical_offset"] for hit in hits] == POSITIONS and
            all(hit["byte_length"] == 4 and hit["source"] == "verified_original_bytes" and
                SOURCE[hit["logical_offset"]:hit["logical_offset"] + 4] == b"echo"
                for hit in hits), "cross-version search differs from source")


def compare(pre3, candidate, work):
    binaries = {"pre3": pre3, "pre4_candidate": candidate}
    require(command(pre3, work, "--version").strip() == b"qzt 0.1.0-pre.3",
            "pre.3 binary has wrong version")
    require(command(candidate, work, "--version").strip() == b"qzt 0.1.0-pre.4",
            "candidate binary has wrong version")
    (work / "input.log").write_bytes(SOURCE)
    result = {"source_sha256": sha256(work / "input.log"),
              "pre3_binary_sha256": sha256(pre3),
              "candidate_binary_sha256": sha256(candidate),
              "containers": {}, "sidecars": {}}
    for maker, binary in binaries.items():
        container = f"{maker}.qzt"
        command(binary, work, "pack", "input.log", "-o", container)
        container_path = work / container
        attestation_bytes = []
        for reader, reader_binary in binaries.items():
            report = json.loads(command(reader_binary, work, "verify", container,
                                        "--deep", "--format", "json"))
            require(report["ok"] and report["original_checksum_verified"],
                    "cross-version Deep verify failed")
            restored = f"{maker}-{reader}.log"
            command(reader_binary, work, "export", container, "-o", restored)
            require((work / restored).read_bytes() == SOURCE,
                    "cross-version export changed the source")
            attested = command(reader_binary, work, "attest", container)
            require(json.loads(attested)["attestation_schema"] == "qzt-attestation-v1",
                    "cross-version attestation schema changed")
            attestation_bytes.append(attested)
        require(attestation_bytes[0] == attestation_bytes[1],
                "canonical attestation signing bytes changed")
        result["containers"][maker] = {
            "sha256": sha256(container_path), "bytes": container_path.stat().st_size,
            "deep_attestation_sha256": hashlib.sha256(attestation_bytes[0]).hexdigest(),
            "deep_attestation_byte_equal": True,
        }

    for maker, binary in binaries.items():
        for kind in ("token", "ngram"):
            sidecar = f"pre3-{maker}-{kind}.qzi"
            options = ("--ngram", "3") if kind == "ngram" else ()
            command(binary, work, "sidecar-rebuild", "pre3.qzt", "-o", sidecar,
                    "--index", kind, *options)
            path = work / sidecar
            result["sidecars"][f"{maker}-{kind}"] = {
                "sha256": sha256(path), "bytes": path.stat().st_size,
            }
            for reader_binary in binaries.values():
                inspected = json.loads(command(reader_binary, work, "inspect-sidecar",
                                               "pre3.qzt", "--sidecar", sidecar,
                                               "--format", "json"))
                require(inspected["index_type"] == kind and inspected["complete"],
                        "cross-version sidecar inspect failed")
                check_search(reader_binary, work, "pre3.qzt", sidecar)

    for kind in ("token", "ngram"):
        require(result["sidecars"][f"pre3-{kind}"]["sha256"] ==
                result["sidecars"][f"pre4_candidate-{kind}"]["sha256"],
                f"same-QZT {kind} sidecar bytes changed")
    command(candidate, work, "sidecar-rebuild", "pre4_candidate.qzt", "-o",
            "candidate-container-token.qzi", "--index", "token")
    for binary in binaries.values():
        check_search(binary, work, "pre4_candidate.qzt", "candidate-container-token.qzi")
    result["cross_version_deep_verify_export_search"] = "passed"
    result["same_qzt_token_ngram_qzi_byte_equal"] = True
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pre3-bin", required=True, type=Path)
    parser.add_argument("--candidate-bin", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="qzt-pre4-compat-") as directory:
        result = compare(args.pre3_bin.resolve(), args.candidate_bin.resolve(), Path(directory))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
