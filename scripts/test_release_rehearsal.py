"""The pre-host gate must accept only the explicitly selected preview tag."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "rehearsal", Path(__file__).with_name("verify-release-rehearsal.py"))
rehearsal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rehearsal)


class AssemblyTagTests(unittest.TestCase):
    def test_selected_pre6_assembly_passes_and_other_tag_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifacts, staged = root / "artifacts", root / "staged"
            artifacts.mkdir()
            staged.mkdir()
            for name in rehearsal.ARCHIVES | {"source.tar.gz"}:
                (artifacts / name).write_bytes(name.encode())
                digest = rehearsal.digest(artifacts / name)
                (artifacts / (name + ".sha256")).write_text(f"{digest} *{name}\n")
            (artifacts / "sha256.sum").write_text("".join(
                f"{rehearsal.digest(artifacts / name)} *{name}\n"
                for name in sorted(rehearsal.ARCHIVES | {"source.tar.gz"})))
            for name in ("qzt-installer.sh", "qzt-installer.ps1"):
                (artifacts / name).write_text("installer")
            for name in rehearsal.ASSETS:
                (staged / name).write_bytes((artifacts / name).read_bytes())
            plan = {"announcement_tag": "v0.1.0-pre.6", "announcement_is_prerelease": True,
                    "dist_version": "0.31.0", "artifacts": {name: {} for name in rehearsal.ASSETS}}
            plan_path = artifacts / "plan-dist-manifest.json"
            plan_path.write_text(json.dumps(plan))
            for target in (*rehearsal.TARGETS, "global"):
                environment = {"schema": "qzt-build-environment-v1", "source_sha": "a" * 40,
                               "target": target, "cargo_dist": "cargo-dist 0.31.0",
                               "github_run": {"run_id": "1", "run_attempt": "1"}}
                (artifacts / f"{target}-build-environment.json").write_text(json.dumps(environment))
                manifest = {**plan, "artifacts": {name: {} for name in rehearsal.ASSETS
                            if target == "global" or name.startswith(f"qzt-{target}.")}}
                (artifacts / f"{target}-dist-manifest.json").write_text(json.dumps(manifest))
            with patch.object(rehearsal.subprocess, "check_output", return_value="a" * 40):
                result = rehearsal.inspect(artifacts, staged, plan_path, "a" * 40, "v0.1.0-pre.6")
                self.assertEqual(result["tag"], "v0.1.0-pre.6")
                self.assertEqual(len(result["staged_assets"]), 13)
                with self.assertRaisesRegex(RuntimeError, "release plan differs"):
                    rehearsal.inspect(artifacts, staged, plan_path, "a" * 40, "v0.1.0-pre.5")


if __name__ == "__main__":
    unittest.main()
