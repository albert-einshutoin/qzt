# QZT v0.1 Release Hardening

[English](QZT_v0.1_Release_Hardening.md)

日付: 2026-06-07

## 目的

この文書は Phase13 後の release hardening gate を記録します。目的は絶対的な性能値を約束することではなく、release evidence を再現可能にすることです。

確認対象:

```text
- larger synthetic corpus
- pack/export/range smoke metrics
- token sidecar rare-query evidence
- token sidecar missing-query evidence
- n-gram sidecar common-query cap evidence
- sidecar size ratios
- クエリケース毎の timing quantile を evidence として記録
```

## コマンド

```bash
cargo test --test release_hardening -- --nocapture
```

同じテストは `make check` にも含まれます。

プロファイル専用実行（ベンチ補助、gating対象外）:

```bash
make bench-profile
```

`QZT_RELEASE_BENCH_QUERY_REPETITIONS` / `QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS` で
反復回数を上書きできます。

```bash
QZT_RELEASE_BENCH_QUERY_REPETITIONS=500 \
QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=20 \
make bench-profile
```

## Corpus

release hardening test は deterministic な合成テキスト corpus を使います。

```text
lines: 24000
bytes: 2423996
chunk size: 8192
rare token: rare-token-unique
common n-gram: aaa
```

反復の多い corpus にすることで、圧縮と high document-frequency search behavior の両方を検証します。

## 最新ローカル出力

`cargo test --test release_hardening -- --nocapture` で出力される `release_bench` 1行には、既存 counter と query-case テレメトリの両方が含まれます。

これはローカル smoke evidence であり、release SLA ではありません。

## 最新ローカルプロファイル結果（3回実行、release build）

`QZT_RELEASE_BENCH_QUERY_REPETITIONS=500` と
`QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=20` で `make bench-profile` を3回実行した結果。

```text
case               run1 p50/p95/p99 (µs)   run2 p50/p95/p99 (µs)   run3 p50/p95/p99 (µs)
rare-token         46 / 50 / 58            45 / 48 / 58            45 / 49 / 59
missing-token      36 / 38 / 43            36 / 39 / 46            36 / 39 / 56
common-ngram       194 / 271 / 571         191 / 205 / 277         192 / 204 / 278
```

この設定では 3 ケース × 500 repetitions × 3 実行分、計 4500 クエリで
候補カウンタの一致を確認できています（反復内決定性ガード）。

## Release gate assertions

自動 gate は以下を検証します。

```text
- corpus が 1,000,000 bytes 以上
- export が original bytes を完全復元する
- rare token query が 1 hit を verify する
- rare token sidecar search が raw scan より decode bytes を減らす
- common n-gram query が candidate decode 前に cap される
- token/missing/n-gram クエリケースの telemetry が報告される
- token/missing/n-gram クエリの timing は warmup + repeat で p50/p95/p99 を記録し、correctness assert には利用しない
- token / n-gram sidecar size が報告される
- pack/export/range throughput metric が 0 ではない

- 将来の release run では指標ゲートを決定論的に保つこと:
  - candidate/cap/decode カウンタは semantic check で比較し、timing は evidence のみ扱いにする
  - index size 比較は path-aware にする:
    - in-memory 見積りと file-sidecar manifest サイズは意図的に非同値
    - 高 skip ワークロードでは index size の大小関係が反転しうる
```

## Self-review

```text
- benchmark は deterministic で、外部ファイルや network に依存しない
- timing は報告するが、machine-specific speed threshold は correctness assertion にしない
- search evidence は QztReader 経由で original-byte verified
- common-query cap behavior を明示し、高頻度語で大量 decompression が暗黙に起きないようにする
```

## 残る product evidence gap

この gate は SQLite FTS、Tantivy、Lucene、seekable zstd、split-frame object storage との比較を含みません。

競合 benchmark は次の product-level release question です。
