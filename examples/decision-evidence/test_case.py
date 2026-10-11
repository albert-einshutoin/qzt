"""Independent fixture oracle; recipient never receives this module or fixture."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

BASE = Path(__file__).resolve().parent
BINARY = Path(os.environ['QZT_DECISION_BINARY']).resolve()


def sha(data):
    return hashlib.sha256(data).hexdigest()


class DecisionEvidenceChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bundle = self.root / 'recipient'
        run = subprocess.run([sys.executable, str(BASE / 'produce.py'), '--binary', str(BINARY),
                              '--fixture', str(BASE / 'fixture'), '--output', str(self.bundle)],
                             capture_output=True, timeout=60)
        self.record('producer', run)
        self.assertEqual(run.returncode, 0, run.stderr.decode())
        self.manifest_bytes = (self.bundle / 'manifest.json').read_bytes()
        self.manifest = json.loads(self.manifest_bytes)
        self.seal = sha(self.manifest_bytes)

    def record(self, label, result):
        path = os.environ.get('QZT_DECISION_TEST_RECORD')
        if path:
            with Path(path).open('a', encoding='utf-8') as handle:
                handle.write(json.dumps({'test': self.id(), 'label': label, 'command': result.args,
                                         'cwd': str(self.bundle), 'exit_code': result.returncode,
                                         'stdout_sha256': sha(result.stdout), 'stderr_utf8': result.stderr.decode()},
                                        ensure_ascii=False) + '\n')

    def recipient(self, manifest=None, output='report.json', log='commands.json', seal=None):
        if manifest is not None:
            data = (json.dumps(manifest, ensure_ascii=False, indent=2) + '\n').encode()
            (self.bundle / 'manifest.json').write_bytes(data)
            seal = sha(data) if seal is None else seal
        result = subprocess.run([sys.executable, 'recipient.py', '--manifest-sha256', seal or self.seal,
                               '--output', output, '--log', log], cwd=self.bundle,
                              capture_output=True, timeout=60)
        self.record(output, result)
        return result

    def test_recipient_restores_fixed_evidence_without_plaintext_or_oracle(self):
        self.assertEqual({p.name for p in self.bundle.iterdir()},
                         {'evidence.qzt', 'manifest.json', 'summary.txt', 'qzt.exe' if os.name == 'nt' else 'qzt', 'recipient.py'})
        result = self.recipient()
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        report = json.loads((self.bundle / 'report.json').read_text())
        questions = json.loads((BASE / 'fixture/questions.json').read_text())
        originals = {p.name: p.read_bytes() for p in (BASE / 'fixture/records').iterdir()}
        offsets = {}; total = 0
        for name, data in sorted(originals.items()):
            offsets[name] = total; total += len(data)
        for expected, actual in zip(questions, report['questions']):
            self.assertEqual(actual['id'], expected['id'])
            self.assertEqual(actual['outcome'], 'unknown' if expected['kind'] == 'unknown' else 'verified_evidence')
            self.assertEqual(len(actual['evidence']), len(expected['docs']))
            for name, evidence in zip(expected['docs'], actual['evidence']):
                self.assertEqual(evidence['doc_id'], 'decision/' + name)
                self.assertEqual(evidence['logical_offset'], offsets[name])
                self.assertEqual(evidence['byte_length'], len(originals[name]))
                self.assertEqual(evidence['text_utf8'].encode(), originals[name])
                self.assertTrue(evidence['expected_checksum_verified'])
        search_question = report['questions'][4]
        self.assertFalse(search_question['search']['index_coverage_verified'])
        context = search_question['context']
        self.assertEqual(context['document']['id'], 'decision/05-change.txt')
        excerpt = context['excerpt']; start = excerpt['logical_offset'] - offsets['05-change.txt']
        self.assertEqual(bytes.fromhex(excerpt['bytes_hex']),
                         originals['05-change.txt'][start:start + excerpt['byte_length']])
        self.assertEqual([q['id'] for q in report['questions'] if q['outcome'] == 'unknown'], ['q7', 'q8'])
        oracle = json.loads((BASE / 'fixture/oracle.json').read_text())
        self.assertEqual(oracle['answers']['q7'], 'unknown')
        self.assertEqual(oracle['answers']['q8'], 'unknown')

    def test_summary_update_and_sidecar_rebuild_preserve_original_refs_and_outputs(self):
        first = self.recipient(); self.assertEqual(first.returncode, 0, first.stderr.decode())
        original = {n: (self.bundle / n).read_bytes() for n in ['evidence.qzt', 'manifest.json', 'report.json', 'commands.json']}
        sidecar = (self.bundle / 'evidence.qzi').read_bytes()
        (self.bundle / 'summary.txt').write_bytes((BASE / 'fixture/summary-updated.txt').read_bytes())
        (self.bundle / 'evidence.qzi').unlink()
        second = self.recipient(output='report-updated.json', log='commands-updated.json')
        self.assertEqual(second.returncode, 0, second.stderr.decode())
        self.assertEqual((self.bundle / 'evidence.qzi').read_bytes(), sidecar)
        for name, data in original.items(): self.assertEqual((self.bundle / name).read_bytes(), data)
        reports = [json.loads((self.bundle / name).read_text()) for name in ['report.json', 'report-updated.json']]
        for a, b in zip(reports[0]['questions'], reports[1]['questions']):
            self.assertEqual(a['evidence'], b['evidence']); self.assertEqual(a['outcome'], b['outcome'])
        self.assertNotEqual(reports[0]['summary_sha256'], reports[1]['summary_sha256'])
        before = {p.name: p.read_bytes() for p in self.bundle.iterdir()}
        denied = self.recipient(); self.assertEqual(denied.returncode, 1)
        self.assertEqual({p.name: p.read_bytes() for p in self.bundle.iterdir()}, before)
        producer = subprocess.run([sys.executable, str(BASE / 'produce.py'), '--binary', str(BINARY),
                                  '--fixture', str(BASE / 'fixture'), '--output', str(self.bundle)],
                                 capture_output=True, timeout=60)
        self.record('producer-existing-destination', producer)
        self.assertEqual(producer.returncode, 1)
        self.assertEqual({p.name: p.read_bytes() for p in self.bundle.iterdir()}, before)

    def test_invalid_references_and_required_evidence_fail_without_success_output(self):
        cases = []
        bad = copy.deepcopy(self.manifest); bad['container_id'] = 'ff' * 16; cases.append(bad)
        for key, value in [('doc_id', 'decision/missing.txt'), ('logical_offset', 1),
                           ('byte_length', 0), ('byte_length', 2**64), ('logical_offset', True)]:
            bad = copy.deepcopy(self.manifest); bad['questions'][0]['references'][0][key] = value; cases.append(bad)
        bad = copy.deepcopy(self.manifest); bad['questions'][0]['references'][0]['checksum']['value'] = '00' * 32; cases.append(bad)
        bad = copy.deepcopy(self.manifest); bad['documents'][0]['checksum']['value'] = '00' * 32; bad['questions'][0]['references'][0]['checksum']['value'] = '00' * 32; cases.append(bad)
        bad = copy.deepcopy(self.manifest); bad['questions'][0]['references'] = []; cases.append(bad)
        bad = copy.deepcopy(self.manifest); bad['questions'][0]['kind'] = 'maybe'; cases.append(bad)
        bad = copy.deepcopy(self.manifest); bad['questions'].pop(); cases.append(bad)
        bad = copy.deepcopy(self.manifest); bad['questions'][4]['search']['query'] = '未記録値999'; bad['questions'][4]['search']['byte_length'] = len('未記録値999'.encode()); cases.append(bad)
        for index, case in enumerate(cases):
            with self.subTest(index=index):
                result = self.recipient(case, f'bad-{index}.json', f'bad-log-{index}.json')
                self.assertEqual(result.returncode, 1, result.stderr.decode())
                self.assertEqual(result.stdout, b'')
                self.assertFalse((self.bundle / f'bad-{index}.json').exists())
        result = self.recipient(self.manifest, 'untrusted.json', 'untrusted-log.json', seal='00' * 32)
        self.assertEqual(result.returncode, 1); self.assertFalse((self.bundle / 'untrusted.json').exists())

    def test_corrupt_missing_qzt_and_untrusted_search_states_fail(self):
        path = self.bundle / 'evidence.qzt'; data = path.read_bytes()
        altered = bytearray(data); altered[-1] ^= 1; path.write_bytes(altered)
        bad = self.recipient(); self.assertEqual(bad.returncode, 1); self.assertFalse((self.bundle / 'report.json').exists())
        path.unlink()
        missing = self.recipient(output='missing.json', log='missing-log.json')
        self.assertEqual(missing.returncode, 1); self.assertFalse((self.bundle / 'missing.json').exists())
        spec = importlib.util.spec_from_file_location('decision_recipient', BASE / 'recipient.py')
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
        for report in [dict(capped=True, incomplete_reason=None, index_coverage_verified=False),
                       dict(capped=False, incomplete_reason='query_shorter_than_ngram_n', index_coverage_verified=False),
                       dict(capped=False, incomplete_reason=None),
                       dict(capped=False, incomplete_reason=None, index_coverage_verified=True)]:
            with self.assertRaises(RuntimeError): module.check_search_status(report)


if __name__ == '__main__': unittest.main()
