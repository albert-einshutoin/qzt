import hashlib
import json
import pathlib
import re
import subprocess
import time


root = pathlib.Path('/private/tmp/qzt27-ngram-20260926-fbef491')
source = root / 'inputs/corpus.qzt'
binaries = {kind: root / f'binaries/qzt-{kind}' for kind in ('before', 'after')}
orders = [('before', 'warmup'), ('after', 'warmup')]
orders += [(kind, f'measured-{index}') for index, kind in enumerate(
    ['before', 'after', 'after', 'before', 'before', 'after'], 1)]
records = []
result_path = root / 'raw/paired-results-final.json'

for kind, trial in orders:
    output = root / f'outputs/final-{kind}-{trial}.qzi'
    if output.exists():
        raise RuntimeError(f'output must be fresh: {output}')
    command = ['/usr/bin/time', '-l', str(binaries[kind]), 'sidecar-rebuild',
               str(source), '-o', str(output), '--index', 'ngram', '--ngram', '3']
    start = time.monotonic()
    try:
        process = subprocess.run(command, capture_output=True, text=True, timeout=120)
        timeout = False
    except subprocess.TimeoutExpired as error:
        process = error
        timeout = True
    wall = time.monotonic() - start
    stderr = process.stderr or ''
    if isinstance(stderr, bytes):
        stderr = stderr.decode(errors='replace')
    match = re.search(r'^\s*(\d+)\s+maximum resident set size$', stderr, re.M)
    record = {
        'kind': kind, 'trial': trial, 'command': command, 'wall_s': round(wall, 4),
        'timeout': timeout, 'exit_code': None if timeout else process.returncode,
        'rss_bytes': int(match.group(1)) if match else None,
        'stdout': None if timeout else process.stdout, 'stderr': stderr,
        'qzi_bytes': output.stat().st_size if output.exists() else None,
        'qzi_sha256': hashlib.sha256(output.read_bytes()).hexdigest()
        if output.exists() else None,
    }
    records.append(record)
    result_path.write_text(json.dumps(records, indent=2, ensure_ascii=False) + '\n')
    print(kind, trial, record['exit_code'], record['rss_bytes'],
          record['wall_s'], record['qzi_sha256'], flush=True)
    if timeout or process.returncode != 0 or match is None or not output.exists():
        raise RuntimeError(f'failed trial: {kind} {trial}')
