# QZT

[English](README.md)

QZT は、大きなテキストを「冷たい証拠コンテナ」として保存するためのバイナリフォーマットです。このリポジトリは Rust による参照実装です。

QZT の目的は、zstd より良い圧縮アルゴリズムを作ることではありません。元テキストを独立した zstd chunk に分け、検証可能なメタデータ、Chunk Table、Footer、検索 sidecar を組み合わせることで、必要な範囲だけを復元し、証拠位置へ戻れるようにすることです。

## 現在の位置づけ

```text
- QZT v0.1 Core: release candidate
- Search Extension / QZI sidecar: technical preview
- Product status: 実験的な参照実装
```

外部に出すときは、production-ready ではなく `v0.1 technical preview` として扱うのが妥当です。

## ビルド / クイックスタート

リポジトリのルートで release binary をビルドします。

```sh
cargo build --release
./target/release/qzt --help
```

`PATH` にインストールしていない場合、binary は `./target/release/qzt` にあります。
以下の例ではそのパスを使います。

QZT は大きなテキストを **seekable かつ verifiable な証拠コンテナ** にまとめます
（`v0.1 technical preview` / 実験的な参照実装であり、production-ready ではありません）。

```sh
./target/release/qzt pack input.txt -o output.qzt
./target/release/qzt info output.qzt
./target/release/qzt range output.qzt --lines 1:10
./target/release/qzt sidecar-rebuild output.qzt -o output.qzt.qzi
./target/release/qzt search output.qzt "error" --sidecar output.qzt.qzi
```

`pack` でコンテナを作成し、`info` と `range` で全体展開せずに確認・部分読み取り、
`sidecar-rebuild` で検索 index を構築、`search --sidecar` で検索します。

## v0.1 Technical Preview の制限

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
- **Sidecar のサイズ**: QZI token / n-gram sidecar は非圧縮の MVP 構造です。
  現実的な 45 MB のログコーパスでは token sidecar は原文の約 2.1 倍でした。
  sidecar のストレージはその前提で見積もってください。
- **Production benchmark は未実施**: v0.1 では SQLite FTS、Tantivy、Lucene、
  seekable-zstd との比較はまだ実施していません。

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

## 主な CLI

```sh
qzt pack input.txt -o output.qzt
journalctl --since today | qzt pack - -o today.qzt
qzt info output.qzt
qzt info output.qzt --format json
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

CLI でよくある失敗パターンです。QZT は `v0.1 technical preview` のままであり、
production-ready ではありません。以下は参照実装の想定制約として扱ってください。

### `qzt pack -`（stdin）が拒否される

stdin pack は streaming core path のみ対応です: `--profile core` かつ Dense Line
Index なし（`--dense-line-index on` は不可）。`-o <path>` は常に必須で、stdout
出力はサポートしません。別 profile、dense line index、`-o` 欠落は exit **2** で
usage 系エラーになります。

### `qzt sidecar-rebuild` で高 RSS または OOM

`qzt sidecar-rebuild` は posting map 全体をメモリに構築します。decode は
チャンク単位ですが、構築時の RSS は語彙量と posting-map サイズに応じて増えます
（おおよそ sidecar サイズの展開分）。これは v0.1 technical preview の既知制約であり、
production の障害ではありません。

sidecar の構築はコーパスに見合ったマシンで実行してください。繰り返し検索する場合は、
一度 `qzt sidecar-rebuild` で sidecar を作成し、その後
`qzt search --sidecar <file.qzi>` を使います。sidecar 検索は bounded-memory な
`QztFileReader` 上で動作し、query された posting list と候補 granule レコードだけを
fetch します。

### index の `n` より短い n-gram query

query が sidecar の n-gram `n`（デフォルト 3）より短い場合、index では回答できません。
CLI は確信を持った 0 件を返さず、`incomplete_reason=query_shorter_than_ngram_n` と
警告を出力します。

### 検索結果が上限で打ち切られた場合（`capped=true`）

hit 数が結果上限を超えると、metrics 行（text mode）または JSON の
`"capped": true` に `capped=true` が出ます。これは**失敗ではありません**。
command は上限まで見つかった hit を返して **exit 0** のままです。
`incomplete_reason` は `none` のままで、n-gram query が短すぎるケースとは別物です
（index は回答できており、設定された上限に達しただけです）。

より多くの hit が必要なら `--max-results <N>` で上限を上げてください（例:
`qzt search file.qzt needle --max-results 100`）。

### memory profile には Document Index が必要

memory profile（`"memory"`）は pack 時に Document Index が必須です。`qzt pack` CLI は
Document Index を受け取れないため、`qzt pack --profile memory` は拒否されます
（`MetadataInvalid`、exit **1**）。writer API（`pack_bytes_with_memory_profile`）で
`DocumentIndex` を渡すか、別 profile（例: `core`）で pack してください。

## ドキュメント

- 仕様要約: [docs/QZT_v0.1_Core_Spec.ja.md](docs/QZT_v0.1_Core_Spec.ja.md)
- QZI sidecar 仕様: [docs/QZI_v0.1_Sidecar_Spec.ja.md](docs/QZI_v0.1_Sidecar_Spec.ja.md)
- Core readiness: [docs/QZT_v0.1_Core_Readiness.ja.md](docs/QZT_v0.1_Core_Readiness.ja.md)
- Release hardening: [docs/QZT_v0.1_Release_Hardening.ja.md](docs/QZT_v0.1_Release_Hardening.ja.md)
- 実装 Phase: [tasks/README.ja.md](tasks/README.ja.md)
- 進捗: [tasks/status.ja.md](tasks/status.ja.md)

## Phase 計画

実装は 2 トラックで進み、全 Phase が完了しています。

- **v0.1 Core (Phase 0–13)**: deterministic CBOR、fixed structures、UTF-8 chunker、
  no-dictionary zstd writer、reader open/info/export、verify levels、sparse/dense line
  index、document index、dictionaries、resource limits、transient search extension と
  QZI sidecar。
- **Product Completeness (Phase 14–23)**: open-source hygiene、file-backed seeking
  reader (`QztFileReader`)、streaming verify/export/writer、competitive benchmarks、
  resource governance、curated public API、verified evidence retrieval、portable
  conformance vectors と frozen format-stability statement。

Phase ドキュメントは [tasks/](tasks/) にあり、日本語版は同じディレクトリの `*.ja.md` です。
進捗は [tasks/status.md](tasks/status.md) と [tasks/status.ja.md](tasks/status.ja.md) で管理します。
