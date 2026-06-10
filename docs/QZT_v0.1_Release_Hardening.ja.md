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
- n-gram sidecar common-query cap evidence
- sidecar size ratios
```

## コマンド

```bash
cargo test --test release_hardening -- --nocapture
```

同じテストは `make check` にも含まれます。

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

```text
release_bench corpus_bytes=2423996 lines=24000 packed_bytes=132320 compression_ratio=0.054588 qzi_token_bytes=3777818 qzi_token_ratio=1.558508 qzi_ngram_bytes=3689927 qzi_ngram_ratio=1.522250 pack_mib_s=22.732 export_mib_s=60.833 range_mib_s=59.361 rare_token_candidate_granules=1 rare_token_candidate_chunks=1 rare_token_decoded_bytes=97 rare_token_verified_matches=1 common_ngram_candidate_granules=24000 common_ngram_decoded_bytes=0 common_ngram_capped=true raw_scan_decoded_bytes=2423996
```

これはローカル smoke evidence であり、release SLA ではありません。

## Release gate assertions

自動 gate は以下を検証します。

```text
- corpus が 1,000,000 bytes 以上
- export が original bytes を完全復元する
- rare token query が 1 hit を verify する
- rare token sidecar search が raw scan より decode bytes を減らす
- common n-gram query が candidate decode 前に cap される
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
