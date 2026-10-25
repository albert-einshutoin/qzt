# 独立Agentによる意思決定履歴の比較評価

公開PR #286を未mergeで閉じた判断一件について、回答9体・採点3体をfresh contextで実行した。
[比較結果と次の判断](RESULT.ja.md)、[事前計画](PLAN.ja.md)、[質問と公平な要約](questions.json)、
[固定採点規則・正解根拠](rubric.json)、[出典snapshot](snapshot/manifest.json)を参照する。
人間の利用者評価、市場需要、独立第三者検証、production-readyの証明ではない。
既存External Validationの「作者以外の本人データ/本人操作」の定義と未達状態は変更しない。

sourceはmain `4681072bd78f8a098f4583a86c765deaab5894c6` と同じCore/Cargo bytes。
実験用debug binaryのSHA-256は
`c2f61139cb65aa9c38b9342ff33b5e9b6d894382dd6f84b7ae0d99db22ba83e4`。
公開pre.6のsource `0c8110e6b6e08513b4def3f636b5b79d068e275f` と区別する。
version文字列の一致は公開binaryとの同一性を示さない。binary/QZT/QZIは生成物としてGitに入れない。

保存結果の入口は[canonical index](canonical-index.json)。比較の入力freeze、全試行の原回答、
gateway/CLIの生記録、匿名対応、共通採点入力、全243採点、機械照合、15行の不一致と親の
原文照合、実agent ID/実model/effort/入力・tool出力hash/token記録を辿れる。
同一filesystem上のAgent分離は指示と記録監査によるもの。CLI子processだけを既存sandbox-execで
network/write禁止にした。Agent全体のOS sandbox、モデルの隠れた知識の排除、本人性・署名認証は保証しない。

保存物の再照合はrepository rootで行う。index自身の外部期待hashが必要な運用では、Git revisionと
index SHAを別経路で固定する。自己計算hashを出典認証と呼ばない。

```sh
python3 - <<'PY'
import hashlib, json, pathlib, sys
root = pathlib.Path('examples/agent-decision-evaluation')
index = json.loads((root / 'canonical-index.json').read_text())
for item in index['files']:
    data = pathlib.Path(item['path']).read_bytes()
    assert len(data) == item['bytes']
    assert hashlib.sha256(data).hexdigest() == item['sha256']
sys.path.insert(0, str(root))
from harness import snapshot, check_citation
_, docs = snapshot(root / 'snapshot')
docs['summary'] = json.loads((root / 'questions.json').read_text())['summary'].encode()
count = 0
for path in sorted((root / 'raw/trials').glob('*/output/answer.json')):
    for answer in json.loads(path.read_text())['answers']:
        for cite in answer['citations']:
            assert check_citation(cite, docs)['valid'], (path, answer['id'])
            count += 1
assert count == 105
print('index and 105 citation byte checks passed')
PY
```

新しい環境でCLIを含む動作確認をするには、Python 3.9以降、Cargo/Rust、rgが必要。
本評価の既存runtimeやrawを上書きしない。下記runtimeは新規で、回答を実行していない状態を使う。
CLIのsandbox-execはmacOSで利用可能な場合だけ設定され、有効にした後の失敗を黙って迂回しない。
Linux/Windowsで再実行した結果を、本評価のmacOS証拠と同一扱いしない。

初回は、製品base、判断時main、閉じた未merge PR #286のheadをローカルGit objectとして取得する。
通常のmain checkoutだけでは未mergeのPR objectは揃わず、shallow checkoutでは古いmainも不足し得る。
保存済みsnapshotとは別に、prepare.pyが固定commitのbytesを`git show`で照合するための前提条件である。
必要な公開refだけを取得し、固定SHAのcommitが読めることを確認する。PR refの現在のheadへ
根拠を読み替えない。将来refが移動して固定objectを取得できなければ停止する。
この初回取得にはネットワークが必要で、objectが揃った後の準備・検証は保存snapshotで実行できる。

