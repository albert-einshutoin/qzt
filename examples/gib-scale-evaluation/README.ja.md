# 1 GiB 実文書評価

PR350の小規模評価とは別の実験。製品Core、format、既定予算は変更しない。
条件・質問・採点規則は [PLAN.ja.md](PLAN.ja.md)、測定結果は `RESULT.ja.md` を参照。
これは英語百科資料に対する同一モデルの評価であり、人間の評価や一般的な需要の証明ではない。

## 再実行

macOS、Python 3.12以上、Rust/Cargo、ripgrep、`/usr/bin/time -l`、32 GiB程度のmemory、
30 GiB以上の空き容量を前提とする。監視はsamplingで、OSによる厳密なRSS制限ではない。
ネットワーク取得は `corpus.py` の固定URL allowlistに限る。新しい空の出力directoryを指定する。
取得前にライセンスと配布継続を確認する。欠落した固定dumpを別の日付へ暗黙に差し替えない。

```sh
git cat-file -e 1562940cf9a93e885d32faf0f838a5b288d96497^{commit}
git diff --exit-code 1562940cf9a93e885d32faf0f838a5b288d96497 -- src Cargo.toml Cargo.lock
cargo build --release --locked --bin qzt
mkdir /absolute/fresh-root
python3 examples/gib-scale-evaluation/corpus.py /absolute/fresh-root/corpus
python3 examples/gib-scale-evaluation/experiment.py calibrate /absolute/fresh-root --corpus /absolute/fresh-root/corpus --binary "$PWD/target/release/qzt"
python3 examples/gib-scale-evaluation/experiment.py layout /absolute/fresh-root --corpus /absolute/fresh-root/corpus --binary "$PWD/target/release/qzt"
python3 examples/gib-scale-evaluation/freeze.py main /absolute/fresh-root
python3 examples/gib-scale-evaluation/experiment.py prepare /absolute/fresh-root
python3 examples/gib-scale-evaluation/bench.py /absolute/fresh-root
python3 examples/gib-scale-evaluation/freeze.py assets /absolute/fresh-root
```

本実行は最初の収集がhost pressureで停止したため、原本・ログを保持し、正常回復後の明示的な一度のretryで
`corpus-complete` を正本にした。`corpus.py --help` にある検証済みarchive再利用・既消費通信量の引継ぎを使い、
既存のpartialを成功扱いしない。校正値を見て本条件を変更するなら、main freeze前に理由と条件を固定する。
freeze後の条件・質問・rubric変更は別試行となる。配置path・作成時刻・compilerは再実行間で異なり得る。
`configuration.source` はこの実験で固定したCoreの基準commitである。上記のGit objectとCore/Cargoの一致を
先に確認し、別のCoreで走らせてこの基準の結果を名乗らない。shallow cloneにobjectがなければ実commitを取得する。
保存snapshotだけでGit objectの存在を代用しない。binary SHAとcompiler情報は実行ごとに新しく記録する。

納品直前のGateで再現経路を補強した。prepareの開始/成功保存前とassets封印時に宣言binary SHAを要求する。
この固定事例の単体indexはexit 1とresource-limit診断だけを期待し、exit 0を含む別結果は条件変化として停止する。
任意の非0から分割へ継続しない。異なる入力やCoreへ結果を転用しない。
実測に使った8スクリプトの正確な本文は `evidence/measured-harness-source.json.gz` にhash付きで保存した。
measured main-freezeはその旧本文を束縛し、現行の補強済みコードを測定したとは主張しない。
現行コードで再実行するときは新しい空rootでfreezeを作る。旧rootのfreezeを緩めるfallbackはない。

回答者は新規context・同じmodel/effortとし、`run-plan.json` 順で各trialの `output/activation.json` に
`{"started_epoch": <開始時刻>}` を保存して開始する。各自の `access.py TRIAL info` から使い方を取得する。
原本や別trial、rubric、正解経路への直接accessは禁止する。これは指示による分離で、OS sandboxではない。
同一pairの質問順は同一、B/C開始順は交互。全native CLIは共通lockで逐次実行しqueue時間を別記録する。
seedは資料参照列と実行・質問・匿名化の順序だけに適用する。モデル生成のseedや同一回答の再生成は保証しない。
`finish` は時間切れ後も未完了回答を保存でき、lateは別状態として残す。

8回答を固定した後 `ratings.py prepare ROOT` を一度だけ実行する。新規の独立採点者4体へ同じ匿名bundleを
渡し、各judgeのactivationを記録する。`ratings.py JUDGE info|sources START|answers START|finish JSON` を使う。
引用byte範囲/SHAの機械照合は意味採点と分離する。多数決を真実の根拠にしない。

## 費用と範囲

平文Bは23,743ファイルと共通inventory、CはDocument Indexのないstreaming QZTと同じ外部inventoryを使う。
全体検索Cは全142 QZI/QZTをfan-outし、IDを知っていれば両条件とも直接read/withinが可能。
検索のtoken/whole-wordの意味、表示上限、coverage未知、CLI起動・結合費用を結果に含む。
cacheはwarm/unknown。機械計測とAgent並列wallを分離し、未知のOS I/O・料金は推計しない。
元原文を保持する運用ではCの保存量は追加費用であり、原文削除後の最小配布量とは別である。

