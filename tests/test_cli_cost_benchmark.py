"""Small subprocess-failure checks for the opt-in #295 benchmark harness."""

import concurrent.futures
import importlib.util
import io
import json
import pathlib
import sys
import time
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "scripts/cli-cost-benchmark.py"
SPEC = importlib.util.spec_from_file_location("cli_cost_benchmark", SCRIPT)
BENCHMARK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BENCHMARK)


class FailureAccountingTests(unittest.TestCase):
    def test_elapsed_deadline_does_not_launch(self):
        result = BENCHMARK.run([sys.executable, "-c", "print('unexpected')"],
                               2, time.monotonic() - 1)
        self.assertEqual(result["status"], "deadline-exceeded")
        self.assertIsNone(result["spawned_ns"])

    def test_in_flight_child_is_killed_at_overall_deadline(self):
        started = time.monotonic()
        result = BENCHMARK.run([sys.executable, "-c", "import time; time.sleep(10)"],
                               5, started + 0.1)
        self.assertEqual(result["status"], "deadline-timeout")
        self.assertLess(time.monotonic() - started, 2)

    def test_spawn_failure_and_successful_sibling_are_both_recorded(self):
        commands = [["/nonexistent/qzt295-probe"],
                    [sys.executable, "-c", "print('ok')"]]
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(lambda command: BENCHMARK.run(command, 2), commands))
        log = io.StringIO()
        for ordinal, result in enumerate(results):
            BENCHMARK.record(log, "cli", f"attempt-{ordinal}", result)
        rows = [json.loads(line) for line in log.getvalue().splitlines()]
        self.assertEqual([row["status"] for row in rows], ["spawn-error", "exit-0"])
        self.assertEqual(rows[1]["stdout"], "ok\n")
        self.assertEqual(BENCHMARK.peak_overlap(results), 1)


if __name__ == "__main__":
    unittest.main()
