#!/usr/bin/env python3
"""Verify the published pre.3 Release on native runners, without rebuilding it."""

import argparse
import hashlib
import importlib.util
import json
import os
import platform
import re
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path

PRODUCT_SHA = "017d4d19739800773ab6a54adf636ff5a43ec1fc"
RELEASE_RUN = 36237333455
TAG = "v0.1.0-pre.3"
API = "https://api.github.com/repos/albert-einshutoin/qzt"
BASE = f"https://github.com/albert-einshutoin/qzt/releases/download/{TAG}"
TARGETS = {
    "aarch64-apple-darwin": ("Darwin", "arm64"),
    "x86_64-apple-darwin": ("Darwin", "x86_64"),
    "x86_64-unknown-linux-gnu": ("Linux", "x86_64"),
    "x86_64-pc-windows-msvc": ("Windows", "AMD64"),
}
ARCHIVES = {
    target: f"qzt-{target}" + (".zip" if "windows" in target else ".tar.xz")
    for target in TARGETS
}
EXPECTED_ASSETS = set(ARCHIVES.values()) | {
    name + ".sha256" for name in ARCHIVES.values()
} | {
    "qzt-installer.sh", "qzt-installer.ps1", "source.tar.gz",
    "source.tar.gz.sha256", "sha256.sum", "dist-manifest.json",
}

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    "candidate_verifier", Path(__file__).with_name("verify-release-candidate.py")
)
candidate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(candidate)


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def get_json(url):
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "qzt-pre3-verifier"}
    token = os.environ.get("GH_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=30) as response:
        return json.load(response)


def publication():
    tag_ref = get_json(f"{API}/git/ref/tags/{TAG}")
    require(tag_ref["object"]["type"] == "tag", "published tag is not annotated")
    tag = get_json(tag_ref["object"]["url"])
    require(tag["object"]["type"] == "commit" and tag["object"]["sha"] == PRODUCT_SHA,
            "tag points to a different product commit")
    release = get_json(f"{API}/releases/tags/{TAG}")
    require(release["tag_name"] == TAG and release["target_commitish"] == PRODUCT_SHA,
            "Release does not target the product commit")
    require(release["prerelease"] is True and release["draft"] is False,
            "Release is not a published prerelease")
    run = get_json(f"{API}/actions/runs/{RELEASE_RUN}")
    require(run["path"] == ".github/workflows/release.yml" and run["event"] == "push" and
            run["head_sha"] == PRODUCT_SHA and run["head_branch"] == TAG and
            run["conclusion"] == "success", "release workflow did not succeed for the product commit")
    assets = {item["name"]: item for item in release["assets"]}
    require(set(assets) == EXPECTED_ASSETS,
            f"unexpected published assets: missing={sorted(EXPECTED_ASSETS - set(assets))}, "
            f"extra={sorted(set(assets) - EXPECTED_ASSETS)}")
    return release, assets


def download(assets, name, directory):
    asset = assets[name]
    url = f"{BASE}/{name}"
    require(asset["browser_download_url"] == url, f"wrong public URL for {name}")
    destination = directory / name
    request = urllib.request.Request(url, headers={"User-Agent": "qzt-pre3-verifier"})
    with urllib.request.urlopen(request, timeout=120) as response, destination.open("wb") as output:
        require(response.url.startswith("https://"), "asset redirected outside HTTPS")
        while chunk := response.read(1024 * 1024):
            output.write(chunk)
    require(destination.stat().st_size == asset["size"], f"download size mismatch: {name}")
    digest = asset.get("digest", "")
    require(re.fullmatch(r"sha256:[0-9a-f]{64}", digest) is not None,
            f"published asset has no SHA-256 digest: {name}")
    require(candidate.sha256(destination) == digest[7:], f"published asset digest mismatch: {name}")
    return destination


def source_and_checksums(assets, directory):
    records = {}
    for name in EXPECTED_ASSETS:
        path = download(assets, name, directory)
        records[name] = {"size": path.stat().st_size, "sha256": candidate.sha256(path)}
    for archive in (*ARCHIVES.values(), "source.tar.gz"):
        candidate.checksum_matches(directory / archive, directory / (archive + ".sha256"))
    aggregate = (directory / "sha256.sum").read_text(encoding="ascii")
    listed = {}
    for line in aggregate.splitlines():
        if not line:
            continue
        match = re.fullmatch(r"([0-9a-f]{64})\s+\*?([A-Za-z0-9_.-]+)", line)
        require(match is not None, f"invalid aggregate checksum line: {line!r}")
        digest, name = match.groups()
        require(name in records and name not in listed, f"unknown or duplicate aggregate entry: {name}")
        require(records[name]["sha256"] == digest, f"aggregate checksum mismatch: {name}")
        listed[name] = digest
    require(set(listed) == set(ARCHIVES.values()) | {"source.tar.gz"},
            "aggregate checksum must cover exactly the four archives and source.tar.gz")
    manifest = json.loads((directory / "dist-manifest.json").read_text(encoding="utf-8"))
    require(manifest["announcement_tag"] == TAG and manifest["announcement_is_prerelease"] is True
            and manifest["dist_version"] == "0.31.0", "published dist manifest differs from plan")
    require(set(manifest["artifacts"]) == EXPECTED_ASSETS - {"dist-manifest.json"},
            "published dist manifest has missing or unexpected artifacts")
    with tarfile.open(directory / "source.tar.gz", "r:gz") as archive:
        member = archive.extractfile("qzt-0.1.0-pre.3/Cargo.toml")
        require(member is not None and b'version = "0.1.0-pre.3"' in member.read(),
                "published source archive has wrong version")
    for name in ("qzt-installer.sh", "qzt-installer.ps1"):
        content = (directory / name).read_text(encoding="utf-8")
        require(f"/releases/download/{TAG}" in content, f"installer has wrong release URL: {name}")
    return {"assets": records, "aggregate_checksum_entries": sorted(listed)}


def install_and_smoke(assets, directory, work, target, archive_binary_hash, vectors_dir):
    windows = target.endswith("windows-msvc")
    name = "qzt-installer.ps1" if windows else "qzt-installer.sh"
    installer = download(assets, name, directory)
    require(ARCHIVES[target] in installer.read_text(encoding="utf-8"),
            "published installer does not contain the expected target archive")
    install_root = work / "install"
    env = os.environ.copy()
    env["QZT_INSTALL_DIR"] = str(install_root)
    env["QZT_NO_MODIFY_PATH"] = "1"
    env["INSTALLER_NO_MODIFY_PATH"] = "1"
    cmd = (["powershell", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
            "-File", str(installer)] if windows else ["sh", str(installer)])
    result = subprocess.run(cmd, cwd=work, env=env, input=b"", capture_output=True,
                            timeout=180, check=False)
    log = (result.stdout + b"\n" + result.stderr).decode("utf-8", errors="replace")
    require(result.returncode == 0, f"published installer failed: {log[-3000:]}")
    require(target in log, "published installer did not report the expected target")
    binary = (install_root / "bin" / ("qzt.exe" if windows else "qzt")).resolve()
    require(binary.is_file(), f"installer did not place the expected binary: {binary}")
    installed_hash = candidate.sha256(binary)
    require(installed_hash == archive_binary_hash, "installed binary differs from published archive")
    require(candidate.command(binary, work, "--version")[0].strip() == candidate.VERSION.encode(),
            "installed binary reports the wrong version")
    installed_work = work / "installed-smoke"
    installed_work.mkdir()
    installed_smoke = candidate.smoke(binary, installed_work, vectors_dir, target)
    return {
        "installer_url": f"{BASE}/{name}",
        "selected_target": target,
        "selected_archive_url": f"{BASE}/{ARCHIVES[target]}",
        "installed_binary_path": str(binary),
        "installed_binary_sha256": installed_hash,
        "installed_binary_version": candidate.VERSION,
        "installed_smoke": installed_smoke,
        "installer_output": log[-3000:],
    }


def verify_local(assets, target, vectors_dir, directory):
    expected = TARGETS[target]
    actual = (platform.system(), platform.machine())
    require(actual == expected, f"target {target} does not match native runner {actual}")
    archive_name = ARCHIVES[target]
    archive = download(assets, archive_name, directory)
    sidecar = download(assets, archive_name + ".sha256", directory)
    archive_hash = candidate.checksum_matches(archive, sidecar)
    with tempfile.TemporaryDirectory(prefix="qzt-published-pre3-") as temp:
        work = Path(temp)
        binary = candidate.extract_archive(archive, work / "extracted", target)
        linkage = "not applicable outside Linux"
        if target.endswith("linux-gnu"):
            dynamic = subprocess.check_output(["readelf", "-d", str(binary)], text=True)
            needed = set(re.findall(r"\(NEEDED\).*\[(.*?)\]", dynamic))
            require(needed == {"libc.so.6", "libgcc_s.so.1"},
                    f"unexpected Linux dynamic linkage: {sorted(needed)}")
            linkage = sorted(needed)
        smoke_work = work / "archive-smoke"
        smoke_work.mkdir()
        smoke = candidate.smoke(binary, smoke_work, vectors_dir, target)
        binary_hash = candidate.sha256(binary)
        install = install_and_smoke(assets, directory, work, target, binary_hash, vectors_dir)
    return {
        "target": target, "archive_url": f"{BASE}/{archive_name}",
        "sidecar_url": f"{BASE}/{archive_name}.sha256",
        "archive_sha256": archive_hash, "archive_size": archive.stat().st_size,
        "archive_binary_sha256": binary_hash, "archive_binary_smoke": smoke,
        "linux_linkage": linkage, "installer": install,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=("global", "local"))
    parser.add_argument("--target", choices=TARGETS)
    parser.add_argument("--vectors-dir", type=Path)
    parser.add_argument("--verifier-sha", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    require(re.fullmatch(r"[0-9a-f]{40}", args.verifier_sha), "verifier SHA must be full")
    actual_sha = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    require(actual_sha == args.verifier_sha, "verifier checkout is not the recorded commit")
    require(not subprocess.check_output(["git", "status", "--porcelain"]),
            "verifier checkout is not clean")
    release, assets = publication()
    with tempfile.TemporaryDirectory(prefix="qzt-pre3-download-") as temp:
        directory = Path(temp)
        if args.kind == "global":
            outcome = source_and_checksums(assets, directory)
        else:
            require(args.target is not None and args.vectors_dir is not None,
                    "local verification needs target and vectors")
            outcome = verify_local(assets, args.target, args.vectors_dir.resolve(), directory)
    evidence = {
        "kind": args.kind, "product_source_sha": PRODUCT_SHA,
        "release_run_id": RELEASE_RUN, "release_id": release["id"],
        "release_url": release["html_url"], "published_at": release["published_at"],
        "verifier_sha": args.verifier_sha,
        "verifier_runner": os.environ.get("RUNNER_NAME", platform.node()),
        "verifier_os": platform.system(), "verifier_architecture": platform.machine(),
        "verifier_rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        **outcome,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(evidence, sort_keys=True))


if __name__ == "__main__":
    main()
