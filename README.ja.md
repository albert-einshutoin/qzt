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

macOS / Linuxに公開済みの[`v0.1.0-pre.2` technical preview](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.2)
をinstallします。

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/albert-einshutoin/qzt/releases/download/v0.1.0-pre.2/qzt-installer.sh | sh
qzt --version
```

展開前にchecksumを検証してください。macOS/Linux、Windows、source buildの
手順は以下で確認できます。

<details>
<summary>checksum検証付き手動導入とsource build</summary>

<br>

checksumを確認して手動導入する場合は、`aarch64-apple-darwin`、
`x86_64-apple-darwin`、`x86_64-unknown-linux-gnu`から対象を選び、展開前に
archiveを検証します。Apple siliconでの例:

```sh
set -eu
release=v0.1.0-pre.2
target=aarch64-apple-darwin
archive="qzt-${target}.tar.xz"
base="https://github.com/albert-einshutoin/qzt/releases/download/${release}"
curl --proto '=https' --tlsv1.2 -fLO "${base}/${archive}"
curl --proto '=https' --tlsv1.2 -fLO "${base}/${archive}.sha256"
expected="$(awk 'NF { print $1; exit }' "${archive}.sha256")"
actual="$(shasum -a 256 "${archive}" | awk '{ print $1 }')"
test "${expected}" = "${actual}"
tar -xJf "${archive}"
./"qzt-${target}"/qzt --version
```

Windowsでは同じReleaseの`.zip`と`.zip.sha256`を取得し、展開前に検証できます。

```powershell
$archive = "qzt-x86_64-pc-windows-msvc.zip"
$expected = (Get-Content "$archive.sha256" | Select-String -Pattern '\S').Line.Split()[0]
$actual = (Get-FileHash -Algorithm SHA256 $archive).Hash
if ($expected -ne $actual) { throw "SHA-256 checksum mismatch" }
```

または`qzt-installer.ps1`を利用できます。プリビルドbinaryを使わず、review済み
tagからbuildして導入する場合:

```sh
cargo install --git https://github.com/albert-einshutoin/qzt --tag v0.1.0-pre.2 --locked
```

</details>

## 60秒ツアー

install後、次のコマンドを上から順に実行します。

```sh
printf 'alpha\nbeta\nerror gamma\n' > app.log
qzt pack app.log -o app.qzt
qzt info app.qzt --format json
qzt range app.qzt --lines 2:2
qzt sidecar-rebuild app.qzt -o app.qzt.qzi
qzt search app.qzt "error" --sidecar app.qzt.qzi
qzt verify app.qzt --deep
qzt attest app.qzt > app.attest.json
```

rangeは`beta`を出力します。searchはsourceが`verified_original_bytes`のhitを返し、
deep verifyは全chunkを検証します。attestationは外部署名や信頼できるtimestampへ
渡せる決定的なJSON claimを1行で出力します。

## ユースケース

- **server log保全** — logを日次containerへstreamし、定期的にdeep verifyして、
  決定的attestationを別系統へanchorします。
- **pipeline artifact固定** — `pack-docs`で各input artifactを、1つのcontainer内の
  名前付き・checksum検証可能なdocumentとして固定します。
- **incident forensics** — 再構築可能なQZI sidecarを検索し、調査に必要な検証済み
  byte範囲または行範囲だけを提示します。

## ステータスと制限

QZT v0.1 Coreはrelease candidateです。QZI検索とproduct全体は実験的な
`v0.1 technical preview`であり、production-readyではありません。

```text
- QZT v0.1 Core: release candidate
- Search Extension / QZI sidecar: technical preview
- Product status: 実験的な参照実装
```

QZI（`.qzi`）はCore container formatの一部ではなく、派生・再構築可能・非信頼の
検索sidecarです。導入前にfail-closed境界とon-disk layoutを
[QZI v0.1 Sidecar Spec](docs/QZI_v0.1_Sidecar_Spec.ja.md)で確認してください。

QZT v0.1 は、仕様カバレッジと正しさを重視した参照実装です。
production use の前に残っている既知の制限は以下です。

- **Index 構築メモリは語彙量に比例する**: `qzt search --sidecar` を含むすべての
  CLI コマンドが bounded-memory な `QztFileReader` 上で動作し、sidecar 検索は
  query された term の posting list と候補 granule レコードだけを fetch します
  （42 MB / 40 万行コーパスで rare query の最大 RSS は 518 MB → 9.8 MB）。
  一方、index の構築（`qzt sidecar-rebuild`、または `--sidecar` なしの
  `qzt search`）は posting map 全体をメモリに保持する（おおよそ sidecar サイズの
  展開分）ため、sidecar の構築はコーパスに見合ったマシンで行ってください。
- **一時 search index**: `--sidecar` なしの `qzt search` は、実行ごとに
  search index を再構築します（チャンク単位の decode ですが index 全体はメモリに
  残ります）。繰り返し検索する場合は、先に `qzt sidecar-rebuild` を一度実行し、
  その後 `qzt search --sidecar <file.qzi>` を使ってください。
- **Token search は phrase search ではなく co-occurrence**: multi-token query
  `"foo bar"` は、両方の token を任意の順序で含む行に match します。token が隣接している
  必要はありません。grep-compatible ではありません。
- **Normalized search は未実装**: `SearchIndexSource::NormalizedUtf8`
  (Unicode normalization、case folding、width folding) はまだ実装されていません。
- **Sidecar のサイズ**: 現行 writer は compact な QZI v2 layout を出力します。
  既存の QZI v1 sidecar も読み込めますが、v2 の容量削減を得るには再構築が必要です。
  release gate では再現可能な 10 MB high-cardinality log corpus に対し token / n-gram
  sidecar を原文の 1.7 倍以下に保ちます。語彙や行形状が異なるデータでは結果も変わります。
- **Production benchmarkは未実施**: v0.1ではSQLite FTS、Tantivy、Lucene、
  seekable-zstd との比較はまだ実施していません。

### 性能数値の再現

上記の RSS 数値はローカル smoke evidence であり、SLA や production 保証では
ありません。release benchmark と profiling は次のコマンドで再現できます。

```sh
cargo test --test release_hardening -- --nocapture
make bench-profile
```

軽量な反復用:

```sh
QZT_RELEASE_BENCH_QUERY_REPETITIONS=5 QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=2 make bench-profile
```

コーパス詳細、指標の定義、追加の profiling 対象は
[release-hardening guide](docs/QZT_v0.1_Release_Hardening.ja.md) を参照してください。

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
cargo test --features bench-compete --test phase18_competitive_benchmark -- --nocapture
```

