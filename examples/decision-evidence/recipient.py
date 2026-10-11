#!/usr/bin/env python3
"""Byte verification only. Contains no fixture, answers, or semantic inference."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_small(path):
    require(not path.is_symlink() and path.is_file(), f'regular file required: {path}')
    with path.open('rb') as handle:
        data = handle.read(65537)
    require(len(data) <= 65536, f'input exceeds 64KiB: {path}')
    return data


def write_new(path, value):
    data = (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode()
    with path.open('xb') as handle:
        handle.write(data)


class Runner:
    def __init__(self, binary, cwd):
        self.binary = binary.resolve()
        self.cwd = cwd
        self.records = []

    def call(self, *args):
        command = [str(self.binary), *map(str, args)]
        started = time.perf_counter()
        try:
            result = subprocess.run(command, cwd=self.cwd, capture_output=True, timeout=30, check=False)
        except subprocess.TimeoutExpired as error:
            self.records.append({'command': command, 'exit_code': None, 'stop': 'timeout',
                                 'stdout_hex': (error.stdout or b'').hex(), 'stderr_hex': (error.stderr or b'').hex()})
            raise RuntimeError('CLI timeout (30s)') from error
        self.records.append({'command': command, 'exit_code': result.returncode,
                             'wall_ms': round((time.perf_counter() - started) * 1000, 3),
                             'stdout_utf8': result.stdout.decode('utf-8'),
                             'stderr_utf8': result.stderr.decode('utf-8')})
        require(result.returncode == 0, f'CLI failed: {args[0]} exit={result.returncode}')
        return result.stdout

    def json(self, *args):
        return json.loads(self.call(*args))


def checksum_valid(value):
    return (isinstance(value, dict) and set(value) == {'algorithm', 'value'}
            and value['algorithm'] == 'sha256' and isinstance(value['value'], str)
            and re.fullmatch('[0-9a-f]{64}', value['value']) is not None)


def validate_manifest(value):
    require(isinstance(value, dict) and set(value) == {'schema', 'fictional', 'container_id', 'documents', 'questions'}, 'manifest fields')
    require(value['schema'] == 'qzt-decision-evidence-v1' and value['fictional'] is True, 'fixed fictional case required')
    require(isinstance(value['container_id'], str) and re.fullmatch('[0-9a-f]{32}', value['container_id']), 'container id')
    docs = value['documents']; require(isinstance(docs, list) and len(docs) == 7, 'seven documents required')
    by_id = {}; offset = 0
    for doc in docs:
        require(isinstance(doc, dict) and set(doc) == {'doc_id', 'logical_offset', 'byte_length', 'checksum'}, 'document fields')
        require(isinstance(doc['doc_id'], str) and doc['doc_id'] and doc['doc_id'] not in by_id, 'unique doc id')
        require(type(doc['logical_offset']) is int and doc['logical_offset'] == offset, 'ordered document offset')
        require(type(doc['byte_length']) is int and 0 < doc['byte_length'] <= 65536, 'document byte length')
        require(checksum_valid(doc['checksum']), 'SHA-256 expectation required')
        offset += doc['byte_length']; require(offset <= 65536, 'fixed small corpus only')
        by_id[doc['doc_id']] = doc
    questions = value['questions']; require(isinstance(questions, list) and len(questions) == 8, 'eight questions required')
    seen = set(); referenced = set()
    for question in questions:
        require(isinstance(question, dict) and set(question) in [
            {'id', 'question', 'kind', 'references'}, {'id', 'question', 'kind', 'references', 'search'}], 'question fields')
        require(isinstance(question['id'], str) and question['id'] not in seen, 'unique question id')
        require(isinstance(question['question'], str) and question['question'], 'question text')
        seen.add(question['id'])
        refs = question['references']; require(isinstance(refs, list), 'reference array')
        if question['kind'] == 'unknown':
            require(not refs and 'search' not in question, 'unknown cannot claim evidence')
        else:
            require(question['kind'] == 'evidence' and refs, 'required evidence missing or unknown kind')
            for ref in refs:
                require(isinstance(ref, dict) and ref == by_id.get(ref.get('doc_id')), 'reference doc/range/checksum mismatch')
                # Equality alone would accept True == 1 in JSON numeric coordinates.
                require(type(ref['logical_offset']) is int and type(ref['byte_length']) is int, 'integer reference coordinates')
                referenced.add(ref['doc_id'])
        if 'search' in question:
            search = question['search']
            require(isinstance(search, dict) and set(search) == {'query', 'doc_id', 'logical_offset', 'byte_length'}, 'search fields')
            require(isinstance(search['query'], str) and len(search['query'].encode()) >= 3, 'search query')
            require(search['doc_id'] in {r['doc_id'] for r in refs}, 'search outside required evidence')
            doc = by_id[search['doc_id']]
            require(type(search['logical_offset']) is int and type(search['byte_length']) is int
                    and search['byte_length'] == len(search['query'].encode())
                    and doc['logical_offset'] <= search['logical_offset']
                    and search['logical_offset'] + search['byte_length'] <= doc['logical_offset'] + doc['byte_length'], 'search range')
    require(referenced == set(by_id), 'document evidence missing')
    require(sum('search' in q for q in questions) >= 1, 'search/context case missing')
    return by_id


def check_search_status(report):
    require(report.get('capped') is False and report.get('stop_reason', 'missing') is None
            and report.get('incomplete_reason', 'missing') is None
            and report.get('index_coverage_verified') is False, 'search capped/incomplete or coverage contract unknown')


def restore(runner, manifest, summary):
    docs = validate_manifest(manifest)
    info = runner.json('info', 'evidence.qzt', '--format', 'json')
    require(info['container_id'] == manifest['container_id'] and info['document_index'] is True, 'container/index mismatch')
    stored = runner.json('docs', 'evidence.qzt', '--format', 'json')['documents']
    require(len(stored) == len(docs), 'stored document count mismatch')
    for actual, expected in zip(stored, docs.values()):
        require(all(actual[key] == expected[key] for key in ['doc_id', 'logical_offset', 'byte_length']), 'Document Index coordinates mismatch')
    verified = runner.json('verify', 'evidence.qzt', '--deep', '--format', 'json')
    require(verified['ok'] is True and verified['original_checksum_verified'] is True
            and verified['document_index_status'] == 'source_checked', 'Deep verification incomplete')
    restored = {}
    for doc in docs.values():
        data = runner.call('doc', 'evidence.qzt', doc['doc_id'])
        end = doc['logical_offset'] + doc['byte_length']
        ranged = runner.call('range', 'evidence.qzt', '--bytes', f"{doc['logical_offset']}:{end}")
        require(len(data) == doc['byte_length'] and sha(data) == doc['checksum']['value'] and ranged == data, 'trusted expected bytes differ')
        restored[doc['doc_id']] = data
    sidecar = Path('evidence.qzi')
    require(not sidecar.is_symlink(), 'sidecar symlink refused')
    if not sidecar.exists():
        runner.call('sidecar-rebuild', 'evidence.qzt', '-o', 'evidence.qzi', '--index', 'ngram', '--ngram', 3)
    result = {'schema': 'qzt-decision-recovery-report-v1', 'summary_utf8': summary.decode(),
              'summary_sha256': sha(summary), 'container_id': manifest['container_id'],
              'deep_verify': verified, 'questions': []}
    for question in manifest['questions']:
        entry = {'id': question['id'], 'question': question['question'],
                 'outcome': 'unknown' if question['kind'] == 'unknown' else 'verified_evidence', 'evidence': []}
        for ref in question['references']:
            entry['evidence'].append({**ref, 'expected_checksum_verified': True, 'text_utf8': restored[ref['doc_id']].decode()})
        if 'search' in question:
            expected = question['search']
            search = runner.json('search', 'evidence.qzt', expected['query'], '--sidecar', 'evidence.qzi', '--max-results', 100, '--format', 'json')
            check_search_status(search)
            require(len(search['hits']) == 1, 'fixed expected search hit missing/changed; no absence proof')
            hit = search['hits'][0]
            require(hit['source'] == 'verified_original_bytes' and hit['logical_offset'] == expected['logical_offset']
                    and hit['byte_length'] == expected['byte_length'], 'search hit mismatch')
            doc = docs[expected['doc_id']]; local = hit['logical_offset'] - doc['logical_offset']
            require(restored[doc['doc_id']][local:local + hit['byte_length']] == expected['query'].encode(), 'query bytes differ')
            context = runner.json('context', 'evidence.qzt', '--offset', hit['logical_offset'], '--length', hit['byte_length'],
                                  '--before', 2, '--after', 2, '--format', 'json')
            require(context['mapping_status'] == 'unique' and context['document']['id'] == doc['doc_id']
                    and context['document']['local_offset'] == local
                    and context['document']['local_end'] == local + hit['byte_length'], 'context document/range mismatch')
            excerpt = context['excerpt']; start = excerpt['logical_offset'] - doc['logical_offset']; end = start + excerpt['byte_length']
            require(0 <= start <= local and local + hit['byte_length'] <= end <= doc['byte_length'], 'context outside verified document')
            require(bytes.fromhex(excerpt['bytes_hex']) == restored[doc['doc_id']][start:end], 'context bytes mismatch')
            require(context['before']['stop'] in ['complete', 'scope_boundary'] and context['after']['stop'] in ['complete', 'scope_boundary']
                    and excerpt['leading_fragment'] is False and excerpt['trailing_fragment'] is False, 'partial context')
            entry.update(search=search, context=context)
        result['questions'].append(entry)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest-sha256', required=True, help='expectation delivered through a trusted channel')
    parser.add_argument('--output', default='report.json')
    parser.add_argument('--log', default='commands.json')
    args = parser.parse_args()
    output = Path(args.output); log = Path(args.log)
    runner = None
    try:
        require(output.name == args.output and log.name == args.log and output != log, 'fresh output basenames only')
        require(not output.exists() and not output.is_symlink() and not log.exists() and not log.is_symlink(), 'existing output/log protected')
        require(re.fullmatch('[0-9a-f]{64}', args.manifest_sha256), 'manifest expectation must be SHA-256')
        data = read_small(Path('manifest.json'))
        require(sha(data) == args.manifest_sha256, 'untrusted manifest bytes')
        manifest = json.loads(data)
        summary = read_small(Path('summary.txt'))
        require(not Path('evidence.qzt').is_symlink() and Path('evidence.qzt').is_file(), 'QZT missing or symlink')
        binary = Path('qzt.exe' if os.name == 'nt' else 'qzt')
        require(not binary.is_symlink() and binary.is_file(), 'local executable required')
        runner = Runner(binary, Path.cwd())
        result = restore(runner, manifest, summary)
        write_new(log, runner.records)
        write_new(output, result)
        return 0
    except (RuntimeError, OSError, ValueError, KeyError, TypeError) as error:
        if runner is not None and not log.exists():
            write_new(log, runner.records)
        print(f'Error: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__': sys.exit(main())
