import copy
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[1]
SOURCE_SHA = "c11b7eb9a025f75d84ea9d3029a0674a606c04c6"


class PublicHistoryEvidence(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = Path(os.environ.get("QZT_PUBLIC_HISTORY_BINARY", ROOT / "target/debug/qzt")).resolve()
        cls.temp = tempfile.TemporaryDirectory(prefix="qzt-public-history-test-")
        cls.workspace = Path(cls.temp.name)
        cls.handoff = cls.workspace / "handoff"
        command = [sys.executable, str(BASE / "produce.py"), "--binary", str(cls.binary), "--snapshot", str(BASE / "snapshot"),
                   "--source-sha", SOURCE_SHA, "--output", str(cls.handoff)]
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=60)
        if result.returncode:
            raise AssertionError(result.stderr)
        cls.manifest_sha = result.stdout.strip().splitlines()[-1]

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def run_recipient(self, directory, output="report.json", log="commands.json", manifest_sha=None):
        return subprocess.run([sys.executable, "recipient.py", "--manifest-sha256", manifest_sha or self.manifest_sha, "--output", output, "--log", log],
                              cwd=directory, capture_output=True, text=True, timeout=60)

    def fresh_handoff(self, name):
        path = self.workspace / name
        shutil.copytree(self.handoff, path)
        return path

    def recipient_module(self):
        spec = importlib.util.spec_from_file_location("public_history_recipient", BASE / "recipient.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def evaluate_report(self, run_dir, report, name):
        report_path = run_dir / f"{name}-report.json"
        output_path = self.workspace / f"{name}-evaluation.json"
        report_path.write_text(json.dumps(report, ensure_ascii=False))
        return subprocess.run(
            [sys.executable, str(BASE / "evaluate.py"), "--report", str(report_path),
             "--manifest", str(run_dir / "manifest.json"), "--evaluation", str(BASE / "evaluation.json"),
             "--receipt", str(self.workspace / "handoff-receipt.json"), "--output", str(output_path)],
            cwd=ROOT, capture_output=True, text=True, timeout=30,
        ), output_path

    def test_acquired_snapshot_is_directly_accepted_by_producer(self):
        acquire_spec = importlib.util.spec_from_file_location("public_history_acquire", BASE / "acquire.py")
        acquire = importlib.util.module_from_spec(acquire_spec)
        acquire_spec.loader.exec_module(acquire)
        produce_spec = importlib.util.spec_from_file_location("public_history_produce", BASE / "produce.py")
        produce = importlib.util.module_from_spec(produce_spec)
        produce_spec.loader.exec_module(produce)

        saved = json.loads((BASE / "snapshot/snapshot.json").read_text())
        responses = []
        requests = []

        class Response:
            def __init__(self, raw, headers, status):
                self.raw = raw
                self.headers = headers
                self.status = status

            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def read(self, _limit):
                return self.raw

        for record in saved["records"]:
            responses.append(Response(
                (BASE / "snapshot" / record["raw"]["path"]).read_bytes(),
                record["headers"],
                record["status"],
            ))

        def urlopen(request, timeout):
            requests.append(request)
            self.assertEqual(timeout, 30)
            return responses[len(requests) - 1]

        acquired = self.workspace / "fresh-acquisition"
        fixed_stamp = "2026-10-12T00:00:00Z"
        with mock.patch.object(sys, "argv", [str(BASE / "acquire.py"), str(acquired)]), \
             mock.patch.object(acquire, "stamp", return_value=fixed_stamp), \
             mock.patch.object(acquire.urllib.request, "urlopen", side_effect=urlopen):
            acquire.main()

        self.assertEqual([request.full_url for request in requests], acquire.URLS)
        self.assertTrue(all(request.get_method() == "GET" for request in requests))
        self.assertTrue(all({key.lower() for key, _ in request.header_items()} == {"accept", "user-agent"}
                            for request in requests))
        index = json.loads((acquired / "snapshot.json").read_text())
        expected_fields = {"url", "fetched_at", "status", "headers", "raw", "body", "updated_at", "id", "node_id"}
        with mock.patch.dict(sys.modules, {"acquire": acquire}):
            docs, _, _, _ = produce.snapshot_docs(acquired)
        self.assertEqual(acquire.RECORD_FIELDS, expected_fields)
        self.assertTrue(all(set(record) == expected_fields for record in index["records"]))
        self.assertEqual(len(docs), 10)
        self.assertEqual(sum(len(data) for _, data, _ in docs), 36_633)
        records_by_url = {record["url"]: record for record in index["records"]}
        fixed_sources = []
        for _, _, source in docs:
            url = source["acquisition_url"]
            if "/reviews" in url:
                raw = json.loads((acquired / records_by_url[url]["raw"]["path"]).read_text())
                review = raw[int(source["body_json_pointer"].split("/")[1])]
                self.assertEqual(source["submitted_at"], review["submitted_at"])
            if url.startswith("https://raw.githubusercontent.com/"):
                self.assertEqual(source["fetched_at"], fixed_stamp)
                fixed_sources.append(url)
        self.assertEqual(fixed_sources, acquire.URLS[-2:])

    def test_snapshot_policy_and_handoff_inventory(self):
        policy = json.loads((BASE / "snapshot/acquisition-policy.json").read_text())
        snapshot = json.loads((BASE / "snapshot/snapshot.json").read_text())
        self.assertEqual(len(policy["urls"]), 9)
        self.assertEqual({r["url"] for r in snapshot["records"]}, set(policy["urls"]))
        self.assertLessEqual(snapshot["total_response_bytes"], 10 * 1024 * 1024)
        self.assertLessEqual(snapshot["total_body_bytes"], 1024 * 1024)
        expected = {"evidence.qzt", "manifest.json", "summary.txt", "questions.json", "recipient.py", "qzt.exe" if os.name == "nt" else "qzt"}
        self.assertEqual({p.name for p in self.handoff.iterdir()}, expected)
        self.assertFalse((self.handoff / "evaluation.json").exists())
        self.assertNotIn(str(ROOT), (self.handoff / "manifest.json").read_text())
        # The producer must freeze provenance before packing, not merely log success afterwards.
        receipt = self.workspace / "handoff-receipt.json"
        self.assertTrue(receipt.is_file())
        frozen = json.loads(receipt.read_text())
        self.assertEqual(frozen["stage"], "pre_pack")
        self.assertEqual(frozen["product_source_sha_self_reported"], "c11b7eb9a025f75d84ea9d3029a0674a606c04c6")
        producer_log = json.loads((self.workspace / "handoff-producer.jsonl").read_text())
        self.assertTrue(all(record["input_sha256"] for record in producer_log["records"]))
        manifest = json.loads((self.handoff / "manifest.json").read_text())
        self.assertEqual(manifest["selection"]["excluded"], [{
            "id": 5479806767,
            "node_id": "PRR_kwDOSzopF88AAAABRp83Lw",
            "json_pointer": "/1/body",
            "reason": "empty body excluded",
        }])

    def test_e2e_reaches_four_questions_and_two_q5_documents(self):
        run_dir = self.fresh_handoff("e2e")
        result = self.run_recipient(run_dir)
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads((run_dir / "report.json").read_text())
        command_records = json.loads((run_dir / "commands.json").read_text())
        qzt_sha = hashlib.sha256((run_dir / "evidence.qzt").read_bytes()).hexdigest()
        self.assertTrue(all(qzt_sha in record["input_sha256"].values() for record in command_records))
        evaluation_path = self.workspace / "evaluation-report.json"
        evaluation = subprocess.run([sys.executable, str(BASE / "evaluate.py"), "--report", str(run_dir / "report.json"),
                                     "--manifest", str(run_dir / "manifest.json"), "--evaluation", str(BASE / "evaluation.json"),
                                     "--receipt", str(self.workspace / "handoff-receipt.json"),
                                     "--output", str(evaluation_path)], cwd=ROOT, capture_output=True, text=True, timeout=30)
        self.assertEqual(evaluation.returncode, 0, evaluation.stderr)
        results = json.loads(evaluation_path.read_text())["questions"]
        self.assertGreaterEqual(sum(x["outcome"] == "acquired" for x in results), 4)
        q5 = next(x for x in report["questions"] if x["id"] == "q5")
        self.assertTrue({"github/pr-345-comment-4238230444.txt", "github/pr-345-comment-4238290457.txt"}
                        <= {hit["doc_id"] for hit in q5["hits"]})
        manifest = json.loads((run_dir / "manifest.json").read_text())
        snapshot_bytes = {}
        for document in manifest["documents"]:
            source = document["source"]
            raw = (BASE / "snapshot" / source["raw_response"]["path"]).read_bytes()
            pointer = source["body_json_pointer"]
            if pointer is None:
                data = raw
            else:
                value = json.loads(raw)
                data = (value["body"] if pointer == "/body" else value[int(pointer.split("/")[1])]["body"]).encode()
            snapshot_bytes[document["doc_id"]] = data
        verified = {item["doc_id"]: bytes.fromhex(item["bytes_hex"]) for item in report["verified_documents"]}
        self.assertEqual(verified, {doc_id: snapshot_bytes[doc_id] for doc_id in verified})
        for question in report["questions"]:
            for hit in question["hits"]:
                data = snapshot_bytes[hit["doc_id"]]
                start, end = hit["local_range"]
                self.assertEqual(data[start:end], hit["query"].encode())
                excerpt = hit["context"]["excerpt"]
                local = excerpt["logical_offset"] - next(
                    doc["logical_offset"] for doc in manifest["documents"] if doc["doc_id"] == hit["doc_id"]
                )
                self.assertEqual(bytes.fromhex(excerpt["bytes_hex"]), data[local:local + excerpt["byte_length"]])
        q5_criterion = json.loads((BASE / "evaluation.json").read_text())["questions"]["q5"]
        self.assertTrue(all(any(
            hit["doc_id"] == required["doc_id"]
            and required["fragment"].encode() in bytes.fromhex(hit["context"]["excerpt"]["bytes_hex"])
            for hit in q5["hits"]
        ) for required in q5_criterion["required"]))
        self.assertEqual(next(x for x in results if x["id"] == "q7")["outcome"], "record_insufficient")
        self.assertTrue(all(x["coverage_verified"] is False for x in results))

    def test_corruption_range_checksum_and_reexecution_fail_closed(self):
        run_dir = self.fresh_handoff("retry")
        result = self.run_recipient(run_dir)
        self.assertEqual(result.returncode, 0, result.stderr)
        original_report = (run_dir / "report.json").read_bytes()
        rerun = self.run_recipient(run_dir, "report.json", "commands.json")
        self.assertNotEqual(rerun.returncode, 0)
        self.assertEqual((run_dir / "report.json").read_bytes(), original_report)
        corrupt = self.fresh_handoff("corrupt")
        self.assertEqual(self.run_recipient(corrupt).returncode, 0)
        qzi = corrupt / "evidence.qzi"; data = bytearray(qzi.read_bytes()); data[-1] ^= 1; qzi.write_bytes(data)
        corrupt_qzi = qzi.read_bytes()
        bad = self.run_recipient(corrupt, "bad.json", "bad-log.json")
        self.assertNotEqual(bad.returncode, 0)
        self.assertFalse((corrupt / "bad.json").exists())
        self.assertEqual(qzi.read_bytes(), corrupt_qzi)
        bad_manifest = self.fresh_handoff("bad-manifest")
        manifest = json.loads((bad_manifest / "manifest.json").read_text()); manifest["documents"][0]["logical_offset"] = 1
        (bad_manifest / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False))
        resealed = hashlib.sha256((bad_manifest / "manifest.json").read_bytes()).hexdigest()
        bad_hash = self.run_recipient(bad_manifest, "bad.json", "bad-log.json", resealed)
        self.assertNotEqual(bad_hash.returncode, 0)
        broken_qzt = self.fresh_handoff("broken-qzt")
        qzt = broken_qzt / "evidence.qzt"; bytes_ = bytearray(qzt.read_bytes()); bytes_[-1] ^= 1; qzt.write_bytes(bytes_)
        corrupted_qzt = qzt.read_bytes()
        broken = self.run_recipient(broken_qzt, "broken.json", "broken-log.json")
        self.assertNotEqual(broken.returncode, 0); self.assertFalse((broken_qzt / "broken.json").exists())
        self.assertEqual(qzt.read_bytes(), corrupted_qzt)

    def test_invalid_questions_and_source_sha_syntax_are_rejected(self):
        invalid = self.fresh_handoff("invalid")
        questions = json.loads((invalid / "questions.json").read_text()); questions["questions"][0]["queries"] = []
        (invalid / "questions.json").write_text(json.dumps(questions))
        self.assertNotEqual(self.run_recipient(invalid, "invalid.json", "invalid-log.json").returncode, 0)
        output = self.workspace / "wrong-source"
        result = subprocess.run([sys.executable, str(BASE / "produce.py"), "--binary", str(self.binary), "--snapshot", str(BASE / "snapshot"),
                                 "--source-sha", "not-a-sha", "--output", str(output)], cwd=ROOT, capture_output=True, text=True, timeout=30)
        self.assertNotEqual(result.returncode, 0)

    def test_snapshot_tamper_runtime_question_tamper_and_existing_receipt_fail_closed(self):
        snapshot = self.workspace / "snapshot"; shutil.copytree(BASE / "snapshot", snapshot)
        raw = next((snapshot / "raw").iterdir()); data = bytearray(raw.read_bytes()); data[0] ^= 1; raw.write_bytes(data)
        tampered = subprocess.run([sys.executable, str(BASE / "produce.py"), "--binary", str(self.binary), "--snapshot", str(snapshot),
                                  "--source-sha", SOURCE_SHA, "--output", str(self.workspace / "bad-snapshot")], cwd=ROOT, capture_output=True, text=True, timeout=30)
        self.assertNotEqual(tampered.returncode, 0)
        run_dir = self.fresh_handoff("question-seal")
        questions = json.loads((run_dir / "questions.json").read_text()); questions["questions"][0]["queries"] = ["入力QZT"]
        (run_dir / "questions.json").write_text(json.dumps(questions, ensure_ascii=False))
        sealed = hashlib.sha256((run_dir / "manifest.json").read_bytes()).hexdigest()
        self.assertNotEqual(self.run_recipient(run_dir, "bad.json", "bad-log.json", sealed).returncode, 0)
        self.assertFalse((run_dir / "bad.json").exists())
        protected = self.workspace / "receipt-only-receipt.json"; protected.write_bytes(b"keep")
        denied = subprocess.run([sys.executable, str(BASE / "produce.py"), "--binary", str(self.binary), "--snapshot", str(BASE / "snapshot"),
                                "--source-sha", SOURCE_SHA, "--output", str(self.workspace / "receipt-only")], cwd=ROOT, capture_output=True, text=True, timeout=30)
        self.assertNotEqual(denied.returncode, 0); self.assertEqual(protected.read_bytes(), b"keep")

    def test_unknown_schema_is_error_and_known_partial_is_not_complete(self):
        module = self.recipient_module()
        valid = {"hits": [], "metrics": {
                    "query": "query", "index_kind": "ngram", "posting_granularity": "line",
                    "index_size_bytes": 1, "source_size_bytes": 1, "index_size_ratio": 1.0,
                    "term_lookups": 1, "posting_bytes_read": 0, "candidate_granules": 0,
                    "candidate_chunks": 0, "decoded_bytes": 0, "physical_decoded_bytes": 0,
                    "physical_decoded_chunks": 0, "verified_matches": 0, "query_time_ms": 0.1},
                 "capped": False, "stop_reason": None,
                 "index_complete_declared": True, "index_coverage_verified": False, "incomplete_reason": None}
        self.assertIsNone(module.search_status(valid, "query"))
        capped = copy.deepcopy(valid); capped["capped"] = True; capped["stop_reason"] = "max_search_results"
        self.assertEqual(module.search_status(capped, "query")["kind"], "partial_unknown")
        incomplete = copy.deepcopy(valid); incomplete["incomplete_reason"] = "query_shorter_than_ngram_n"
        self.assertEqual(module.search_status(incomplete, "query")["kind"], "partial_unknown")
        declared = copy.deepcopy(valid); declared["index_complete_declared"] = False
        self.assertEqual(module.search_status(declared, "query")["kind"], "partial_unknown")
        unknown = copy.deepcopy(valid); unknown.pop("stop_reason")
        with self.assertRaises(RuntimeError): module.search_status(unknown, "query")
        unknown_reason = copy.deepcopy(valid); unknown_reason["incomplete_reason"] = "future_reason"
        with self.assertRaises(RuntimeError): module.search_status(unknown_reason, "query")
        fake_cap = copy.deepcopy(valid); fake_cap["capped"] = True; fake_cap["stop_reason"] = "max_results"
        with self.assertRaises(RuntimeError): module.search_status(fake_cap, "query")
        inconsistent = copy.deepcopy(valid); inconsistent["capped"] = True
        with self.assertRaises(RuntimeError): module.search_status(inconsistent, "query")

    def test_report_byte_proof_query_binding_and_partial_evaluation(self):
        run_dir = self.fresh_handoff("report-proof")
        self.assertEqual(self.run_recipient(run_dir).returncode, 0)
        original = json.loads((run_dir / "report.json").read_text())
        self.assertTrue(original["verified_documents"])

        mutations = {}
        missing_search = copy.deepcopy(original)
        missing_search["questions"][0]["searches"].pop()
        mutations["missing-search"] = missing_search
        wrong_query = copy.deepcopy(original)
        wrong_query["questions"][0]["searches"][0]["query"] = "違うquery"
        mutations["wrong-query"] = wrong_query
        wrong_digest = copy.deepcopy(original)
        wrong_digest["verified_documents"][0]["sha256"] = "0" * 64
        mutations["wrong-doc-digest"] = wrong_digest
        wrong_container = copy.deepcopy(original)
        wrong_container["container_id"] = "0" * 32
        mutations["wrong-container"] = wrong_container
        false_verified = copy.deepcopy(original)
        false_verified["questions"][0]["hits"][0]["document_checksum_verified"] = False
        mutations["false-verified"] = false_verified
        false_context_verified = copy.deepcopy(original)
        false_context_verified["questions"][0]["hits"][0]["context"]["verification"]["decoded_chunks_verified"] = False
        mutations["false-context-verified"] = false_context_verified
        overflow = copy.deepcopy(original)
        overflow["questions"][0]["hits"][0]["global_range"][1] += 10**9
        mutations["range-overflow"] = overflow
        unknown_stop = copy.deepcopy(original)
        search = unknown_stop["questions"][0]["searches"][0]
        search["raw_metadata"]["capped"] = True
        search["raw_metadata"]["stop_reason"] = "future_cap"
        search["classification"] = "partial_unknown"
        search["reason"] = "future_cap"
        unknown_stop["questions"][0]["candidate_status"] = "partial_unknown"
        mutations["unknown-stop"] = unknown_stop

        tampered_excerpt = copy.deepcopy(original)
        excerpt = tampered_excerpt["questions"][0]["hits"][0]["context"]["excerpt"]
        excerpt_bytes = bytearray.fromhex(excerpt["bytes_hex"])
        excerpt_bytes[-1] ^= 1
        excerpt["bytes_hex"] = excerpt_bytes.hex()
        mutations["tampered-excerpt"] = tampered_excerpt
        tampered_display = copy.deepcopy(original)
        tampered_display["questions"][0]["hits"][0]["context"]["excerpt"]["text_escaped"] += "x"
        mutations["tampered-display"] = tampered_display

        for name, report in mutations.items():
            with self.subTest(name=name):
                result, output = self.evaluate_report(run_dir, report, name)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(output.exists())

        partial = copy.deepcopy(original)
        search = partial["questions"][0]["searches"][0]
        search["raw_metadata"]["capped"] = True
        search["raw_metadata"]["stop_reason"] = "max_search_results"
        search["classification"] = "partial_unknown"
        search["reason"] = "max_search_results"
        partial["questions"][0]["candidate_status"] = "partial_unknown"
        result, output = self.evaluate_report(run_dir, partial, "known-partial")
        self.assertEqual(result.returncode, 0, result.stderr)
        evaluated = json.loads(output.read_text())["questions"]
        self.assertEqual(len(evaluated), 7)
        self.assertEqual(evaluated[0]["outcome"], "partial_unknown")

        fragment = copy.deepcopy(original)
        context = fragment["questions"][0]["hits"][0]["context"]
        context["excerpt"]["trailing_fragment"] = True
        context["after"]["stop"] = "budget"
        context["after"]["returned"] = 0
        entry = fragment["questions"][0]
        entry["candidate_status"] = "partial_unknown"
        result, output = self.evaluate_report(run_dir, fragment, "context-fragment")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(output.read_text())["questions"][0]["outcome"], "partial_unknown")

    def test_snapshot_body_must_equal_raw_extraction(self):
        snapshot = self.workspace / "body-binding-snapshot"
        shutil.copytree(BASE / "snapshot", snapshot)
        index_path = snapshot / "snapshot.json"
        index = json.loads(index_path.read_text())
        record = index["records"][0]
        body_path = snapshot / record["body"]["path"]
        body = body_path.read_bytes() + b"x"
        body_path.write_bytes(body)
        index["total_body_bytes"] += 1
        record["body"]["size"] = len(body)
        record["body"]["sha256"] = hashlib.sha256(body).hexdigest()
        index_path.write_text(json.dumps(index))
        denied = subprocess.run(
            [sys.executable, str(BASE / "produce.py"), "--binary", str(self.binary), "--snapshot", str(snapshot),
             "--source-sha", SOURCE_SHA, "--output", str(self.workspace / "body-binding")],
            cwd=ROOT, capture_output=True, text=True, timeout=30,
        )
        self.assertNotEqual(denied.returncode, 0)

    def test_snapshot_escape_policy_tamper_and_forged_report_are_rejected(self):
        snapshot = self.workspace / "safe-snapshot"; shutil.copytree(BASE / "snapshot", snapshot)
        outside = self.workspace / "outside.raw"; outside.write_bytes(b"not read")
        index_path = snapshot / "snapshot.json"; index = json.loads(index_path.read_text())
        index["records"][0]["raw"]["path"] = "../outside.raw"
        index_path.write_text(json.dumps(index))
        denied = subprocess.run([sys.executable, str(BASE / "produce.py"), "--binary", str(self.binary), "--snapshot", str(snapshot),
                                 "--source-sha", SOURCE_SHA, "--output", str(self.workspace / "escape")], cwd=ROOT, capture_output=True, text=True, timeout=30)
        self.assertNotEqual(denied.returncode, 0)
        self.assertEqual(outside.read_bytes(), b"not read")
        symlink_snapshot = self.workspace / "symlink-snapshot"; shutil.copytree(BASE / "snapshot", symlink_snapshot)
        raw_dir = symlink_snapshot / "raw"; raw_copy = self.workspace / "raw-copy"; raw_dir.rename(raw_copy)
        raw_dir.symlink_to(raw_copy, target_is_directory=True)
        denied = subprocess.run([sys.executable, str(BASE / "produce.py"), "--binary", str(self.binary), "--snapshot", str(symlink_snapshot),
                                 "--source-sha", SOURCE_SHA, "--output", str(self.workspace / "parent-symlink")], cwd=ROOT, capture_output=True, text=True, timeout=30)
        self.assertNotEqual(denied.returncode, 0)
        policy_snapshot = self.workspace / "policy-snapshot"; shutil.copytree(BASE / "snapshot", policy_snapshot)
        policy_path = policy_snapshot / "acquisition-policy.json"; policy = json.loads(policy_path.read_text()); policy["network"]["credentials"] = True
        policy_path.write_text(json.dumps(policy))
        denied = subprocess.run([sys.executable, str(BASE / "produce.py"), "--binary", str(self.binary), "--snapshot", str(policy_snapshot),
                                 "--source-sha", SOURCE_SHA, "--output", str(self.workspace / "policy")], cwd=ROOT, capture_output=True, text=True, timeout=30)
        self.assertNotEqual(denied.returncode, 0)
        run_dir = self.fresh_handoff("forged")
        self.assertEqual(self.run_recipient(run_dir).returncode, 0)
        report = json.loads((run_dir / "report.json").read_text()); report["container_id"] = "0" * 32
        (run_dir / "report.json").write_text(json.dumps(report))
        result = subprocess.run([sys.executable, str(BASE / "evaluate.py"), "--report", str(run_dir / "report.json"),
                                 "--manifest", str(run_dir / "manifest.json"), "--evaluation", str(BASE / "evaluation.json"),
                                 "--receipt", str(self.workspace / "handoff-receipt.json"), "--output", str(self.workspace / "forged-eval.json")], cwd=ROOT, capture_output=True, text=True, timeout=30)
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