詳細と QZT を使うべき場面は
[competitive benchmark methodology](docs/QZT_v0.1_Competitive_Benchmarks.md)
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

local checkoutから使う場合は、先にrelease binaryをbuildします。

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

全option、JSON schema、profileの実挙動、stdout/stderr規則、v0.1自動化向け
安定性契約は[docs/CLI.ja.md](docs/CLI.ja.md)を参照してください。この節は
commandの早見表だけに留めます。

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

range の範囲指定: `--bytes A:B` は half-open なバイト範囲 `[A, B)`、
`--lines A:B` は 1-based で両端を含みます。`qzt line FILE N` は
`qzt range FILE --lines N:N` と同じ raw line bytes を返します（1行だけの
range の便利なショートカット）。index の `n`（デフォルト 3）より
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

hit 数が結果上限を超えると、metrics 行（text mode）または JSON の
`"capped": true` に `capped=true` が出ます。これは**失敗ではありません**。
command は上限まで見つかった hit を返して **exit 0** のままです。
`incomplete_reason` は `none` のままで、n-gram query が短すぎるケースとは別物です
（index は回答できており、設定された上限に達しただけです）。

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

### memory profile には Document Index が必要

`memory` profile は pack 時に Document Index が必須です。`qzt pack`では
`qzt pack --profile memory`がexit code **1**で失敗し、
`pack_bytes_with_memory_profile`を案内します。ファイル入力には
`qzt pack-docs --profile memory`、ライブラリでは`DocumentSpan`/`DocumentIndex`、
または`core`など別のprofileを使ってください。

## ドキュメント

- 仕様要約: [docs/QZT_v0.1_Core_Spec.ja.md](https://github.com/albert-einshutoin/qzt/blob/main/docs/QZT_v0.1_Core_Spec.ja.md)
- v0.1 byte-layout 互換性方針（英語正本）: [docs/QZT_v0.1_Format_Stability.md](docs/QZT_v0.1_Format_Stability.md)
- QZI sidecar 仕様: [docs/QZI_v0.1_Sidecar_Spec.ja.md](docs/QZI_v0.1_Sidecar_Spec.ja.md)
- attestationの署名とanchor: [docs/guides/attestation.md](docs/guides/attestation.md)
- Core readiness: [docs/QZT_v0.1_Core_Readiness.ja.md](docs/QZT_v0.1_Core_Readiness.ja.md)
- Release hardening: [docs/QZT_v0.1_Release_Hardening.ja.md](docs/QZT_v0.1_Release_Hardening.ja.md)
- 実装 Phase: [tasks/README.ja.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/README.ja.md)
- 進捗: [tasks/status.ja.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/status.ja.md)

## 開発

実装は 2 トラックで進み、全 Phase が完了しています。

- **v0.1 Core (Phase 0–13)**: deterministic CBOR、fixed structures、UTF-8 chunker、
  no-dictionary zstd writer、reader open/info/export、verify levels、sparse/dense line
  index、document index、dictionaries、resource limits、transient search extension と
  QZI sidecar。
- **Product Completeness (Phase 14–23)**: open-source hygiene、file-backed seeking
  reader (`QztFileReader`)、streaming verify/export/writer、competitive benchmarks、
  resource governance、curated public API、verified evidence retrieval、portable
  conformance vectors と frozen format-stability statement。

Phase ドキュメントは [tasks/](https://github.com/albert-einshutoin/qzt/tree/main/tasks) にあり、日本語版は同じディレクトリの `*.ja.md` です。
進捗は [tasks/status.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/status.md) と
[tasks/status.ja.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/status.ja.md) で管理します。
