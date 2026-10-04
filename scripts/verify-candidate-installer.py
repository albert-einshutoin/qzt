#!/usr/bin/env python3
"""Native candidate installer rehearsal using an explicit loopback download URL."""

import argparse
import functools
import http.server
import json
import platform
import subprocess
import tempfile
import threading
from pathlib import Path

# Import the existing installer isolation and C4 dispatcher; no installer rewrite.
import importlib.util
spec = importlib.util.spec_from_file_location(
    "published_verifier", Path(__file__).with_name("verify-published-release.py"))
published = importlib.util.module_from_spec(spec)
spec.loader.exec_module(published)
candidate = published.candidate
require = published.require


def verify(distrib, target, vectors_dir, tag, source_sha):
    require((platform.system(), platform.machine()) == candidate.TARGETS[target], "wrong native target")
    require(subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip() == source_sha,
            "candidate installer source SHA differs")
    require(not subprocess.check_output(["git", "status", "--porcelain"]), "dirty candidate source")
    archive = distrib / published.ARCHIVES[target]
    archive_hash = candidate.checksum_matches(archive, distrib / (archive.name + ".sha256"))
    installer = distrib / ("qzt-installer.ps1" if target.endswith("windows-msvc") else "qzt-installer.sh")
    candidate.check_installer_tag(installer.read_text(encoding="utf-8"), tag)
    installer_hash = candidate.sha256(installer)
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(distrib))
    with tempfile.TemporaryDirectory(prefix="qzt-candidate-install-") as temp, \
            http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler) as server:
        work = Path(temp)
        binary = candidate.extract_archive(archive, work / "extracted", target)
        binary_hash = candidate.sha256(binary)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        base = f"http://127.0.0.1:{server.server_port}"
        try:
            installed = published.run_installer(installer, work, target, binary_hash, vectors_dir,
                                                tag, candidate.expected_version(tag),
                                                "public-workflow-v1", candidate_base=base)
        finally:
            server.shutdown()
            thread.join()
    require(candidate.sha256(installer) == installer_hash and candidate.sha256(archive) == archive_hash,
            "candidate files changed during install")
    return {"source_sha": source_sha, "tag": tag, "target": target,
            "archive_sha256": archive_hash, "binary_sha256": binary_hash,
            "installer_sha256": installer_hash, "download_override": "explicit loopback candidate staging",
            "public_release_urls_tested": False, "installer": installed}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--distrib", type=Path, required=True)
    parser.add_argument("--target", choices=candidate.TARGETS, required=True)
    parser.add_argument("--vectors-dir", type=Path, required=True)
    parser.add_argument("--expected-tag", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    require(not args.output.exists(), "output exists; preserve the previous evidence")
    report = verify(args.distrib.resolve(), args.target, args.vectors_dir.resolve(),
                    args.expected_tag, args.source_sha)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8") as output:
        output.write(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
