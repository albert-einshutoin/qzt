"""Expose small offline scale-harness checks to the existing Python gate."""
import importlib.util
import pathlib
import unittest

def load_tests(loader,tests,pattern):
    path=pathlib.Path(__file__).resolve().parents[1]/'examples/gib-scale-evaluation/test_scale.py'
    spec=importlib.util.spec_from_file_location('gib_scale_tests',path);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return loader.loadTestsFromModule(module)
