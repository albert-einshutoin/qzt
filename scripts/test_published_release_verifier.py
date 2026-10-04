"""Focused trust-boundary checks for the public Release verifier."""

import importlib.util
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "published_verifier", Path(__file__).with_name("verify-published-release.py")
)
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)

TAG = "v0.1.0-pre.5"
SHA = "3bc7d2561c58b59cd166fed75bfbd45f2f4c0ebe"
RUN = 12345
ATTEMPT = 1


class PublishedReleaseTests(unittest.TestCase):
    def test_candidate_override_cannot_use_an_external_download_url(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.object(verifier.subprocess, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "explicit loopback"):
                verifier.run_installer(Path("installer"), Path(directory),
                                       "x86_64-pc-windows-msvc", "0" * 64, Path("vectors"),
                                       "v0.1.0-pre.6", "qzt 0.1.0-pre.6", "public-workflow-v1",
                                       candidate_base="https://example.invalid")
            run.assert_not_called()

    def metadata(self):
        return {
            f"{verifier.API}/git/ref/tags/{TAG}": {"object": {"type": "tag", "url": "tag-object"}},
            "tag-object": {"object": {"type": "commit", "sha": SHA}},
            f"{verifier.API}/releases/tags/{TAG}": {
                "tag_name": TAG, "target_commitish": SHA, "prerelease": True,
                "draft": False, "assets": [{"name": name} for name in verifier.EXPECTED_ASSETS],
            },
            f"{verifier.API}/actions/runs/{RUN}": {
                "path": ".github/workflows/release.yml", "event": "push",
                "head_sha": SHA, "head_branch": TAG, "run_attempt": ATTEMPT,
                "conclusion": "success",
            },
        }

    def check(self, metadata):
        with patch.object(verifier, "get_json", side_effect=metadata.__getitem__):
            return verifier.publication(TAG, SHA, RUN, ATTEMPT)

    def test_exact_publication_metadata_passes(self):
        release, assets = self.check(self.metadata())
        self.assertEqual(release["tag_name"], TAG)
        self.assertEqual(set(assets), verifier.EXPECTED_ASSETS)

    def test_wrong_tag_product_or_run_attempt_fails(self):
        for key, value in (
            ("tag-object", {"object": {"type": "commit", "sha": "0" * 40}}),
            (f"{verifier.API}/actions/runs/{RUN}", {
                **self.metadata()[f"{verifier.API}/actions/runs/{RUN}"], "run_attempt": 2,
            }),
            (f"{verifier.API}/releases/tags/{TAG}", {
                **self.metadata()[f"{verifier.API}/releases/tags/{TAG}"], "tag_name": "v0.1.0-pre.3",
            }),
        ):
            with self.subTest(key=key):
                metadata = self.metadata()
                metadata[key] = value
                with self.assertRaises(RuntimeError):
                    self.check(metadata)

    def test_asset_missing_or_duplicated_fails(self):
        key = f"{verifier.API}/releases/tags/{TAG}"
        for assets in ([], [{"name": name} for name in verifier.EXPECTED_ASSETS] +
                       [{"name": "source.tar.gz"}]):
            with self.subTest(count=len(assets)):
                metadata = self.metadata()
                metadata[key]["assets"] = assets
                with self.assertRaises(RuntimeError):
                    self.check(metadata)

    def test_downloaded_code_gets_no_actions_credentials(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(
            verifier.os.environ,
            {"GH_TOKEN": "secret", "GITHUB_TOKEN": "secret", "AWS_SECRET_ACCESS_KEY": "secret"},
        ):
            env = verifier.execution_env(Path(directory))
            self.assertFalse({"GH_TOKEN", "GITHUB_TOKEN", "AWS_SECRET_ACCESS_KEY"} & env.keys())
            self.assertEqual(env["HOME"], str(Path(directory) / "home"))
            with verifier.isolated_binary_env(Path(directory)):
                self.assertFalse({"GH_TOKEN", "GITHUB_TOKEN", "AWS_SECRET_ACCESS_KEY"} &
                                 verifier.os.environ.keys())
            self.assertEqual(verifier.os.environ["GH_TOKEN"], "secret")


if __name__ == "__main__":
    unittest.main()
