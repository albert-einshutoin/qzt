#!/usr/bin/env python3
"""Verify unpublished pre.3 cargo-dist artifacts without using a release URL."""

import argparse
import hashlib
import json
import os
import platform
import re
import stat
import subprocess
import tarfile
import tempfile
import zipfile
from pathlib import Path, PurePosixPath


TAG = "v0.1.0-pre.3"
VERSION = "qzt 0.1.0-pre.3"
TARGETS = {
    "aarch64-apple-darwin": ("Darwin", "arm64"),
    "x86_64-apple-darwin": ("Darwin", "x86_64"),
    "x86_64-unknown-linux-gnu": ("Linux", "x86_64"),
    "x86_64-pc-windows-msvc": ("Windows", "AMD64"),
}
SOURCE = b"alpha\nbeta\nerror gamma\nerror delta\n"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha256(path):
    return hashlib.file_digest(path.open("rb"), "sha256").hexdigest()


def command(binary, cwd, *args, exit_code=0):
    result = subprocess.run(
        [str(binary), *args], cwd=cwd, capture_output=True, timeout=30, check=False
    )
    require(
        result.returncode == exit_code,
        f"{args}: exit {result.returncode}, expected {exit_code}; stderr={result.stderr!r}",
    )
    return result.stdout, result.stderr


def checksum_matches(archive, sidecar):
    fields = sidecar.read_text(encoding="ascii").strip().split(maxsplit=1)
    require(len(fields) == 2, f"malformed checksum sidecar: {sidecar}")
    expected, name = fields
    require(re.fullmatch(r"[0-9a-f]{64}", expected), "invalid SHA-256 sidecar value")
    require(name.lstrip("*") == archive.name, "checksum sidecar names another archive")
    actual = sha256(archive)
    require(actual == expected, f"archive SHA-256 mismatch: {archive}")
    return actual


def check_manifest(path, expected_artifacts):
    manifest = json.loads(path.read_text(encoding="utf-8"))
    require(manifest["announcement_tag"] == TAG, "dist manifest has wrong tag")
    require(manifest["dist_version"] == "0.31.0", "dist manifest has wrong version")
    for name in expected_artifacts:
        require(name in manifest["artifacts"], f"dist manifest omits {name}")
    return manifest


def extract_archive(archive, destination, target):
    root = f"qzt-{target}"

    def safe_name(name):
        parts = PurePosixPath(name).parts
        require(parts and parts[0] == root and ".." not in parts, f"unsafe archive path: {name}")

    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as zipped:
            for member in zipped.infolist():
                safe_name(member.filename)
            zipped.extractall(destination)
    else:
        with tarfile.open(archive, "r:xz") as tarred:
            for member in tarred.getmembers():
                safe_name(member.name)
                require(member.isfile() or member.isdir(), "archive contains a link or special file")
            tarred.extractall(destination, filter="data")

    binary = destination / root / ("qzt.exe" if target.endswith("windows-msvc") else "qzt")
    require(binary.is_file(), f"archive does not contain its expected binary: {binary}")
    return binary.resolve()


