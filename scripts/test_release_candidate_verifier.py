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
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "candidate_verifier", Path(__file__).with_name("verify-release-candidate.py")
)
candidate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(candidate)


class VersionBoundaryTests(unittest.TestCase):
    def test_manifest_rejects_another_candidate_tag(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "manifest.json"
            manifest.write_text(
                json.dumps({"announcement_tag": "v0.1.0-pre.3", "dist_version": "0.31.0",
                            "artifacts": {"archive": {}}}), encoding="utf-8"
            )
            with self.assertRaisesRegex(RuntimeError, "wrong tag"):
                candidate.check_manifest(manifest, ("archive",), "v0.1.0-pre.4")

    def test_manifest_rejects_extra_artifact_and_non_prerelease(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "manifest.json"
            contents = {"announcement_tag": "v0.1.0-pre.4",
                        "announcement_is_prerelease": True, "dist_version": "0.31.0",
                        "artifacts": {"archive": {}, "unexpected": {}}}
            manifest.write_text(json.dumps(contents), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "unexpected artifacts"):
                candidate.check_manifest(manifest, ("archive",), "v0.1.0-pre.4")
            contents["artifacts"].pop("unexpected")
            contents["announcement_is_prerelease"] = False
            manifest.write_text(json.dumps(contents), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "not a prerelease"):
                candidate.check_manifest(manifest, ("archive",), "v0.1.0-pre.4")

    def test_smoke_rejects_binary_from_another_version(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "old-qzt"
            binary.write_text(f"#!{sys.executable}\nprint('qzt 0.1.0-pre.3')\n", encoding="utf-8")
            binary.chmod(binary.stat().st_mode | 0o111)
            work = root / "work"
            work.mkdir()
            with self.assertRaisesRegex(RuntimeError, "wrong binary version"):
                candidate.smoke(binary, work, root, "aarch64-apple-darwin", "v0.1.0-pre.4")
            self.assertEqual(os.listdir(work), [])

    def test_source_archive_rejects_another_package_version(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "source.tar.gz"
            content = b'[package]\nname = "qzt"\nversion = "0.1.0-pre.3"\n'
            with tarfile.open(archive, "w:gz") as source:
                member = tarfile.TarInfo("qzt-0.1.0-pre.4/Cargo.toml")
                member.size = len(content)
                source.addfile(member, io.BytesIO(content))
            with self.assertRaisesRegex(RuntimeError, "wrong package version"):
                candidate.check_source_archive_version(archive, "v0.1.0-pre.4")

    def test_source_archive_rejects_another_commit_with_same_version(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "source.tar.gz"
            expected = b'[package]\nname = "qzt"\nversion = "0.1.0-pre.4"\n'
            changed = expected + b"# another commit\n"
            object_id = hashlib.sha1(b"blob " + str(len(expected)).encode() + b"\0" + expected).hexdigest()
            tree = f"100644 blob {object_id}\tCargo.toml\0".encode()
            with tarfile.open(archive, "w:gz") as source:
                member = tarfile.TarInfo("qzt-0.1.0-pre.4/Cargo.toml")
                member.size = len(changed)
                source.addfile(member, io.BytesIO(changed))
            with patch.object(candidate.subprocess, "check_output", side_effect=["sha1", tree]):
                with self.assertRaisesRegex(RuntimeError, "differs from source commit"):
                    candidate.check_source_archive_commit(archive, "v0.1.0-pre.4", "a" * 40)

    def test_installer_rejects_mixed_release_urls(self):
        content = ("/releases/download/v0.1.0-pre.4/qzt.tar.xz\n"
                   "/releases/download/v0.1.0-pre.3/qzt.tar.xz\n")
        with self.assertRaisesRegex(RuntimeError, "wrong version"):
            candidate.check_installer_tag(content, "v0.1.0-pre.4")

    def test_installer_rejects_mixed_stable_release_url(self):
        content = ("/releases/download/v0.1.0-pre.4/qzt.tar.xz\n"
                   "/releases/download/v0.1.0/qzt.tar.xz\n")
        with self.assertRaisesRegex(RuntimeError, "wrong version"):
            candidate.check_installer_tag(content, "v0.1.0-pre.4")


if __name__ == "__main__":
    unittest.main()
