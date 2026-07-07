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

## クイックスタート

1 つのテキストファイルで pack → inspect → export → diff まで試す最短パスです。
QZT は `v0.1 technical preview` であり、production-ready ではない実験的な参照実装として扱ってください。

プレーンテキスト（例: `input.txt`）を用意し、次を実行します。

```sh
qzt pack input.txt -o output.qzt
qzt info output.qzt
qzt export output.qzt -o restored.txt
diff input.txt restored.txt
```

`diff` で出力がなければ、復元されたバイト列が元ファイルと一致しています。

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
`--lines A:B` は 1-based で両端を含みます。index の `n`（デフォルト 3）より
短い n-gram query は index では回答できないため、確信を持った 0 件ではなく
`incomplete_reason=query_shorter_than_ngram_n` と警告を出力します。

## 終了コード

```text
Exit codes:
  0  success (verify: container is valid)
  1  command failed (verify: container is corrupt or unreadable)
  2  usage error (unknown option / missing argument)
```

## ドキュメント

- 仕様要約: [docs/QZT_v0.1_Core_Spec.ja.md](docs/QZT_v0.1_Core_Spec.ja.md)
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