def smoke(binary, work, vectors_dir, target):
    require(command(binary, work, "--version")[0].strip() == VERSION.encode(), "wrong binary version")
    (work / "app.log").write_bytes(SOURCE)
    command(binary, work, "pack", "app.log", "-o", "app.qzt")
    container = work / "app.qzt"
    container_bytes = container.read_bytes()

    info = json.loads(command(binary, work, "info", "app.qzt", "--format", "json")[0])
    require((info["format"], info["original_size"], info["line_count"], info["chunk_count"]) ==
            ("qzt-0.1", len(SOURCE), 4, 1), "incorrect packed metadata")
    require(command(binary, work, "range", "app.qzt", "--lines", "2:2")[0] == b"beta\n", "wrong range")

    command(binary, work, "sidecar-rebuild", "app.qzt", "-o", "app.qzi")
    sidecar = json.loads(command(binary, work, "inspect-sidecar", "app.qzt", "--sidecar", "app.qzi", "--format", "json")[0])
    require(sidecar["index_type"] == "token" and sidecar["complete"] and
            sidecar["source_size_bytes"] == len(SOURCE) and sidecar["granule_count"] == 4,
            "incorrect inspected sidecar")
    search_args = ("search", "app.qzt", "error", "--sidecar", "app.qzi")
    full = json.loads(command(binary, work, *search_args, "--format", "json")[0])
    hits = full["hits"]
    require([(hit["logical_offset"], hit["byte_length"], hit["source"])
             for hit in hits] == [(11, 5, "verified_original_bytes"),
                                  (23, 5, "verified_original_bytes")], "incorrect search hits")
    require(all(SOURCE[hit["logical_offset"]:hit["logical_offset"] + hit["byte_length"]] == b"error"
                for hit in hits), "search hit does not identify the original bytes")
    require(full["capped"] is False and full["stop_reason"] is None and
            full["index_complete_declared"] is True and
            full["index_coverage_verified"] is False and full["incomplete_reason"] is None,
            "incorrect full search coverage declaration")
    require(full["metrics"]["physical_decoded_bytes"] == len(SOURCE) and
            full["metrics"]["physical_decoded_chunks"] == 1 and
            full["metrics"]["verified_matches"] == 2, "incorrect physical search work")
    capped = json.loads(command(binary, work, *search_args, "--max-results", "1", "--format", "json")[0])
    require(capped["capped"] is True and capped["stop_reason"] == "max_search_results" and
            len(capped["hits"]) == 1 and capped["hits"][0]["logical_offset"] == 11 and
            capped["index_coverage_verified"] is False, "incorrect verified partial result")
    out, err = command(binary, work, *search_args, "--max-posting-bytes", "0", "--format", "json", exit_code=1)
    require(not out and b"resource limit" in err, "posting hard error became a report")

    verified = json.loads(command(binary, work, "verify", "app.qzt", "--deep", "--format", "json")[0])
    require(verified["ok"] is True and verified["level"] == "deep" and
            verified["checked_chunks"] == verified["compressed_checksum_chunks"] ==
            verified["decoded_chunks"] == 1 and verified["decoded_bytes"] == len(SOURCE) and
            verified["original_checksum_verified"] is True and
            verified["container_checksum_status"] == "verified", "incorrect deep verification")
    attested = command(binary, work, "attest", "app.qzt")[0]
    require(attested == command(binary, work, "attest", "app.qzt")[0], "attestation bytes changed on retry")
    attestation = json.loads(attested)
    require(attestation["attestation_schema"] == "qzt-attestation-v1" and
            attestation["format"] == "qzt-0.1" and
            attestation["verify"]["decoded_bytes"] == len(SOURCE) and
            attestation["verify"]["original_checksum_verified"] is True,
            "incorrect canonical attestation")
    command(binary, work, "export", "app.qzt", "-o", "restored.log")
    require((work / "restored.log").read_bytes() == SOURCE, "export changed original bytes")

    before = {path.name for path in work.iterdir()}
    out, err = command(binary, work, "export", "app.qzt", "-o", "app.qzt", exit_code=1)
    require(not out and b"same file" in err and container.read_bytes() == container_bytes,
            "self-overwrite damaged input or lacked a clear error")
    require({path.name for path in work.iterdir()} == before, "self-overwrite left a temporary file")
    corrupted = container_bytes[:-1]
    (work / "corrupt.qzt").write_bytes(corrupted)
    (work / "existing.log").write_bytes(b"existing-output")
    before = {path.name for path in work.iterdir()}
    out, _ = command(binary, work, "export", "corrupt.qzt", "-o", "existing.log", exit_code=1)
    require(not out and (work / "corrupt.qzt").read_bytes() == corrupted and
            (work / "existing.log").read_bytes() == b"existing-output", "failed export lost data")
    require({path.name for path in work.iterdir()} == before, "failed export left a temporary file")

    for name, expected in (("valid_c1", b"alpha\nbeta\n"), ("valid_crlf", b"a\r\nb\r\n")):
        vector = bytes.fromhex((vectors_dir / f"{name}.qzt.hex").read_text(encoding="ascii").strip())
        (work / f"{name}.qzt").write_bytes(vector)
        report = json.loads(command(binary, work, "verify", f"{name}.qzt", "--deep", "--format", "json")[0])
        require(report["ok"] is True and report["original_checksum_verified"] is True,
                f"valid v0.1 vector rejected: {name}")
        command(binary, work, "export", f"{name}.qzt", "-o", f"{name}.txt")
        require((work / f"{name}.txt").read_bytes() == expected, f"vector bytes changed: {name}")

    readonly = "passed" if target.endswith("windows-msvc") else "not applicable outside Windows"
    if target.endswith("windows-msvc"):
        path = work / "readonly.log"
        path.write_bytes(b"readonly-existing")
        path.chmod(stat.S_IREAD)
        try:
            attributes = path.stat().st_file_attributes
            require(attributes & stat.FILE_ATTRIBUTE_READONLY, "Windows readonly attribute was not set")
            before = {item.name for item in work.iterdir()}
            out, err = command(binary, work, "export", "app.qzt", "-o", "readonly.log", exit_code=1)
            require(not out and b"read-only" in err.lower() and
                    path.read_bytes() == b"readonly-existing" and
                    path.stat().st_file_attributes == attributes,
                    "readonly output was changed or not diagnosed")
            require({item.name for item in work.iterdir()} == before, "readonly rejection created a temporary file")
        finally:
            path.chmod(stat.S_IWRITE | stat.S_IREAD)
        command(binary, work, "export", "app.qzt", "-o", "readonly.log")
        require(path.read_bytes() == SOURCE, "writable replacement failed after readonly rejection")
    else:
        (work / "existing.log").write_bytes(b"replace-me")
        command(binary, work, "export", "app.qzt", "-o", "existing.log")
        require((work / "existing.log").read_bytes() == SOURCE, "writable replacement failed")

    require((work / "app.log").read_bytes() == SOURCE and container.read_bytes() == container_bytes,
            "smoke changed its source input")
    return {"normal_tour": "passed", "result_cap": "passed", "hard_error": "passed",
            "self_overwrite": "passed", "failed_export_preservation": "passed",
            "v0_1_vectors": ["valid_c1", "valid_crlf"], "readonly": readonly}


