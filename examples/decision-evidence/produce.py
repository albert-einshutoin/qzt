#!/usr/bin/env python3
"""Freeze source-derived expectations before pack; hand off no plaintext oracle."""
import argparse
import json
import os
from pathlib import Path
import shutil
import sys

from recipient import Runner, read_small, require, sha, validate_manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--fixture', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    runner = None; record = args.output.with_name(args.output.name + '-producer.jsonl')
    try:
        require(not args.output.exists() and not args.output.is_symlink() and not record.exists(), 'fresh producer destination required')
        sources = sorted((args.fixture / 'records').glob('*.txt'))
        require(len(sources) == 7, 'seven fictional documents required')
        questions = json.loads(read_small(args.fixture / 'questions.json'))
        oracle = json.loads(read_small(args.fixture / 'oracle.json'))
        require(oracle['fictional'] is True and oracle['fixed_before_pack'] is True and len(oracle['answers']) == 8, 'fixed fictional oracle required')
        require(set(oracle['answers']) == {q['id'] for q in questions}, 'oracle question identities')
        docs = []; by_name = {}; raw = {}; offset = 0
        for path in sources:
            data = read_small(path); data.decode('utf-8')
            doc = {'doc_id': 'decision/' + path.name, 'logical_offset': offset, 'byte_length': len(data),
                   'checksum': {'algorithm': 'sha256', 'value': sha(data)}}
            docs.append(doc); by_name[path.name] = doc; raw[path.name] = data; offset += len(data)
        require(offset <= 65536, 'small fixed corpus required')
        entries = []
        for question in questions:
            entry = {k: question[k] for k in ['id', 'question', 'kind']}
            entry['references'] = [by_name[name] for name in question['docs']]
            if 'search' in question:
                search = question['search']; data = raw[search['doc']]; query = search['query'].encode()
                require(data.count(query) == 1, 'fixed query occurrence must be unique in source')
                entry['search'] = {'query': search['query'], 'doc_id': by_name[search['doc']]['doc_id'],
                                   'logical_offset': by_name[search['doc']]['logical_offset'] + data.index(query), 'byte_length': len(query)}
            entries.append(entry)
        manifest = {'schema': 'qzt-decision-evidence-v1', 'fictional': True, 'container_id': '00' * 16,
                    'documents': docs, 'questions': entries}
        validate_manifest(manifest)
        # Persist expected coordinates/digests before any QZT call. No restored-byte oracle.
        binary_hash = sha(args.binary.read_bytes())
        frozen = {'event': 'expectations_before_pack', 'manifest': manifest, 'fixture_bytes': offset,
                  'binary_sha256': binary_hash,
                  'question_file_sha256': sha(read_small(args.fixture / 'questions.json')),
                  'oracle_file_sha256': sha(read_small(args.fixture / 'oracle.json'))}
        with record.open('x', encoding='utf-8') as handle:
            handle.write(json.dumps(frozen, ensure_ascii=False) + '\n')
        args.output.mkdir()
        runner = Runner(args.binary, args.output.resolve())
        runner.call('pack-docs', *[p.resolve() for p in sources], '--doc-id-prefix', 'decision/', '-o', 'evidence.qzt')
        info = runner.json('info', 'evidence.qzt', '--format', 'json')
        manifest['container_id'] = info['container_id']
        exported = runner.call('export', 'evidence.qzt')
        require(exported == b''.join(raw[p.name] for p in sources), 'export differs from prepack originals')
        (args.output / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        (args.output / 'summary.txt').write_bytes(read_small(args.fixture / 'summary.txt'))
        shutil.copy2(Path(__file__).with_name('recipient.py'), args.output / 'recipient.py')
        executable = args.output / ('qzt.exe' if os.name == 'nt' else 'qzt')
        shutil.copy2(args.binary, executable)
        require(sha(executable.read_bytes()) == binary_hash and sha(args.binary.read_bytes()) == binary_hash, 'binary changed during producer run')
        with record.open('a', encoding='utf-8') as handle:
            handle.write(json.dumps({'event': 'producer_complete', 'manifest_sha256': sha((args.output / 'manifest.json').read_bytes()),
                                     'binary_sha256': binary_hash, 'records': runner.records}, ensure_ascii=False) + '\n')
        print(sha((args.output / 'manifest.json').read_bytes()))
        return 0
    except (RuntimeError, OSError, ValueError, KeyError, TypeError) as error:
        if runner is not None:
            with record.open('a', encoding='utf-8') as handle:
                handle.write(json.dumps({'event': 'producer_error', 'error': str(error), 'records': runner.records}, ensure_ascii=False) + '\n')
        print(f'Error: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__': sys.exit(main())
