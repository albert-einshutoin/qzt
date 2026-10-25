import copy
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock
from pathlib import Path

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("harness", ROOT / "harness.py")
h = importlib.util.module_from_spec(spec)
spec.loader.exec_module(h)

class EvidenceTests(unittest.TestCase):
    def test_quote_is_checked_against_original_bytes(self):
        data = "条件付き\n採用を保留\n".encode()
        cite = h.citation("d01.txt", data, "採用を保留")
        self.assertTrue(h.check_citation(cite, {"d01.txt": data})["valid"])
        for field, value in [("start", 0), ("length", 1), ("sha256", "0"*64),
                             ("quote", "本採用済み"), ("doc_id", "../oracle")]:
            broken = dict(cite, **{field: value})
            self.assertFalse(h.check_citation(broken, {"d01.txt": data})["valid"])
        self.assertFalse(h.check_citation(dict(cite, start=True), {"d01.txt": data})["valid"])

    def test_partial_unknown_is_not_absence_or_error(self):
        base = {"capped": False, "stop_reason": None, "index_complete_declared": True,
                "index_coverage_verified": False, "incomplete_reason": None, "hits": []}
        self.assertEqual(h.search_state(base), "coverage_unknown")
        for extra in [{"capped": True, "stop_reason": "max_search_results"},
                      {"index_complete_declared": False},
                      {"incomplete_reason": "query_shorter_than_ngram_n"}]:
            self.assertEqual(h.search_state(dict(base, **extra)), "partial_unknown")
        for extra in [{"incomplete_reason": "new_unknown"}, {"capped": True},
                      {"index_coverage_verified": True}]:
            with self.assertRaises(ValueError):
                h.search_state(dict(base, **extra))

    def test_path_boundary_and_corrupt_snapshot(self):
        h.snapshot(ROOT / "snapshot")
        with tempfile.TemporaryDirectory() as t:
            out = Path(t)
            (out / "ok").write_bytes(b"good")
            (out / "link").symlink_to(ROOT)
            for path in ["../outside", "/etc/passwd", "link/PLAN.ja.md"]:
                with self.assertRaises(ValueError):
                    h.safe_read(out, path)
            import shutil
            shutil.copytree(ROOT / "snapshot", out / "snapshot")
            (out / "snapshot/body/d01.txt").write_bytes(b"corrupt")
            with self.assertRaises(ValueError):
                h.snapshot(out / "snapshot")

    def test_bounds_and_submission_schema(self):
        h.budget({"started_epoch": 100, "seconds": 600, "accesses": 40}, 39, 699)
        for count, now in [(40, 699), (0, 701)]:
            with self.assertRaises(ValueError):
                h.budget({"started_epoch": 100, "seconds": 600, "accesses": 40}, count, now)
        answer = {"answers": [{"id": "q1", "status": "unknown", "answer": "不明", "citations": []}]}
        h.validate_answer(answer, ["q1"])
        for mutated in [dict(answer, verified=True), {"answers": answer["answers"]*2}]:
            with self.assertRaises(ValueError):
                h.validate_answer(mutated, ["q1"])

    def test_missing_invalid_and_explicit_unknown_are_distinct(self):
        with tempfile.TemporaryDirectory() as t:
            out = Path(t)
            state = h.submission(out, ["q1"])
            self.assertEqual((state["trial_status"], state["answer"]["answers"][0]["status"]), ("missing", "missing"))
            (out/"answer.json").write_text("not JSON")
            self.assertEqual(h.submission(out, ["q1"])["trial_status"], "invalid")
            answer = {"answers": [{"id": "q1", "status": "unknown", "answer": "不明", "citations": []}]}
            (out/"answer.json").write_text(json.dumps(answer))
            self.assertEqual(h.submission(out, ["q1"])["trial_status"], "invalid")
            h.save(out/"submission.json", {"epoch": 100, "late": False, "answer_sha256": h.sha((out/"answer.json").read_bytes())})
            state = h.submission(out, ["q1"])
            self.assertEqual((state["trial_status"], state["answer"]["answers"][0]["status"]), ("submitted", "unknown"))

    def test_cli_timeout_preserves_call_and_partial_bytes(self):
        with mock.patch.object(h.subprocess, "run", side_effect=subprocess.TimeoutExpired(["qzt"], 30, output=b"partial", stderr=b"diagnostic")):
            result = h.run_cli(["qzt"], ROOT)
        self.assertTrue(result["timeout"])
        self.assertEqual(result["argv"], ["qzt"])
        self.assertIsNone(result["exit"])
        self.assertEqual(bytes.fromhex(result["stdout_hex"]), b"partial")
        self.assertEqual(bytes.fromhex(result["stderr_hex"]), b"diagnostic")

    def test_all_missing_trials_reach_audit_and_anonymous_judge_input(self):
        from audit import audit
        from judge import prepare
        with tempfile.TemporaryDirectory() as t:
            runtime = Path(t)
            plan = {"trials": [{"trial": f"{c}{i}", "condition": c} for i in range(1, 4) for c in "ABC"]}
            h.save(runtime/"run-plan.json", plan)
            h.save(runtime/"freeze.json", {"files": []})
            for item in plan["trials"]:
                out = runtime/"trials"/item["trial"]/"output"
                (out/"access").mkdir(parents=True)
                h.save(out/"activation.json", {"started_epoch": time.time()})
            result = audit(runtime)
            self.assertEqual(len(result["trials"]), 9)
            self.assertTrue(all(t["trial_status"] == "missing" and len(t["missing_questions"]) == 9 for t in result["trials"]))
            prepare(runtime)
            common = json.loads((runtime/"anonymous-input.json").read_text())
            self.assertEqual(len(common["answers"]), 9)
            self.assertTrue(all(a["trial_status"] == "missing" for a in common["answers"]))

    def test_logged_gateway_e2e_when_prepared(self):
        runtime = os.environ.get("QZT_AGENT_RUNTIME")
        if not runtime:
            self.skipTest("QZT_AGENT_RUNTIME required for prepared CLI E2E")
        with tempfile.TemporaryDirectory() as t:
            for name in ("A1", "B1", "C1"):
                trial = Path(t) / name
                shutil.copytree(Path(runtime)/"trials"/name, trial)
                h.save(trial/"output/activation.json", {"started_epoch": time.time()})
                def call(*args, ok=True):
                    r = subprocess.run([sys.executable, str(ROOT/"access.py"), str(trial), *args], capture_output=True, timeout=35)
                    self.assertEqual(r.returncode == 0, ok, r.stderr.decode())
                    return json.loads(r.stdout) if ok else None
                call("info")
                if name == "A1":
                    call("doc", "d01.txt", ok=False)
                    call("search", "anything", ok=False)
                    for i in range(4, 41):
                        h.save(trial/"output/access"/f"{i:02d}.json", {})
                    call("info", ok=False)
                    self.assertEqual(len(list((trial/"output/access").glob("*.json"))), 40)
                    self.assertEqual(len(list((trial/"output/denied").glob("*.json"))), 1)
                    (trial/"output/activation.json").write_text(json.dumps({"started_epoch": time.time()-601}))
                    call("info", ok=False)
                    self.assertEqual(len(list((trial/"output/denied").glob("*.json"))), 2)
                    continue
                search = call("search", "main")
                if name == "C1":
                    call("doc", "d01.txt", ok=False)
                    hit = search["hits"][0]
                    context = call("context", str(hit["logical_offset"]), str(hit["byte_length"]))
                    docid = context["document"]["id"]
                else:
                    docid = "d01.txt"
                doc = call("doc", docid)
                cite = call("cite", docid, "main")
                self.assertTrue(h.check_citation(cite, {docid: doc["text"].encode()})["valid"])
                call("cite", "../rubric.json", "main", ok=False)
                if name == "C1":
                    (trial/"input/evidence.qzi").write_bytes(b"broken")
                    call("search", "main", ok=False)
                    self.assertEqual((trial/"input/evidence.qzi").read_bytes(), b"broken")

if __name__ == "__main__":
    unittest.main()
