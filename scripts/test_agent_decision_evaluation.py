"""Expose the offline example checks to the existing Python verification gate."""
import pathlib
import sys
import unittest

EXAMPLE = pathlib.Path(__file__).resolve().parents[1] / "examples/agent-decision-evaluation"

def load_tests(loader, tests, pattern):
    sys.path.insert(0, str(EXAMPLE))
    return loader.loadTestsFromName("test_harness")
