# QZT公開pre.5による実CIログの調査・引き渡し

これはQZT自身の小さな利用例であり、外部利用者の受入実績ではない。公開済み
`v0.1.0-pre.5`をmacOS 27.0 / Apple M4 / arm64で使用した。製品sourceは
`3bc7d2561c58b59cd166fed75bfbd45f2f4c0ebe`であり、この例のscript・文書
commitとは別である。Release archiveと`.sha256` sidecarを照合した。展開binaryは
`qzt 0.1.0-pre.5`、target `aarch64-apple-darwin`、SHA-256
`989e6f21d80c5f14ee90b27aabe156760f0c6992bc6a5520aaa1400ba20b29be`。

2026-09-27 UTCに`gh api .../actions/jobs/{job}/logs`でjob logの応答byteを
保存した。BOM、timestamp、ANSI、LFを保持し、`.gitattributes`で改行変換を防いだ。
取得時刻・方法・run/attempt/job・commit・step結果の正本は
[`source.json`](source.json)。

| 原本 | 対象commit | byte数 | SHA-256 |
| --- | --- | ---: | --- |
| [pre.4失敗job](https://github.com/albert-einshutoin/qzt/actions/runs/36257908117/job/108448130555) | `4715196614c54c54b7809f99a422343ef2cf7b85` | 34,063 | `1d42a7aae2ba967bf6a39e40dfaaedfd8262ba9e42fa7fd2b3511f0a1004523b` |
| [pre.5成功job](https://github.com/albert-einshutoin/qzt/actions/runs/36317632199/job/108615277305) | `3bc7d2561c58b59cd166fed75bfbd45f2f4c0ebe` | 38,038 | `70bcd3e7abdf6340ff17f9259f058a68b27bd78c2563b8e8a61475489628de42` |

QZTへ保存する**前**に、原本byteの独立走査で3つの文字列と前後行を
[`queries.json`](queries.json)へ固定した。以下のoffsetは0始まり、範囲は
半開区間。2文書目の全体開始位置34,063は1文書目の原本長から計算した。

## 3つの問いへの回答

1. **pre.4はどこで失敗したか。** job metadataでは`Build artifacts`は成功、
   `Verify the dist build kept its selected toolchain`が失敗。pre.4の
   `dist ran successfully`は、echoされたコマンドが文書byte
   `25026:25047`、実際の出力が`29906:29927`。したがってビルド後の
   環境検査で失敗した。[元job](https://github.com/albert-einshutoin/qzt/actions/runs/36257908117/job/108448130555)と
   [`query-2.json`](raw/query-2.json)を参照。
2. **失敗の根拠は何か。** `ci/pre4-linux.log`の文書・全体byte
   `32023:32060`に`RuntimeError: build checkout is dirty`がある。
   前後範囲`31220:32135`には`Traceback (most recent call last):`、
   `scripts/record-build-environment.py`の`main`→`snapshot`、
   `raise RuntimeError("build checkout is dirty")`、exit code 1が含まれる。
   [元job](https://github.com/albert-einshutoin/qzt/actions/runs/36257908117/job/108448130555)、
   [`query-0.json`](raw/query-0.json)、[`queries.json`](queries.json)に
   出典と前後行を残した。
3. **pre.5の対応箇所はどうなったか。** metadataでは同じビルド工程と
   環境検査が成功。実際の`dist ran successfully`出力は
   `ci/pre5-linux.log` byte `29938:29959`（echoは`25027:25048`）。
   文書byte `31281:31309`（全体`65344:65372`）に
   `build environment unchanged:
   3bc7d2561c58b59cd166fed75bfbd45f2f4c0ebe
   x86_64-unknown-linux-gnu`と記録される。前後範囲は文書
   `31129:31586`（全体`65192:65649`）。
   [元job](https://github.com/albert-einshutoin/qzt/actions/runs/36317632199/job/108615277305)と
   [`query-1.json`](raw/query-1.json)を参照。対応する検査の成功は確認できるが、
   このログだけでpre.4のdirty checkoutの内部原因すべては確定しない。

## 保存・受取側の結果

2文書を1つの15,569 byteのQZTへ`pack-docs`で保存し、Deep verify、正準
attestation、文書一覧、検証付き`doc`復元、全体exportの72,101 byte連結一致を
確認した。1つのn=3 QZIで3 queryを検索し、hitの全体offset・長さ・元文書・
文書内offset・前後行のbyteが独立oracleと一致した。
`dist ran successfully`の4 hitにはechoと実出力の両方がある。
`--max-results 1`の別検索は1 hitで`capped=true`、
`stop_reason=max_search_results`。完全結果とは扱わない。
`index_complete_declared=true`、`index_coverage_verified=false`であり、
この2原本・3 queryとの一致はQZI一般の網羅性を証明しない。

別directoryにはQZT・attestation・source/query manifest・script・公開CLI
のみを渡し、原本とQZIは渡さなかった。受取側でDeep verify、attestation
byte一致、両docの原本hash一致、QZI再構築、同じqueryと証拠範囲取得を確認した。
結果は[`recipient-measurements.json`](raw/recipient-measurements.json)。
未署名のmanifestとattestationは保存byteの整合・調査の再実行性を示すが、
ログ内容の真実性、作成者、信頼できる時刻は認証しない。

## 容量・時間・手間

| 形態 | 内訳 | 合計byte |
| --- | --- | ---: |
| 平文 | 2原本72,101 + 共通source manifest 6,748 | 78,849 |
| tar.gz | archive 15,186 + 同manifest 6,748 | 21,934 |
| QZT | 本体15,569 + attestation 695 + 同manifest 6,748 | 23,012 |
| QZT＋QZI | 上記23,012 + n-gram QZI 124,088 | 147,100 |

調査用query manifestは全形態に別途5,696 byte。公開binaryとscriptは
tooling費用として別。原本もGitで保持する場合は、QZT引き渡し一式に
72,101 byteを追加する（Git圧縮前）。tar.gzの展開byteも原本と一致した。

並列度1の観測1回で、QZT作成14.272 ms、Deep verify 2.833 ms、QZI構築
18.577 ms、通常検索各3.377–3.625 ms、前後範囲取得各2.402–3.229 ms、
文書復元2.582/2.628 ms、tar.gz作成3.477 ms。各command・設定・終了状態は
[`measurements.json`](raw/measurements.json)。CLI起動と出力処理を含む
macOS上のwall timeであり、cold cacheやp95/p99、大規模データの性能は示さない。
平文・tar.gzで同じ証拠byteまで取り出す時間は測っておらず、形式間の
検索速度比較はしていない。

QZTの利点は、名前付き文書を検証付きで復元し、原本を渡さずに検索sidecarを
再構築できたこと。一方、この小さな入力ではtar.gzの方がQZT＋attestation
より小さく、QZIは原本より大きい。job/step metadataは別取得、文書開始位置と
検索結果の全体offset→文書内offsetの変換は手計算をscript化する必要があった。
echo行と実出力の判定も前後行・metadataを読んで行った。
`index_complete_declared`と`index_coverage_verified`、打切り状態を併読する
手順は分かりにくい。製品機能の失敗は今回観測せず、これらは次のUX検討候補
として#31へ渡す。
