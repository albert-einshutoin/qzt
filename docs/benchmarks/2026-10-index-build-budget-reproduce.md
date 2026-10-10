# 構築予算の再現（repository同梱版）

これは2026-10-11のGitHub納品用手順。過去の計測host固有driverを実行する必要はない。
元のローカル手順/出力/manifestは原archiveに保持し、[納品証拠index](2026-10-index-build-budget-delivery-evidence.json)
へ元の所在・hashを記録する。ここでの再実行は新しい観測で、過去値へ混ぜない。

## 小fixtureと入口契約

repository rootで実行する。Rust1.96.0/Cargo.lockがローカル計測の条件。
対応環境でtargeted checksを再実行できる。MSRV1.87とplatformの最終検証はhosted CIと別に扱う。

```sh
cargo test --locked --all-features --test phase49_index_build_budgets \
  --test phase19_search_budgets --test phase11_search --test phase12_ngram_planner \
  --test phase13_sidecar --test phase44_safe_file_output
cargo test --locked --lib build_counter_overflow
```

`phase49_index_build_budgets`の小inputと`tests/fixtures/index-build-budget/*.hex`は同梱済み。
空入力、inclusive境界/超過、duplicate pair、Unicode/emoji/CRLF/chunk跨ぎ、encoded skip/
section境界、CLI不正2/制限拒否1、入力/有効既存QZI保持、新規出力不在を確認する。
hex vectorは変更前source buildのQZI bytesを保存した回帰用artifactであり、第三者検証ではない。
生成元identityは同梱`golden-provenance.json`の派生digest記録。
元のJSON原本はlocal-onlyで保持し、所在/hashを納品indexへ記録している。

## 決定的なC2/C4の生成とhash照合

追加依存のない小adapterが変更していない`src/corpus.rs`を直接使い、seed295と同じmarkerで
過去と同じ原文を生成する。QZTの製品source buildとは別の生成専用compileである。
100 MiBまでの生成を許すが、以下の通常再現は1 MiBだけを使用する。

```sh
qzt_replay_dir=$(mktemp -d)
rustc --edition 2024 -O scripts/index-build-budget-corpus.rs \
  -o "$qzt_replay_dir/corpus-generator"
printf '%s\n' C2 1048576 295 "$qzt_replay_dir/C2-1MiB.txt" | "$qzt_replay_dir/corpus-generator"
printf '%s\n' C4 1048576 295 "$qzt_replay_dir/C4-1MiB.txt" | "$qzt_replay_dir/corpus-generator"
python3 - "$qzt_replay_dir" <<'PY'
import hashlib, json, pathlib, sys
directory = pathlib.Path(sys.argv[1])
bindings = json.loads(pathlib.Path('docs/benchmarks/raw/2026-10-index-build-budget/fixture-bindings.json').read_text())
for kind in ('C2', 'C4'):
    entry = next(item for item in bindings if item['fixture'] == kind + '-1MiB')
    data = (directory / (kind + '-1MiB.txt')).read_bytes()
    assert len(data) == entry['source_bytes']
    assert hashlib.sha256(data).hexdigest() == entry['source_sha256']
    assert data.count(b'qztpre6needleunique') == 1
    assert data.count(b'qztpre6absentunique') == 0
print('C2/C4 source size/hash/query oracle: OK')
PY
```

adapterはstdinの4行（kind/bytes/seed/output）を4096 byte以内で受理し、argvのprogram nameを
identityに使用しない。10 MiBならbytesを10485760、output名を`C2-10MiB.txt`等に変え、
同じmanifestへ照合する。100 MiBは104857600。元のQZT/QZIやbinaryをGitへ同梱していない。
同じ生成rawでもbuild toolchain/環境によりbinary hashの一致を保証しない。

## 変更後CLIの成功と拒否

現在のsourceからbuildする。公開pre.6では新flagは使用できない。同じversion文字列を
公開binary同一の証拠にしない。次は1 MiBの限定的な手順で、各commandに30秒のtimeoutを使う。
output directoryはfresh、inputとoutputは別にする。

