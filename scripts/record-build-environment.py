#!/usr/bin/env python3
"""Record the toolchain selected immediately around a cargo-dist build."""

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path


BUILD_FLAGS = (
    "RUSTUP_TOOLCHAIN", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET",
    "CARGO_PROFILE_DIST_DEBUG", "CARGO_PROFILE_DIST_LTO", "CARGO_PROFILE_DIST_OPT_LEVEL",
    "CARGO_PROFILE_DIST_CODEGEN_UNITS", "CC", "CXX", "CFLAGS", "CXXFLAGS",
    "MACOSX_DEPLOYMENT_TARGET", "SDKROOT",
)


def run(*command):
    return subprocess.check_output(command, text=True).strip()


def snapshot(source_sha, target, profile, features, build_command):
    if not re.fullmatch(r"[0-9a-f]{40}", source_sha):
        raise RuntimeError("source SHA must be full")
    if run("git", "rev-parse", "HEAD") != source_sha:
        raise RuntimeError("build checkout differs from source SHA")
    if run("git", "status", "--porcelain"):
        raise RuntimeError("build checkout is dirty")
    if not os.environ.get("RUSTUP_TOOLCHAIN"):
        raise RuntimeError("RUSTUP_TOOLCHAIN must pin the selected build toolchain")
    if run("dist", "--version") != "cargo-dist 0.31.0":
        raise RuntimeError("unexpected cargo-dist version")
    return {
        "schema": "qzt-build-environment-v1",
        "source_sha": source_sha,
        "target": target,
        "build_profile": profile,
        "features": features,
        "build_command": build_command,
        "rustc": run("rustc", "--version", "--verbose"),
        "cargo": run("cargo", "--version", "--verbose"),
        "rustup_toolchain": run("rustup", "show", "active-toolchain"),
        "rustup_rustc": run("rustup", "which", "rustc"),
        "rustup_cargo": run("rustup", "which", "cargo"),
        "rustc_path": shutil.which("rustc"),
        "cargo_path": shutil.which("cargo"),
        "cargo_dist": "cargo-dist 0.31.0",
        "build_flags": {name: os.environ.get(name) for name in BUILD_FLAGS},
        "runner": {
            "name": os.environ.get("RUNNER_NAME", platform.node()),
            "os": os.environ.get("RUNNER_OS", platform.system()),
            "architecture": os.environ.get("RUNNER_ARCH", platform.machine()),
            "platform": platform.platform(),
        },
        "github_run": {
            "repository": os.environ.get("GITHUB_REPOSITORY"),
            "run_id": os.environ.get("GITHUB_RUN_ID"),
            "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
            "job": os.environ.get("GITHUB_JOB"),
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="mode", required=True)
    record = subcommands.add_parser("record")
    record.add_argument("--source-sha", required=True)
    record.add_argument("--target", required=True)
    record.add_argument("--profile", required=True)
    record.add_argument("--features", required=True)
    record.add_argument("--build-command", required=True)
    record.add_argument("--output", required=True, type=Path)
    verify = subcommands.add_parser("verify")
    verify.add_argument("--input", required=True, type=Path)
    args = parser.parse_args()

    if args.mode == "record":
        evidence = snapshot(args.source_sha, args.target, args.profile, args.features,
                            args.build_command)
        evidence["recorded_at_utc"] = datetime.now(timezone.utc).isoformat()
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n",
                               encoding="utf-8")
    else:
        saved = json.loads(args.input.read_text(encoding="utf-8"))
        current = snapshot(saved["source_sha"], saved["target"], saved["build_profile"],
                           saved["features"], saved["build_command"])
        if any(saved.get(key) != value for key, value in current.items()):
            raise RuntimeError("build toolchain, flags, source, or runner changed during dist build")
        print(f"build environment unchanged: {saved['source_sha']} {saved['target']}")


if __name__ == "__main__":
    main()
