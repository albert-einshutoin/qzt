"""Regression checks for the release build's clean-checkout boundary."""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parent


class ReleaseManifestTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="qzt-release-manifest-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.git("init", "-q")
        (self.repo / ".gitignore").write_text("/target/\n", encoding="utf-8")
        (self.repo / "source.txt").write_text("source\n", encoding="utf-8")
        self.git("add", ".gitignore", "source.txt")
        self.git("-c", "user.name=QZT", "-c", "user.email=qzt@example.invalid",
                 "commit", "-qm", "source")
        self.source_sha = self.git("rev-parse", "HEAD").stdout.strip()
        tools = self.root / "bin"
        tools.mkdir()
        dist = tools / "dist"
        dist.write_text(
            "#!/bin/sh\n"
            "if [ \"$1\" = --version ]; then echo 'cargo-dist 0.31.0'; exit; fi\n"
            "if [ \"$1\" = build ]; then "
            "echo '{\"upload_files\":[\"target/distrib/qzt-test.tar.xz\"]}'; exit; fi\n"
            "if [ \"$1\" = print-upload-files-from-manifest ]; then "
            "test \"$2\" = --manifest && test -f \"$3\" && "
            "echo target/distrib/qzt-test.tar.xz; exit; fi\n"
            "exit 2\n", encoding="utf-8")
        dist.chmod(0o755)
        self.env = os.environ.copy()
        self.env["PATH"] = f"{tools}{os.pathsep}{self.env['PATH']}"
        self.env["RUSTUP_TOOLCHAIN"] = subprocess.check_output(
            ["rustup", "show", "active-toolchain"], text=True).split()[0]
        self.env["BUILD_MANIFEST_NAME"] = "target/distrib/test-dist-manifest.json"
        self.record()

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.repo, text=True,
                              capture_output=True, check=True)

    def recorder(self, mode, *, env=None, source_sha=None):
        command = [sys.executable, str(SCRIPTS / "record-build-environment.py"), mode]
        if mode == "record":
            command += ["--source-sha", source_sha or self.source_sha,
                        "--target", "test", "--profile", "test",
                        "--features", "default", "--build-command", "dist build",
                        "--output", "target/release-workflow/environment.json"]
        else:
            command += ["--input", "target/release-workflow/environment.json"]
        return subprocess.run(command, cwd=self.repo, env=env or self.env,
                              text=True, capture_output=True)

    def record(self):
        result = self.recorder("record")
        self.assertEqual(result.returncode, 0, result.stderr)

    def helper(self, action):
        return subprocess.run([str(SCRIPTS / "release-workflow-build.sh"), action],
                              cwd=self.repo, env=self.env, text=True,
                              capture_output=True)

    def test_root_manifest_reproduces_dirty_failure(self):
        (self.repo / "dist-manifest.json").write_text("{}\n", encoding="utf-8")
        result = self.recorder("verify")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("build checkout is dirty", result.stderr)

    def test_native_and_global_manifest_operations_preserve_clean_checkout(self):
        for output in ("native", "global"):
            with self.subTest(output=output):
                result = self.helper("build")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.recorder("verify").returncode, 0)
                raw = self.repo / "target/release-workflow/dist-manifest.json"
                self.assertEqual(json.loads(raw.read_text())["upload_files"],
                                 ["target/distrib/qzt-test.tar.xz"])
                result = self.helper("print-local" if output == "native" else "print-global")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), "target/distrib/qzt-test.tar.xz")
                (self.repo / "target/distrib").mkdir(exist_ok=True)
                self.assertEqual(self.helper("copy").returncode, 0)
                self.assertEqual((self.repo / self.env["BUILD_MANIFEST_NAME"]).read_bytes(),
                                 raw.read_bytes())
                self.assertEqual(self.git("status", "--porcelain").stdout, "")

    def test_unrelated_root_file_and_tracked_change_still_fail_closed(self):
        (self.repo / "unknown.txt").write_text("unknown\n", encoding="utf-8")
        self.assertIn("build checkout is dirty", self.recorder("verify").stderr)
        (self.repo / "unknown.txt").unlink()
        (self.repo / "source.txt").write_text("changed\n", encoding="utf-8")
        self.assertIn("build checkout is dirty", self.recorder("verify").stderr)

    def test_source_sha_and_build_flags_mismatch_still_fail_closed(self):
        self.assertIn("build checkout differs from source SHA",
                      self.recorder("record", source_sha="0" * 40).stderr)
        changed_env = self.env.copy()
        changed_env["RUSTFLAGS"] = "-C opt-level=1"
        self.assertIn("build toolchain, flags, source, or runner changed",
                      self.recorder("verify", env=changed_env).stderr)


if __name__ == "__main__":
    unittest.main()