```sh
cargo build --release --locked --bin qzt
python3 - "$qzt_replay_dir" <<'PY'
import importlib.util, json, pathlib, sys
spec = importlib.util.spec_from_file_location('cost', 'scripts/cli-cost-benchmark.py')
cost = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cost)
directory = pathlib.Path(sys.argv[1])
binary = str(pathlib.Path('target/release/qzt').resolve())
attempts = []
def run(*args, expected=0):
    result = cost.run([binary, *map(str, args)], 30)
    attempts.append(result)
    (directory / 'commands.json').write_text(json.dumps(attempts, indent=2) + '\n')
    assert result['exit_code'] == expected, result
    return result
for kind in ('C2', 'C4'):
    source = directory / (kind + '-1MiB.txt')
    qzt = directory / (kind + '.qzt')
    run('pack', source, '-o', qzt, '--chunk-size', 262144, '--max-chunk-size', 262144)
    qzt_before = cost.sha256(qzt)
    for index in ('token', 'ngram'):
        qzi = directory / (kind + '-' + index + '.qzi')
        run('sidecar-rebuild', qzt, '-o', qzi, '--index', index)
        qzi_before = cost.sha256(qzi)
        rare = json.loads(run('search', qzt, 'qztpre6needleunique', '--sidecar', qzi, '--format', 'json')['stdout'])
        assert len(rare['hits']) == 1 and rare['index_coverage_verified'] is False
        hit = rare['hits'][0]
        data = source.read_bytes()
        assert data[hit['logical_offset']:hit['logical_offset'] + hit['byte_length']] == b'qztpre6needleunique'
        denied = run('sidecar-rebuild', qzt, '-o', qzi, '--index', index, '--max-build-postings', 0, expected=1)
        assert 'a resource limit was exceeded' in denied['stderr'] and not denied['stdout']
        assert cost.sha256(qzt) == qzt_before and cost.sha256(qzi) == qzi_before
        fresh = directory / (kind + '-' + index + '-refused.qzi')
        run('sidecar-rebuild', qzt, '-o', fresh, '--index', index, '--max-build-granules', 0, expected=1)
        assert not fresh.exists()
print('admitted search bytes / product refusal / existing and new output protection: OK')
PY
```

この小再現はOS peak RSS計測や厳密なmemory上限を提供しない。
過去の大きい計測では各180秒/全体3600秒、disk8 GiB/空き10 GiB、外部RSS2 GiB、
critical圧迫、warning+RSS1 GiBのbest effort停止を使用した。
その監視を用意せずに過去の大入力比較を一括実行しない。
1 MiB手順の結果を10/100 MiBや一般coverageの保証へ広げない。

## 既存profile matrixの明示予算と拒否記録

`make bench-profile-matrix`は既定の構築予算を使う。`ResourceLimitExceeded`のcaseは
`status=resource_limit_exceeded measured_success=false`を記録し、次caseへ進む。
その他のerrorは実行を失敗させる。拒否caseには成功時の計測値を作らない。
必要な予算を利用者が決めた場合だけ、次の環境変数へ非負u64を指定できる。

| 環境変数 | 対応する構築予算 |
| --- | --- |
| `QZT_RELEASE_BENCH_MAX_BUILD_GRANULES` | granules |
| `QZT_RELEASE_BENCH_MAX_BUILD_KEYS` | distinct keys |
| `QZT_RELEASE_BENCH_MAX_BUILD_POSTINGS` | posting IDs |
| `QZT_RELEASE_BENCH_MAX_BUILD_KEY_BYTES` | distinct key bytes |
| `QZT_RELEASE_BENCH_MAX_BUILD_ENCODED_BYTES` | encoded phase bytes |

未指定は製品default、値0は拒否用にも使える。不正値は失敗する。
例: `QZT_RELEASE_BENCH_MAX_BUILD_POSTINGS=16000000 make bench-profile-matrix`。
この例は他の予算を増やさず、全caseの受理を保証しない。matrixは1/10/100 MBの既存
profileであり、前述の外部監視なしに大きい入力を一括実行しない。
今回の修正確認は小回帰で拒否と後続成功を区別し、matrix全体は再実行していない。

## 10/100 MiBの過去証拠を読む

同梱recordと`attempt-output.jsonl`で全122試行を追跡できる。
10 MiB/ngram既定はC2/C4とも製品拒否、default残り2反復と後続searchは未実施。
posting16Mを明示した10 MiB小controlは成功し、before QZI bytes/検索結果と一致した。
100 MiBはafter既定で通常の製品拒否と保護を確認し、before完走は再実行しなかった。
外部中止した前Goalの成功値はunknownを維持する。
これらのoriginal binary/QZI/corpusはlocal-onlyで、GitHub上の第三者が原本を取得できるとは
説明しない。Source、input hash、command、text output、失敗/未実施と小回帰は同梱している。

完全検証の再実行は`make check`、`cargo package --allow-dirty --locked`。
歴史的local MSRVはSDK27/TAPIで失敗し、installed SDK15.2をそのcommandだけに指定して成功した。
これは当時のhost条件であり、Linuxのhosted MSRV結果と混同しない。
公開Rust/API移行と保証しないRSS範囲は[契約](../QZT_v0.1_Memory_Guarantees.md#index-construction-admission-unreleased)を参照。
G3全体、第三者検証、production-ready、release公開はこの納品の対象外である。
