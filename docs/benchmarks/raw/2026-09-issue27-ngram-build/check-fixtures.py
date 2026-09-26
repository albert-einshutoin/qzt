import hashlib
import json
import pathlib
import random
import subprocess


root = pathlib.Path('/private/tmp/qzt27-ngram-20260926-fbef491')
before = root / 'binaries/qzt-before'
after = root / 'binaries/qzt-after'
cases = {
    'empty': b'',
    'shorter_than_n': b'a\nb\n',
    'repeated_lf': b'aaaaa\naaa\nbbb\n',
    'crlf_no_final_lf': b'one\r\ntwo\nlast',
    'unicode': '東京大学\nabc東京xyz\n'.encode(),
    'continuation': b'x' * 100 + b'\n' + b'y' * 34,
}
randomizer = random.Random(27)
vocabulary = ['alpha', 'beta', 'gamma', 'delta', 'omega', '東京', '証拠']
cases['diverse_seed27'] = ''.join(
    f'{index}: {vocabulary[randomizer.randrange(len(vocabulary))]} '
    f'{randomizer.randrange(100000):05d}\n'
    for index in range(256)
).encode()
results = []

def run(*args):
    command = [str(args[0]), *map(str, args[1:])]
    result = subprocess.run(command, capture_output=True, timeout=30)
    if result.returncode != 0:
        raise RuntimeError((command, result.stderr.decode(errors='replace')))
    return result

for name, source in cases.items():
    source_file = root / f'inputs/fixture-{name}.txt'
    qzt_file = root / f'inputs/fixture-{name}.qzt'
    source_file.write_bytes(source)
    run(before, 'pack', source_file, '-o', qzt_file, '--chunk-size', 16,
        '--max-chunk-size', 16)
    run(before, 'verify', qzt_file, '--deep')
    output_file = root / f'outputs/fixture-{name}-export.txt'
    run(after, 'export', qzt_file, '-o', output_file)
    assert output_file.read_bytes() == source
    for index in (['ngram', 'token'] if name == 'repeated_lf' else ['ngram']):
        variants = {}
        for label, binary in [('before', before), ('after', after)]:
            qzi_file = root / f'outputs/fixture-{name}-{index}-{label}.qzi'
            command = [binary, 'sidecar-rebuild', qzt_file, '-o', qzi_file,
                       '--index', index]
            if index == 'ngram':
                command += ['--ngram', 3]
            run(*command)
            qzi = qzi_file.read_bytes()
            variants[label] = (qzi, qzi_file)
        assert variants['before'][0] == variants['after'][0], name
        qzi_file = variants['after'][1]
        run(after, 'inspect-sidecar', qzt_file, '--sidecar', qzi_file)
        if name == 'repeated_lf':
            output = run(after, 'search', qzt_file, 'aaa', '--sidecar', qzi_file)
            assert b'source=verified_original_bytes' in output.stdout
        results.append({'case': name, 'index': index, 'source_bytes': len(source),
                        'source_sha256': hashlib.sha256(source).hexdigest(),
                        'qzi_bytes': len(variants['after'][0]),
                        'qzi_sha256': hashlib.sha256(variants['after'][0]).hexdigest(),
                        'byte_identical': True, 'deep_verify': True,
                        'export_identical': True, 'sidecar_open': True})

(root / 'raw/fixture-results.json').write_text(json.dumps(results, indent=2) + '\n')
print(json.dumps(results, indent=2))