def local(args):
    expected_os, expected_arch = TARGETS[args.target]
    require((platform.system(), platform.machine()) == (expected_os, expected_arch),
            f"target {args.target} does not match runner {platform.system()}/{platform.machine()}")
    archive = args.archive.resolve()
    require(archive.name == f"qzt-{args.target}" + (".zip" if expected_os == "Windows" else ".tar.xz"),
            "wrong target archive name")
    require(args.checksum.resolve().name == archive.name + ".sha256", "wrong checksum sidecar name")
    digest = checksum_matches(archive, args.checksum)
    check_manifest(args.manifest, (archive.name, args.checksum.name))
    with tempfile.TemporaryDirectory(prefix="qzt-pre3-candidate-") as directory:
        root = Path(directory)
        binary = extract_archive(archive, root / "extracted", args.target)
        linkage = "not applicable outside Linux"
        if expected_os == "Linux":
            dynamic = subprocess.check_output(["readelf", "-d", str(binary)], text=True)
            needed = set(re.findall(r"\(NEEDED\).*\[(.*?)\]", dynamic))
            require(needed == {"libc.so.6", "libgcc_s.so.1"},
                    f"unexpected Linux dynamic linkage: {sorted(needed)}")
            linkage = "libc.so.6 and libgcc_s.so.1 only; no dynamic libzstd"
        work = root / "smoke"
        work.mkdir()
        result = smoke(binary, work, args.vectors_dir.resolve(), args.target)
        binary_digest = sha256(binary)
        binary_size = binary.stat().st_size
    return {"kind": "local", "target": args.target, "archive": archive.name,
            "archive_size": archive.stat().st_size, "archive_sha256": digest,
            "binary_sha256": binary_digest, "binary_size": binary_size,
            "binary_version": VERSION, "linux_linkage": linkage, "smoke": result}


def global_artifacts(args):
    root = args.distrib.resolve()
    names = ("qzt-installer.sh", "qzt-installer.ps1", "source.tar.gz",
             "source.tar.gz.sha256", "sha256.sum")
    check_manifest(args.manifest, names)
    digest = checksum_matches(root / "source.tar.gz", root / "source.tar.gz.sha256")
    require((root / "sha256.sum").read_text(encoding="ascii") ==
            (root / "source.tar.gz.sha256").read_text(encoding="ascii"),
            "unified checksum does not cover the generated source archive")
    for installer in names[:2]:
        content = (root / installer).read_text(encoding="utf-8")
        require(f"/releases/download/{TAG}" in content, f"{installer} targets wrong version")
        require("v0.1.0-pre.2" not in content, f"{installer} contains old release URL")
    shell = (root / "qzt-installer.sh").read_text(encoding="utf-8")
    windows = (root / "qzt-installer.ps1").read_text(encoding="utf-8")
    for target in TARGETS:
        archive = f"qzt-{target}" + (".zip" if "windows" in target else ".tar.xz")
        require(archive in shell, f"shell installer omits {target}")
        if "windows" in target:
            require(archive in windows, "PowerShell installer omits Windows archive")
    with tarfile.open(root / "source.tar.gz", "r:gz") as source:
        member = source.extractfile("qzt-0.1.0-pre.3/Cargo.toml")
        require(member is not None and b'version = "0.1.0-pre.3"' in member.read(),
                "source archive contains the wrong package version")
    return {"kind": "global", "source_sha256": digest,
            "artifacts": {name: {"size": (root / name).stat().st_size,
                                  "sha256": sha256(root / name)} for name in names},
            "installer_download": "not run before publication"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="kind", required=True)
    for kind in ("local", "global"):
        sub = subcommands.add_parser(kind)
        sub.add_argument("--manifest", type=Path, required=True)
        sub.add_argument("--source-sha", required=True)
        sub.add_argument("--output", type=Path, required=True)
        if kind == "local":
            sub.add_argument("--target", choices=TARGETS, required=True)
            sub.add_argument("--archive", type=Path, required=True)
            sub.add_argument("--checksum", type=Path, required=True)
            sub.add_argument("--vectors-dir", type=Path, required=True)
        else:
            sub.add_argument("--distrib", type=Path, required=True)
    args = parser.parse_args()
    require(re.fullmatch(r"[0-9a-f]{40}", args.source_sha), "source SHA must be full and explicit")
    evidence = local(args) if args.kind == "local" else global_artifacts(args)
    evidence.update({"source_sha": args.source_sha, "tag": TAG, "dist_version": "0.31.0",
                     "build_profile": "dist", "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
                     "runner": os.environ.get("RUNNER_NAME", platform.node()),
                     "os": platform.system(), "architecture": platform.machine(),
                     "artifact_retention_days": 14})
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(evidence, sort_keys=True))


if __name__ == "__main__":
    main()
