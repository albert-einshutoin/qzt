import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import oracle
from run import Runner, check_search


class CaseStudyChecks(unittest.TestCase):
    def test_byte_offsets_and_overlaps(self):
        self.assertEqual(oracle.occurrences("éaaaa".encode(), b"aaa"), [2, 3])

    def test_document_boundary_is_not_a_match(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            (base / "inputs").mkdir()
            items = []
            for name, data in (("one", b"ab"), ("two", b"cd")):
                (base / "inputs" / name).write_bytes(data)
                items.append({"file": name, "doc_id": "ci/" + name,
                              "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
            with patch.object(oracle, "QUERIES", ("bc",)):
                result = oracle.build({"input_order": items}, base)
            self.assertEqual(result["document_starts_from_source_lengths"], {"ci/one": 0, "ci/two": 2})
            self.assertEqual(result["queries"][0]["hits"], [])

    def test_cap_is_not_complete(self):
        hit = {"logical_offset": 4, "byte_length": 3, "source": "verified_original_bytes"}
        report = {"index_complete_declared": True, "index_coverage_verified": False,
                  "incomplete_reason": None, "capped": True, "stop_reason": "max_search_results", "hits": [hit]}
        check_search(report, [hit], capped=True)
        with self.assertRaises(RuntimeError):
            check_search(report, [hit], capped=False)
        report["stop_reason"] = None
        with self.assertRaises(RuntimeError):
            check_search(report, [hit], capped=True)

    def test_nonzero_command_fails_and_records_status(self):
        with tempfile.TemporaryDirectory() as directory:
            runner = Runner(Path("/usr/bin/false"), Path(directory))
            with self.assertRaises(RuntimeError):
                runner.call("expected-failure")
            self.assertEqual(runner.records[0]["exit_code"], 1)


if __name__ == "__main__":
    unittest.main()