```sh
git fetch origin \
  4681072bd78f8a098f4583a86c765deaab5894c6 \
  8aaec582259815189deb1844eb66c2b6bc2e1efd \
  refs/pull/286/head
git cat-file -e '4681072bd78f8a098f4583a86c765deaab5894c6^{commit}'
git cat-file -e '8aaec582259815189deb1844eb66c2b6bc2e1efd^{commit}'
git cat-file -e '16033d45bfc96b7b43011e597c302a650d026efc^{commit}'
```

```sh
cargo build --locked --bin qzt
trial_parent=$(mktemp -d)
python3 examples/agent-decision-evaluation/prepare.py "$trial_parent/runtime"
QZT_AGENT_RUNTIME="$trial_parent/runtime" \
  python3 -m unittest discover -s scripts -p test_agent_decision_evaluation.py
```

`prepare.py`は保存済みsnapshotをraw/pointer/hash/上限へ再照合し、正解引用を確認してからpackし、
export完全一致、Deep verify、ngram QZI構築/inspect、Document Indexを検証する。Core差分を拒否する。
snapshotを取り直す場合だけ `python3 examples/agent-decision-evaluation/acquire.py <新規snapshot>` を使う。
取得前に保存する8 URL以外を探索せず、redirect/pagination/上限超過は停止する。
可変本文の新取得を保存済みsnapshotや過去時点の原文と混同しない。

比較を再実行する場合は、[run plan](raw/run-plan.json)と同じモデル/effort、seed、停止条件を使う。
`prepare.py <runtime> --freeze`を回答前に一度だけ実行し、各試行の`output/activation.json`へ
起動epochを新規保存してから、同ファイルのlaunch順でfresh context回答者を3体ずつ作る。
[実際の最小指示とtool trace](raw/execution-registry.json)に全12体の入力・設定がある。
各回答者には専用gateway `access.py <runtime>/trials/<ID> info` とその操作だけを渡し、
原文条件・QZT条件をまたがせない。正解/他者回答/全チャットを渡さない。
質問・採点規則・helper・binaryのfreezeを回答中に変更せず、資料外アクセス試行は無効として残す。

全9回答の提出後にだけ、以下を実行する。無提出/不正提出はmissing/invalidとして残り、
回答者が明示したunknownとは区別される。Agent実行には同じcollaboration機能とモデルアクセスが必要で、
通常のCLIチェックだけで独立Agentの実行を代替したことにはしない。

```sh
python3 examples/agent-decision-evaluation/audit.py <runtime> <新規audit.json>
python3 examples/agent-decision-evaluation/judge.py prepare <runtime>
```

J1〜J3には各専用`input.json`だけを渡す。同じinput SHAを照合し、条件/model名/他者採点を隠す。
`judge.py <runtime>/judges/J1 info`で読み、同じgatewayの`finish '<scores JSON>'`で全81行を保存する。
tool表示がtruncateした場合は専用inputだけを分割して読む。全回答・引用・固定根拠の表示確認は
[補助監査v2](raw/judge-visibility-check-v2.json)にあり、v1のtransport decode不足による偽の欠落も保持した。
この表示確認はモデルが注意を払ったことや意味的正解の証明ではない。

集計は各採点者の元点を上書きしない。各試行/次元の合計を作り、3試行の中央値と最小/最大を記す。
[独立採点集計](raw/score-summary.json)、[実測集計](raw/descriptive-statistics.json)、
[不一致](raw/disagreements.json)、[原文照合](raw/adjudication.json)が今回の算出結果。
多数決を真実の根拠にせず、原文/固定規則で決められない点は評価不能とする。

完全gate・package・securityの保存ログと[専門レビュー](raw/review/specialist.ja.md)はローカル証拠。
Companion Gateも静的受入であり、最終PR/mainのCI/securityとmergeはGitHub上の各HEADに結び付けて確認する。
本評価ではrelease/tag/crates.io/deploy、CI構成変更、次の実装は行わない。
