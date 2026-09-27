"""Version-boundary regressions for the shared release smoke."""

import importlib.util
import hashlib
import io
import json
import os
import sys
import tempfile
import tarfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "candidate_verifier", Path(__file__).with_name("verify-release-candidate.py")
)
candidate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(candidate)


class VersionBoundaryTests(unittest.TestCase):
    def test_global_accepts_cargo_dist_checksum_with_trailing_blank_line(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archives = {"source.tar.gz"} | {
                f"qzt-{target}" + (".zip" if "windows" in target else ".tar.xz")
                for target in candidate.TARGETS}
            for name in archives:
                (root / name).write_bytes(name.encode())
                digest = hashlib.sha256(name.encode()).hexdigest()
                (root / f"{name}.sha256").write_text(f"{digest} *{name}\n")
            checksums = [f"{hashlib.sha256(name.encode()).hexdigest()} *{name}"
                         for name in sorted(archives)]
            (root / "sha256.sum").write_text("\n".join(checksums) + "\n\n")
            native = [name for name in sorted(archives) if name != "source.tar.gz"]
            (root / "qzt-installer.sh").write_text(
                "/releases/download/v0.1.0-pre.5/\n" + "\n".join(native))
            (root / "qzt-installer.ps1").write_text(
                "/releases/download/v0.1.0-pre.5/\n" +
                next(name for name in native if name.endswith(".zip")))
            names = archives | {f"{name}.sha256" for name in archives} | {
                "qzt-installer.sh", "qzt-installer.ps1", "sha256.sum"}
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({"announcement_tag": "v0.1.0-pre.5",
                                            "announcement_is_prerelease": True,
                                            "dist_version": "0.31.0",
                                            "artifacts": {name: {} for name in names}}))
            args = SimpleNamespace(distrib=root, manifest=manifest,
                                   expected_tag="v0.1.0-pre.5", source_sha="a" * 40)
            with patch.object(candidate, "check_source_archive_version"), \
                 patch.object(candidate, "check_source_archive_commit"):
                result = candidate.global_artifacts(args)
            self.assertEqual(result["aggregate_checksum_entries"], sorted(archives))

    def test_global_manifest_describes_native_and_global_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "manifest.json"
            archives = {f"qzt-{target}" + (".zip" if "windows" in target else ".tar.xz")
                        for target in candidate.TARGETS}
            names = archives | {f"{name}.sha256" for name in archives} | {
                "qzt-installer.sh", "qzt-installer.ps1", "source.tar.gz",
                "source.tar.gz.sha256", "sha256.sum"}
            manifest.write_text(json.dumps({"announcement_tag": "v0.1.0-pre.5",
                                            "announcement_is_prerelease": True,
                                            "dist_version": "0.31.0",
                                            "artifacts": {name: {} for name in names}}),
                                encoding="utf-8")
            candidate.check_manifest(manifest, names, "v0.1.0-pre.5")
            with self.assertRaisesRegex(RuntimeError, "unexpected artifacts"):
                candidate.check_manifest(manifest, names - {"sha256.sum"},
                                         "v0.1.0-pre.5")

    def test_manifest_rejects_another_candidate_tag(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "manifest.json"
            manifest.write_text(
                json.dumps({"announcement_tag": "v0.1.0-pre.3", "dist_version": "0.31.0",
                            "artifacts": {"archive": {}}}), encoding="utf-8"
            )
            with self.assertRaisesRegex(RuntimeError, "wrong tag"):
                candidate.check_manifest(manifest, ("archive",), "v0.1.0-pre.5")

    def test_manifest_rejects_extra_artifact_and_non_prerelease(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "manifest.json"
            contents = {"announcement_tag": "v0.1.0-pre.5",
                        "announcement_is_prerelease": True, "dist_version": "0.31.0",
                        "artifacts": {"archive": {}, "unexpected": {}}}
            manifest.write_text(json.dumps(contents), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "unexpected artifacts"):
                candidate.check_manifest(manifest, ("archive",), "v0.1.0-pre.5")
            contents["artifacts"].pop("unexpected")
            contents["announcement_is_prerelease"] = False
            manifest.write_text(json.dumps(contents), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "not a prerelease"):
                candidate.check_manifest(manifest, ("archive",), "v0.1.0-pre.5")

    def test_smoke_rejects_binary_from_another_version(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "old-qzt"
            binary.write_text(f"#!{sys.executable}\nprint('qzt 0.1.0-pre.3')\n", encoding="utf-8")
            binary.chmod(binary.stat().st_mode | 0o111)
            work = root / "work"
            work.mkdir()
            with self.assertRaisesRegex(RuntimeError, "wrong binary version"):
                candidate.smoke(binary, work, root, "aarch64-apple-darwin", "v0.1.0-pre.5")
            self.assertEqual(os.listdir(work), [])

    def test_source_archive_rejects_another_package_version(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "source.tar.gz"
            content = b'[package]\nname = "qzt"\nversion = "0.1.0-pre.3"\n'
            with tarfile.open(archive, "w:gz") as source:
                member = tarfile.TarInfo("qzt-0.1.0-pre.5/Cargo.toml")
                member.size = len(content)
                source.addfile(member, io.BytesIO(content))
            with self.assertRaisesRegex(RuntimeError, "wrong package version"):
                candidate.check_source_archive_version(archive, "v0.1.0-pre.5")

    def test_source_archive_rejects_another_commit_with_same_version(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "source.tar.gz"
            expected = b'[package]\nname = "qzt"\nversion = "0.1.0-pre.5"\n'
            changed = expected + b"# another commit\n"
            object_id = hashlib.sha1(b"blob " + str(len(expected)).encode() + b"\0" + expected).hexdigest()
            tree = f"100644 blob {object_id}\tCargo.toml\0".encode()
            with tarfile.open(archive, "w:gz") as source:
                member = tarfile.TarInfo("qzt-0.1.0-pre.5/Cargo.toml")
                member.size = len(changed)
                source.addfile(member, io.BytesIO(changed))
            with patch.object(candidate.subprocess, "check_output", side_effect=["sha1", tree]):
                with self.assertRaisesRegex(RuntimeError, "differs from source commit"):
                    candidate.check_source_archive_commit(archive, "v0.1.0-pre.5", "a" * 40)

    def test_installer_rejects_mixed_release_urls(self):
        content = ("/releases/download/v0.1.0-pre.5/qzt.tar.xz\n"
                   "/releases/download/v0.1.0-pre.3/qzt.tar.xz\n")
        with self.assertRaisesRegex(RuntimeError, "wrong version"):
            candidate.check_installer_tag(content, "v0.1.0-pre.5")

    def test_installer_rejects_mixed_stable_release_url(self):
        content = ("/releases/download/v0.1.0-pre.5/qzt.tar.xz\n"
                   "/releases/download/v0.1.0/qzt.tar.xz\n")
        with self.assertRaisesRegex(RuntimeError, "wrong version"):
            candidate.check_installer_tag(content, "v0.1.0-pre.5")


if __name__ == "__main__":
    unittest.main()
