#!/usr/bin/env python3
"""Check the unpublished release build files after the host cleanup boundary."""

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path


TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
)
ARCHIVES = {f"qzt-{target}" + (".zip" if "windows" in target else ".tar.xz")
            for target in TARGETS}
GLOBAL = {"qzt-installer.sh", "qzt-installer.ps1", "source.tar.gz",
          "source.tar.gz.sha256", "sha256.sum"}
ASSETS = ARCHIVES | {f"{name}.sha256" for name in ARCHIVES} | GLOBAL
TAG = "v0.1.0-pre.5"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def inspect(artifacts, staged, plan, source_sha):
    require(re.fullmatch(r"[0-9a-f]{40}", source_sha), "source SHA must be full")
    require(subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip() == source_sha,
            "assembly checkout differs from selected source")
    planned = json.loads(plan.read_text(encoding="utf-8"))
    require(planned["announcement_tag"] == TAG and
            planned["announcement_is_prerelease"] is True and
            planned["dist_version"] == "0.31.0" and
            set(planned["artifacts"]) == ASSETS, "release plan differs from the expected 13 build assets")

    internals = {f"{target}-{kind}.json" for target in (*TARGETS, "global")
                 for kind in ("dist-manifest", "build-environment")}
    internals.add("plan-dist-manifest.json")
    files = {path.name for path in artifacts.iterdir() if path.is_file()}
    require(files == ASSETS | internals and len(list(artifacts.iterdir())) == len(files),
            "uploaded build artifact set is incomplete or contains unexpected files")
    staged_files = {path.name for path in staged.iterdir() if path.is_file()}
    require(staged_files == ASSETS and len(list(staged.iterdir())) == len(staged_files),
            "host cleanup did not leave exactly the planned build assets")

    environments = {}
    manifests = {}
    for target in (*TARGETS, "global"):
        environment = json.loads((artifacts / f"{target}-build-environment.json").read_text())
        require(environment["schema"] == "qzt-build-environment-v1" and
                environment["source_sha"] == source_sha and
                environment["target"] == target and
                environment["cargo_dist"] == "cargo-dist 0.31.0" and
                environment["github_run"]["run_id"] and
                environment["github_run"]["run_attempt"],
                f"wrong build provenance for {target}")
        environments[target] = {"sha256": digest(artifacts / f"{target}-build-environment.json"),
                                "record": environment}
        manifest_path = artifacts / f"{target}-dist-manifest.json"
        manifest = json.loads(manifest_path.read_text())
        expected = ASSETS if target == "global" else {
            name for name in ASSETS if name.startswith(f"qzt-{target}.")}
        require(manifest["announcement_tag"] == TAG and
                manifest["announcement_is_prerelease"] is True and
                set(manifest["artifacts"]) == expected,
                f"wrong build manifest for {target}")
        manifests[target] = {"sha256": digest(manifest_path), "artifacts": sorted(expected)}

    assets = {name: {"sha256": digest(staged / name), "size": (staged / name).stat().st_size}
              for name in sorted(ASSETS)}
    for name in ARCHIVES | {"source.tar.gz"}:
        words = (staged / f"{name}.sha256").read_text(encoding="ascii").split()
        require(len(words) == 2 and words[0] == assets[name]["sha256"] and
                words[1].lstrip("*") == name, f"wrong checksum sidecar for {name}")
    aggregate = {}
    for line in (staged / "sha256.sum").read_text(encoding="ascii").splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})\s+\*?([A-Za-z0-9_.-]+)", line)
        require(match is not None, "invalid aggregate checksum line")
        value, name = match.groups()
        require(name in ARCHIVES | {"source.tar.gz"} and name not in aggregate and
                value == assets[name]["sha256"], f"wrong aggregate checksum for {name}")
        aggregate[name] = value
    require(set(aggregate) == ARCHIVES | {"source.tar.gz"},
            "aggregate checksum does not cover every archive")
    return {"source_sha": source_sha, "tag": TAG, "build_environments": environments,
            "build_manifests": manifests, "staged_assets": assets,
            "aggregate_checksum_entries": sorted(aggregate),
            "planned_public_asset_names": sorted(ASSETS | {"dist-manifest.json"}),
            "host_generated_dist_manifest": "not built; host/publish not run"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--staged", type=Path, required=True)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = inspect(args.artifacts, args.staged, args.plan, args.source_sha)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n",
                           encoding="utf-8")
    print(f"staged {len(result['staged_assets'])} build assets; host not run")


if __name__ == "__main__":
    main()