巨大原本、binary、QZT/QZI、全文native stdoutはGitへ入れない。外部保管のrawと公開する小さいmanifest/hash・
集計の対応は結果とevidence manifestに記録する。rawがなければ回答traceの完全な第三者再監査はできない。
`evidence/inventory.jsonl.gz` は共通配置表、`construction-measurements.json.gz` はsampling列を除く構築記録。
標準libraryの `gzip.open` で読める。元のjson/stdout/stderrのSHAとbytesを保持し、巨大な本文traceを埋め込まない。

## 検証

```sh
QZT_GIB_BINARY="$PWD/target/release/qzt" python3 -m unittest discover -s examples/gib-scale-evaluation -p test_scale.py
python3 -m unittest discover -s scripts -p 'test_*.py'
```

native binaryを指定しない場合はその部分がskipとなる。1 GiB本評価は通常CIで再実行しない。

## 帰属とライセンス

文書と `rubric.json` 中の原文引用は英語Wikipedia contributorsによる
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) の資料。
取得元は [2026-10-01 dump](https://dumps.wikimedia.org/enwiki/20261001/)。
各文書のpage ID、revision ID、SHA、抽出規則をinventoryへ記録した。
revision本文は `https://en.wikipedia.org/w/index.php?oldid=REVISION_ID`、履歴は
`https://en.wikipedia.org/w/index.php?curid=PAGE_ID&action=history` で帰属を確認できる。
XMLからwikitext本文を抽出し、末尾LFだけを正規化した。Wikipedia/Wikimediaの推薦・提携は示さない。
評価コードはrepoのMIT OR Apache-2.0 licenseに従う。引用には元のCC BY-SA 4.0を適用する。

固定根拠のcontributors帰属（URLはdumpのrevision/page metadataから組み立てたもの）:

| doc ID | revision本文・記事名 | contributors履歴 |
| --- | --- | --- |
| p0000000902.txt | [Atom](https://en.wikipedia.org/w/index.php?oldid=1377869405) | [履歴](https://en.wikipedia.org/w/index.php?curid=902&action=history) |
| p0000001208.txt | [Alan Turing](https://en.wikipedia.org/w/index.php?oldid=1377975017) | [履歴](https://en.wikipedia.org/w/index.php?curid=1208&action=history) |
| p0000004650.txt | [Black hole](https://en.wikipedia.org/w/index.php?oldid=1378693646) | [履歴](https://en.wikipedia.org/w/index.php?curid=4650&action=history) |
| p0000007955.txt | [DNA](https://en.wikipedia.org/w/index.php?oldid=1374720238) | [履歴](https://en.wikipedia.org/w/index.php?curid=7955&action=history) |
| p0000009236.txt | [Evolution](https://en.wikipedia.org/w/index.php?oldid=1378711656) | [履歴](https://en.wikipedia.org/w/index.php?curid=9236&action=history) |
| p0000012024.txt | [General relativity](https://en.wikipedia.org/w/index.php?oldid=1377998321) | [履歴](https://en.wikipedia.org/w/index.php?curid=12024&action=history) |
| p0000025202.txt | [Quantum mechanics](https://en.wikipedia.org/w/index.php?oldid=1377463988) | [履歴](https://en.wikipedia.org/w/index.php?curid=25202&action=history) |
| p0000026962.txt | [Special relativity](https://en.wikipedia.org/w/index.php?oldid=1378669068) | [履歴](https://en.wikipedia.org/w/index.php?curid=26962&action=history) |
| p0000029952.txt | [Thermodynamics](https://en.wikipedia.org/w/index.php?oldid=1377203325) | [履歴](https://en.wikipedia.org/w/index.php?curid=29952&action=history) |
| p0000046740.txt | [Euler's identity](https://en.wikipedia.org/w/index.php?oldid=1377809648) | [履歴](https://en.wikipedia.org/w/index.php?curid=46740&action=history) |

回答で追加引用した資料も同じライセンスで帰属する:

| doc ID | revision本文・記事名 | contributors履歴 |
| --- | --- | --- |
| p0000009015.txt | [DNA replication](https://en.wikipedia.org/w/index.php?oldid=1376313272) | [履歴](https://en.wikipedia.org/w/index.php?curid=9015&action=history) |
| p0000009603.txt | [Ernest Rutherford](https://en.wikipedia.org/w/index.php?oldid=1378698072) | [履歴](https://en.wikipedia.org/w/index.php?curid=9603&action=history) |
| p0000009891.txt | [Entropy](https://en.wikipedia.org/w/index.php?oldid=1376020512) | [履歴](https://en.wikipedia.org/w/index.php?curid=9891&action=history) |
| p0000021147.txt | [Natural selection](https://en.wikipedia.org/w/index.php?oldid=1373147121) | [履歴](https://en.wikipedia.org/w/index.php?curid=21147&action=history) |
| p0000030403.txt | [Turing machine](https://en.wikipedia.org/w/index.php?oldid=1373616450) | [履歴](https://en.wikipedia.org/w/index.php?curid=30403&action=history) |
