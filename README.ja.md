# QZT — テキストのための Cold Evidence Container

[English](README.md) · [![CI](https://github.com/albert-einshutoin/qzt/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/albert-einshutoin/qzt/actions/workflows/ci.yml?query=branch%3Amain)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

> 大きなテキストを一度保存し、あとから証明する。QZTはchunked zstd、
> BLAKE3検証付きrandom access、行指定、検証済み検索を1つにまとめます。

## QZTを選ぶ理由

- **検証可能な証拠** — readとattestationを原文byteへ結び付けます。
- **random access** — 全体解凍せずbyte範囲や行範囲を復元します。
- **検証済み検索** — token / n-gramのhitを原文byteに再照合します。

## Install / インストール

現在公開されているCLIは
[`v0.1.0-pre.2` Release](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.2)
です。OSとarchitectureに合うarchiveと`.sha256`を取得し、checksum検証後に
展開します。Apple siliconの例です（Intel Macは`x86_64-apple-darwin`、
Linux x86_64は`x86_64-unknown-linux-gnu`を指定）。

```sh
set -eu
release=v0.1.0-pre.2
target=aarch64-apple-darwin
archive="qzt-${target}.tar.xz"
base="https://github.com/albert-einshutoin/qzt/releases/download/${release}"
curl --proto '=https' --tlsv1.2 -fLO "${base}/${archive}"
curl --proto '=https' --tlsv1.2 -fLO "${base}/${archive}.sha256"
expected="$(awk 'NF { print $1; exit }' "${archive}.sha256")"
if command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "${archive}" | awk '{ print $1 }')"
else
  actual="$(sha256sum "${archive}" | awk '{ print $1 }')"
fi
test "${expected}" = "${actual}"
tar -xJf "${archive}"
QZT_BIN="$(pwd)/qzt-${target}/qzt"
"$QZT_BIN" --version
```

以下のツアーでは、この絶対パス`QZT_BIN`を使います。公開CLIのversion出力は
`qzt 0.1.0-pre.2`です。checksumの真正性は、信頼するReleaseとrepositoryの
経路で確認してください。

<details>
<summary>Windows・installer・source build</summary>

<br>

Windowsでは同じReleaseの`.zip`と`.zip.sha256`を取得し、展開前に検証できます。

```powershell
$archive = "qzt-x86_64-pc-windows-msvc.zip"
$expected = (Get-Content "$archive.sha256" | Select-String -Pattern '\S').Line.Split()[0]
$actual = (Get-FileHash -Algorithm SHA256 $archive).Hash
if ($expected -ne $actual) { throw "SHA-256 checksum mismatch" }
```

または同じReleaseの`qzt-installer.sh`／`qzt-installer.ps1`を利用できます。
配布binaryを使わず、**公開版と同じタグ**からbuildする場合:

```sh
cargo install --git https://github.com/albert-einshutoin/qzt --tag v0.1.0-pre.2 --locked qzt
```

`cargo install qzt --version 0.1.0 --locked`は、そのversionがcrates.ioへ
公開された後の選択肢です。現時点の導入手順ではありません。

</details>

## 60秒ツアー

上でchecksumを検証したpre.2 binaryを使い、`QZT_BIN`にはその絶対パスを
設定してください。POSIX shell、`mktemp`、`cmp`を使用します。新しい使い捨て
directoryで既存fileに依存せず実行します。

```sh
set -eu
test -x "$QZT_BIN"
tour_dir="$(mktemp -d)"
cd "$tour_dir"
printf 'alpha\nbeta\nerror gamma\n' > app.log
"$QZT_BIN" pack app.log -o app.qzt
"$QZT_BIN" info app.qzt --format json
"$QZT_BIN" range app.qzt --lines 2:2
"$QZT_BIN" sidecar-rebuild app.qzt -o app.qzt.qzi
"$QZT_BIN" search app.qzt "error" --sidecar app.qzt.qzi --format json
"$QZT_BIN" verify app.qzt --deep --format json
"$QZT_BIN" attest app.qzt > app.attest.json
"$QZT_BIN" export app.qzt -o restored.log
cmp app.log restored.log
```

`info`は原文23 bytes・3行、rangeは改行を含む`beta`を返します。searchは
byte offset 11に`verified_original_bytes`由来の`error` hitを1件返し、verifyは
`ok=true`・`level=deep`を返します。pre.2のattestationは決定的な
**versionなし**JSONで、`attestation_schema`や後のDeep coverage fieldはありません。
`cmp`成功はexportと原文の全byte一致を意味します。JSON・決定性・byteを自動判定する
[配布binary smoke script](docs/guides/examples/smoke-release-tour.sh)は絶対パスのbinaryと`jq`を
必要とします。[実測記録](docs/guides/tutorial-validation.md)も参照してください。

## 開発版CLI

現行の[CLIリファレンス](docs/CLI.ja.md)と以下の運用guideは、commit
`ad709214f1e8ae18eff6e9f0b633345e40d1617b`の開発実装を対象とします。
pre.2にないcommand、検索予算、安全性修正、`qzt-attestation-v1`の検証fieldを
含みます。Rust 1.87以降でそのrevisionを導入する場合:

```sh
cargo install --git https://github.com/albert-einshutoin/qzt \
  --rev ad709214f1e8ae18eff6e9f0b633345e40d1617b --locked qzt
```

QZT container形式の`v0.1`はCLIの配布versionとは別です。pre.2 JSONへ
開発版のattestation判定を適用しないでください。以下の機能・制限説明は、
別途明示した場合を除き、この開発版revisionを対象とします。

## ユースケース

- **[server log保全](docs/guides/log-preservation.ja.md)** — logを日次containerへ
  streamし、定期的にdeep verifyして、決定的attestationを別系統へanchorします。
- **[pipeline artifact固定](docs/guides/artifact-fixation.ja.md)** — `pack-docs`で
  各input artifactを、1つのcontainer内の名前付き・checksum検証可能なdocumentとして固定します。
- **[incident検索運用](docs/guides/search-operations.ja.md)** — 明示したbudget内で
  再構築可能なQZI sidecarを検索し、調査に必要な検証済みbyte/line rangeだけを提示します。

## ステータスと制限

QZT v0.1 Coreはrelease candidateです。QZI検索とproduct全体は実験的な
`v0.1 technical preview`であり、production-readyではありません。

```text
- QZT v0.1 Core: release candidate
- Search Extension / QZI sidecar: technical preview
- Product status: 実験的な参照実装
```

## QZTがしないこと

QZTは境界を意図的に狭くした`v0.1 technical preview`です。

- **zstdの代替ではありません**。独立したzstd frameにseek、整合性metadata、
  evidence向けindexを追加するcontainerです。
- **解凍せずにテキストを表示するものではありません**。range readではcontainer
  全体ではなく、指定範囲と交差するchunkだけを解凍します。
- **Memory Pagerではありません**。mutableなvirtual memoryやOS pagingの抽象化は
  提供しません。
- **vector databaseやFM-indexではありません**。QZIはraw token / n-gramで候補を
  絞り、返すhitを必ずoriginal bytesへ再照合します。

QZI（`.qzi`）はCore container formatの一部ではなく、派生・再構築可能・非信頼の
検索sidecarです。導入前にfail-closed境界とon-disk layoutを
[QZI v0.1 Sidecar Spec](docs/QZI_v0.1_Sidecar_Spec.ja.md)で確認してください。
返すhitは原文byteへ再照合し、token境界と同じ行でのtoken ANDも確認します。
`index_complete_declared`はindexの宣言、`index_coverage_verified`は網羅性の
検証結果で、現在はfalseです。上限に達していない0件も原文での不存在を証明しません。

QZT v0.1 は、仕様カバレッジと正しさを重視した参照実装です。
production use の前に残っている既知の制限は以下です。

- **Index 構築メモリは語彙量に比例する**: `qzt search --sidecar` を含むすべての
  CLI コマンドが bounded-memory な `QztFileReader` 上で動作し、sidecar 検索は
  query された term の posting list と候補 granule レコードだけを fetch します
  （42 MB / 40 万行コーパスで rare query の最大 RSS は 518 MB → 9.8 MB）。
  一方、index の構築（`qzt sidecar-rebuild`、または `--sidecar` なしの
  `qzt search`）は posting map 全体をメモリに保持する（おおよそ sidecar サイズの
  展開分）ため、sidecar の構築はコーパスに見合ったマシンで行ってください。
  queryのkey数、posting処理、検証するlogical byte、実際のchunk展開、返却spanには
  上限がありますが、index構築全体やprocess RSSの上限ではありません。
  既定では16 MiBを超えるsource行を拒否します。
  [検索予算表](docs/QZT_v0.1_Memory_Guarantees.md#search-and-index-build-budgets)を参照してください。
- **一時 search index**: `--sidecar` なしの `qzt search` は、実行ごとに
  search index を再構築します（チャンク単位の decode ですが index 全体はメモリに
  残ります）。繰り返し検索する場合は、先に `qzt sidecar-rebuild` を一度実行し、
  その後 `qzt search --sidecar <file.qzi>` を使ってください。
- **Token search は phrase search ではなく co-occurrence**: multi-token query
  `"foo bar"` は、両方の token を任意の順序で含む行に match します。token が隣接している
  必要はありません。tokenizationはASCII英数字と`_`、`-`を受け付け、index前にASCIIを
  lowercase化します。substringやCJK形式の検索にはn-gram経路を使ってください。
  grep-compatibleではありません。
- **Normalized search は未実装**: `SearchIndexSource::NormalizedUtf8`
  (Unicode normalization、case folding、width folding) はまだ実装されていません。
- **Sidecar のサイズ**: 現行 writer は compact な QZI v2 layout を出力します。
  既存の QZI v1 sidecar も読み込めますが、v2 の容量削減を得るには再構築が必要です。
  release gate では再現可能な 10 MB high-cardinality log corpus に対し token / n-gram
  sidecar を原文の 1.7 倍以下に保ちます。語彙や行形状が異なるデータでは結果も変わります。
- **benchmarkはlocal evidenceでありSLAではない**: raw-zstdとのrange比較、
  ripgrep / SQLite FTS5との正確性照合は[2026年7月 v0.1 report](docs/benchmarks/2026-07-v0.1.md)
  を参照してください。Tantivy、Lucene、seekable-zstd、production log、
  cross-tool search latencyは未計測です。
- **開発版CLIの費用**: [2026年9月の100 MiB計測](docs/benchmarks/2026-09-cli-cost.md)では、
  CLIの新規process検索、実ファイルのQZT/QZI open、open済みAPI検索、独立processでの
  QZI構築時間とpeak RSS、並行検索、合計容量を区別しています。結果は記載された
  開発commitと合成corpusのもので、公開済み`v0.1.0-pre.2`の性能やSLAではありません。

### 性能数値の再現

従来のbenchmark数値はローカルevidenceであり、SLAやproduction保証では
ありません。以前のrelease benchmarkとprofilingは次のコマンドで再現できます。

```sh
cargo test --test release_hardening -- --nocapture
make bench-profile
```

軽量な反復用:

```sh
QZT_RELEASE_BENCH_QUERY_REPETITIONS=5 QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=2 make bench-profile
```

日常的なevidence確認では、同じ設定を使うconvenience targetも利用できます。

```sh
make bench-profile-quick
```

コーパス詳細、指標の定義、追加の profiling 対象は
[release-hardening guide](docs/QZT_v0.1_Release_Hardening.ja.md) を参照してください。

[9月のCLI費用report](docs/benchmarks/2026-09-cli-cost.md)に、開発版の実測コマンドと
raw logを記載しています。再現には[QZT repository](https://github.com/albert-einshutoin/qzt)
のcheckoutで`qzt`と`cli_cost_probe`をrelease buildし、reportの手順に従って
[`scripts/cli-cost-benchmark.py`](https://github.com/albert-einshutoin/qzt/blob/main/scripts/cli-cost-benchmark.py)
を新しいwork/log directoryで実行してください。このscriptはrepository用であり、
crate packageには含まれません。

### 任意の competitive benchmarks

Phase 18 には optional な competitive benchmark harness があります。計測値は
再現可能なローカル evidence であり、SLA や production の性能保証ではありません。

外部ツールを必要としない portable な smoke test:

```sh
cargo test --test phase18_competitive_benchmark -- --nocapture
```

ripgrep と SQLite FTS5 との比較は `bench-compete` で有効になります。`rg` または
FTS5 対応の `sqlite3` が利用できない場合、その comparator は skip されます。
利用可能なツールは参照 byte-scan の hit count と一致する必要があります。

```sh
cargo test --release --all-features --test phase18_competitive_benchmark -- --nocapture
```

詳細と QZT を使うべき場面は
[competitive benchmark methodology](docs/QZT_v0.1_Competitive_Benchmarks.md)
を参照してください。

実際に解凍したbyte数、圧縮payload読込量、独立processのpeak RSSを含む
production-scale range access結果は
[1 GiB partial-decompression evidence](docs/benchmarks/2026-07-partial-decompression.md)
を参照してください。

## ローカル品質ゲート

```sh
make check
```

このコマンドは以下を実行します。

```text
- cargo fmt --all -- --check
- cargo clippy --all-targets --all-features -- -D warnings
- cargo check --lib --bins
- cargo test --all-targets --all-features
```

## Round-trip smoke test

1 つのテキストファイルで pack → inspect → export → diff まで試す最短パスです。
QZT は `v0.1 technical preview` であり、production-ready ではない実験的な参照実装として扱ってください。

local checkoutから使う場合は、そのcheckoutのCLIをrelease modeでbuildします。
挙動はcheckoutしたcommitに従い、公開assetと同じとは限りません。

```sh
cargo build --release
./target/release/qzt --help
```

`PATH`へinstallしない場合、binaryは`./target/release/qzt`にあります。

プレーンテキスト（例: `input.txt`）を用意し、次を実行します。

```sh
./target/release/qzt pack input.txt -o output.qzt
./target/release/qzt info output.qzt
./target/release/qzt export output.qzt -o restored.txt
diff input.txt restored.txt
```

`diff` で出力がなければ、復元されたバイト列が元ファイルと一致しています。

## CLIリファレンス

この早見表と[現行CLIリファレンス](docs/CLI.ja.md)は
[commit固定の開発版](#開発版cli)を対象とします。公開pre.2 binaryでは
[公開タグのCLIリファレンス](https://github.com/albert-einshutoin/qzt/blob/v0.1.0-pre.2/docs/CLI.ja.md)
と上の公開版ツアーを参照してください。QZT形式`v0.1`はCLI versionではありません。

```sh
qzt pack input.txt -o output.qzt
journalctl --since today | qzt pack - -o today.qzt
qzt pack-docs server-a.log server-b.log report.txt -o bundle.qzt
qzt pack-docs server-a.log server-b.log -o logs.qzt --doc-id-prefix logs/ --profile memory
qzt info output.qzt
qzt info output.qzt --format json
qzt attest output.qzt > output.attest.json
qzt export output.qzt -o restored.txt
qzt range output.qzt --bytes 0:1024
qzt range output.qzt --lines 1:10
qzt line output.qzt 1
qzt docs output.qzt
qzt docs output.qzt --format json
qzt doc output.qzt report-2026-06
qzt doc output.qzt report-2026-06 -o out.txt
qzt doc output.qzt report-2026-06 --no-verify
qzt verify output.qzt --deep
qzt sidecar-rebuild output.qzt -o output.qzt.qzi
qzt search output.qzt "error" --sidecar output.qzt.qzi
qzt search output.qzt "error" --sidecar output.qzt.qzi --format json
```

### 複数ドキュメントの証拠コンテナ

`qzt pack-docs` は入力ファイルを指定順に、区切りを追加せず連結し、各ファイルを
個別に検証できるドキュメントとして記録します。doc_id は入力のbasenameです。
`--doc-id-prefix logs/` を指定すると `logs/server-a.log` のようにできます。
同じdoc_idは、`qzt doc`の参照先を曖昧にしないためusage errorとして拒否します。

`pack-docs` はpack前に全入力を読み込むため、合計入力サイズに比例したメモリを使います。
`memory` profileの既定はtarget 256 KiB、maximum 2 MiBで、小さなdocument/range取得の
展開量を抑えます。その代わり小さいchunkは圧縮率を下げる場合があります。
必要に応じて`--chunk-size`と`--max-chunk-size`で調整してください。

ライブラリでは`WriterBuilder::document_spans`へ`DocumentSpan`を渡すことで、
line、chunk、checksumを手計算せず同じDocument Indexを生成できます。

公開Writerは、default Readerがopen・Deep verifyできない設定やoptional indexを
エラーとして拒否します。`QztFileWriter`のsinkは実長0で、seekどおりに読み書きできる
`Read + Write + Seek`である必要があります。先頭やEOFに位置していても既存バイトが
あれば書込み前に拒否し、実長0で位置だけ先へ進んだsinkは先頭へ戻します。
`finish()`成功時はflush済みで位置はEOFですが、flushはfilesystem syncではありません。
streaming開始後の失敗では部分出力を呼出し側が廃棄してください。CLIの`pack`と
`pack-docs`は一時fileからの安全な置換で既存出力を保護します。

range の範囲指定: `--bytes A:B` は half-open なバイト範囲 `[A, B)`、
`--lines A:B` は 1-based で両端を含みます。`qzt line FILE N` は
`qzt range FILE --lines N:N` と同じ raw line bytes を返します（1行だけの
range の便利なショートカット）。空のcontainerでは`qzt line FILE 1`はstdoutへ
何も書かず、lineが範囲外だと報告してexit code **1**で終了します。
index の `n`（デフォルト 3）より
短い n-gram query は index では回答できないため、確信を持った 0 件ではなく
`incomplete_reason=query_shorter_than_ngram_n` と警告を出力します。

## 終了コード

```text
Exit codes:
  0  success (verify: container is valid)
  1  command failed (verify: container is corrupt or unreadable)
  2  usage error (unknown option / missing argument)
```

## トラブルシューティング

QZT は引き続き `v0.1 technical preview` です。以下は production-ready な挙動ではなく、
参照実装の制約として扱ってください。

### Sidecar が古い、または別の container に属している

QZI は1つの正確な QZT container に binding されます。古い、または不一致の `.qzi` は
sidecar 経路だけで fail-closed になり、source `.qzt` の Core read / export / range /
verify は sidecar なしで継続できます。現在の container から再構築してください。

```sh
qzt sidecar-rebuild file.qzt -o file.qzt.qzi
```

### `qzt sidecar-rebuild` で高 RSS または OOM

`qzt sidecar-rebuild` はsourceをchunk単位でdecodeしますが、v0.1 builderは
sidecar生成中にterm dictionaryとposting map全体を保持します。そのためpeak memoryは
corpusの語彙量とposting量に応じて増え、1 chunkのdecode量を大きく上回る場合があります。

corpusに見合ったマシンでsidecarを構築し、繰り返しqueryでは再利用してください。
`qzt search --sidecar <file.qzi>` はfile-backed readerを使い、posting map全体を
再構築せず、query対象のposting listとcandidate granuleを読み込みます。これは
technical preview向けの運用指針であり、production memory SLAではありません。

### 検索結果が上限で打ち切られた場合（`capped=true`）

結果上限に達すると、metrics 行（text mode）または JSON の
`"capped": true` に `capped=true` が出ます。これは**失敗ではありません**。
command は上限まで見つかった hit を返して **exit 0** のままです。
`stop_reason=max_search_results` が上限を示します。`incomplete_reason` は
独立の情報です。`complete=false` を宣言したsidecarでは、hitが返っても
capで停止しても`index_complete_declared=false`を保持します。cap以降に一致が
あるかどうかは分かりません。

より多くの hit が必要なら `--max-results <N>` で上限を上げてください（例:
`qzt search file.qzt needle --max-results 100`）。

### `qzt pack -`（標準入力）が拒否される

標準入力からの pack は `--profile core` のみ対応し、`-o <path>` が必須です。
標準出力、別 profile、`--dense-line-index on` は標準入力では利用できず、
usage error として exit code **2** を返します。

### n-gram query が index の `n` より短い

query が sidecar の n-gram `n`（デフォルト 3）より短い場合、index では回答できません。
確信を持った 0 件として扱わず、警告と
`incomplete_reason=query_shorter_than_ngram_n` を返します。

### Token query にindex可能なtokenがない

空白、記号、または非ASCII textだけのtoken queryは、ASCII tokenization契約では
token keyを生成できません。確信を持った0件として扱わず、warningと
`incomplete_reason=query_has_no_indexable_tokens`を返して正常終了します。
これは設定された`n`より短いn-gram queryだけに適用する
`query_shorter_than_ngram_n`とは異なります。substringやCJK形式の入力にはn-gram
searchを使ってください。

### memory profile には Document Index が必要

`memory` profile は pack 時に Document Index が必須です。`qzt pack`では
`qzt pack --profile memory`がexit code **1**で失敗し、
`WriterBuilder`を案内します。ファイル入力には`qzt pack-docs --profile memory`、
Rustからは`WriterBuilder::new().profile("memory").document_index(index)`を使います。
Document Indexが不要なら`core`など別のprofileを選んでください。

## ドキュメント

- 参照map: [ドキュメント索引](docs/README.md)
- コントリビューター向け手順: [コントリビューションガイド](CONTRIBUTING.ja.md)
- 仕様要約: [docs/QZT_v0.1_Core_Spec.ja.md](https://github.com/albert-einshutoin/qzt/blob/main/docs/QZT_v0.1_Core_Spec.ja.md)
- v0.1 byte-layout 互換性方針（英語正本）: [docs/QZT_v0.1_Format_Stability.md](docs/QZT_v0.1_Format_Stability.md)
- memory保証: [docs/QZT_v0.1_Memory_Guarantees.md](docs/QZT_v0.1_Memory_Guarantees.md)
- competitive benchmark方法論: [docs/QZT_v0.1_Competitive_Benchmarks.md](docs/QZT_v0.1_Competitive_Benchmarks.md)
- validation corpus: [docs/QZT_v0.1_Validation_Corpus.md](docs/QZT_v0.1_Validation_Corpus.md)
- public API安定性: [docs/API_STABILITY.md](docs/API_STABILITY.md)
- QZI sidecar 仕様: [docs/QZI_v0.1_Sidecar_Spec.ja.md](docs/QZI_v0.1_Sidecar_Spec.ja.md)
- attestationの署名とanchor: [docs/guides/attestation.md](docs/guides/attestation.md)
- Core readiness: [docs/QZT_v0.1_Core_Readiness.ja.md](docs/QZT_v0.1_Core_Readiness.ja.md)
- Release hardening: [docs/QZT_v0.1_Release_Hardening.ja.md](docs/QZT_v0.1_Release_Hardening.ja.md)
- 現在の優先度と保留: [ロードマップ #31](https://github.com/albert-einshutoin/qzt/issues/31)
- 現在の進捗要約: [tasks/status.ja.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/status.ja.md)
- 過去の実装Phase: [tasks/README.ja.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/README.ja.md)

## 開発

実装は2トラックで進み、Phase 0–23は当時のスコープに対して完了しています。

- **v0.1 Core (Phase 0–13)**: deterministic CBOR、fixed structures、UTF-8 chunker、
  no-dictionary zstd writer、reader open/info/export、verify levels、sparse/dense line
  index、document index、dictionaries、resource limits、transient search extension と
  QZI sidecar。
- **Product Completeness (Phase 14–23)**: open-source hygiene、file-backed seeking
  reader (`QztFileReader`)、streaming verify/export/writer、competitive benchmarks、
  resource governance、curated public API、verified evidence retrieval、portable
  conformance vectors と frozen format-stability statement。

[価値ロードマップ #47](https://github.com/albert-einshutoin/qzt/issues/47)と
子Issue #33–#46、#31のpreview hardening対象10件、[FFI監査 #307](https://github.com/albert-einshutoin/qzt/issues/307)は
完了しています。Coreはrelease candidate、QZI検索と製品全体はtechnical previewです。
公開済み[v0.1.0-pre.2](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.2)
binaryには、その後のmain変更は含まれません。現在の優先度と保留判断は
[#31](https://github.com/albert-einshutoin/qzt/issues/31)、証拠は子Issue・PRに置きます。
英日の[status](https://github.com/albert-einshutoin/qzt/blob/main/tasks/status.md)と
[status.ja.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/status.ja.md)は要約です。
[Phase資料](https://github.com/albert-einshutoin/qzt/tree/main/tasks)と2026年6月の
Post-Phase23 wave計画は履歴として参照してください。
