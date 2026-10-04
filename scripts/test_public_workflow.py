"""Fail-closed boundaries for the explicit context-enabled binary smoke."""

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "public_workflow", Path(__file__).with_name("verify-public-workflow.py")
)
workflow = importlib.util.module_from_spec(spec)
spec.loader.exec_module(workflow)


class PublicWorkflowTests(unittest.TestCase):
    def test_wrong_target_fails_before_execution(self):
        with patch.object(workflow.platform, "system", return_value="unsupported"), \
             patch.object(workflow.candidate, "smoke") as smoke:
            with self.assertRaisesRegex(RuntimeError, "wrong native target"):
                workflow.verify(Path("missing"), "0" * 64, "v0.1.0-pre.5",
                                "aarch64-apple-darwin", Path("missing"))
            smoke.assert_not_called()

    def test_wrong_hash_fails_before_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "qzt"
            binary.write_bytes(b"not a binary")
            with patch.object(workflow.platform, "system", return_value="Darwin"), \
                 patch.object(workflow.platform, "machine", return_value="arm64"), \
                 patch.object(workflow.candidate, "smoke") as smoke:
                with self.assertRaisesRegex(RuntimeError, "wrong binary SHA-256"):
                    workflow.verify(binary, "0" * 64, "v0.1.0-pre.5",
                                    "aarch64-apple-darwin", Path("missing"))
                smoke.assert_not_called()

    def test_exact_bytes_and_verification_are_independent(self):
        report = {"mapping_status": "unique", "scope": {"kind": "document"},
                  "hit": {"logical_offset": 1, "byte_length": 2, "end": 3, "future_key": "ignored"},
                  "excerpt": {"logical_offset": 1, "byte_length": 2, "bytes_hex": "6263"},
                  "verification": {"decoded_chunks_verified": True,
                                   "document_checksum_verified": False,
                                   "search_query_verified": False,
                                   "external_provenance_verified": False}}
        hit = {"logical_offset": 1, "byte_length": 2}
        workflow.check_context(report, b"abcd", "unique", "document", hit)
        for key in ("document_checksum_verified", "search_query_verified", "external_provenance_verified"):
            with self.subTest(key=key):
                report["verification"][key] = True
                with self.assertRaisesRegex(RuntimeError, "overclaims"):
                    workflow.check_context(report, b"abcd", "unique", "document", hit)
                report["verification"][key] = False
        report["excerpt"]["bytes_hex"] = "7879"
        with self.assertRaisesRegex(RuntimeError, "differs from original"):
            workflow.check_context(report, b"abcd", "unique", "document", hit)

    def test_identical_bytes_at_another_hit_are_rejected(self):
        report = {"mapping_status": "no_document_index", "scope": {"kind": "container"},
                  "hit": {"logical_offset": 5, "byte_length": 2, "end": 7},
                  "excerpt": {"logical_offset": 5, "byte_length": 2, "bytes_hex": "6263"}}
        with self.assertRaisesRegex(RuntimeError, "incorrect context hit coordinates"):
            workflow.check_context(report, b"abcdabcd", "no_document_index", "container",
                                   {"logical_offset": 1, "byte_length": 2})

    def test_report_created_during_smoke_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "report.json"

            def concurrent_report(*args):
                output.write_text("other run", encoding="utf-8")
                return {"ok": True}

            with patch.object(sys, "argv", ["verify-public-workflow.py", "--binary", str(root / "qzt"),
                         "--binary-sha256", "0" * 64, "--expected-tag", "v0.1.0-pre.5",
                         "--target", "aarch64-apple-darwin", "--vectors-dir", str(root),
                         "--output", str(output)]), \
                 patch.object(workflow, "verify", side_effect=concurrent_report):
                with self.assertRaises(FileExistsError):
                    workflow.main()
            self.assertEqual(output.read_text(encoding="utf-8"), "other run")

    def test_missing_context_is_not_skipped(self):
        with patch.object(workflow, "json_command", side_effect=[
                {"hits": [{"logical_offset": 11, "byte_length": 5}]},
                RuntimeError("context: exit 2, expected 0")]):
            with self.assertRaisesRegex(RuntimeError, "context: exit 2"):
                workflow.context_smoke(Path("qzt"), Path("work"))


if __name__ == "__main__":
    unittest.main()
