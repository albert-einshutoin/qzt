# QZT v0.1 Competitive Benchmark Harness

[English](QZT_v0.1_Competitive_Benchmarks.md)

日付: 2026-06-08

Phase18 harness は再現可能な計測であり、SLA ではありません。Phase23 validation corpus generator を使い、同一バイト列に対して QZT の range restore と whole-file raw zstd restore を比較します。

## QZT を使う場面

QZT v0.1 は technical preview です。database 型の indexing や whole-file decompression より、検証済み original-byte evidence が重要な場合に使います。

| Workload | QZT を使う場面 | 注意 |
| --- | --- | --- |
| Evidence containers | source bytes と integrity check を保持する cold な immutable container が必要 | Technical preview; production SLA や performance guarantee はない |
| Large immutable logs | append-once text を archive 全体の再圧縮や decode なしで seekable に保つ必要がある | Benchmark timing は再現可能な evidence であり、performance promise ではない |
| Byte-exact range restore | compressed storage から verified slice が必要で、full-file decode は不要 | Restore は要求 range と重なる chunk のみ decode する |
| Verified document retrieval | stable ID が original-byte range に解決し、partial decode で round-trip する必要がある | Document Index は optional extension であり、original text の代替ではない |
| Rebuildable sidecar search | Search を derived QZI index に置きつつ、hit を container bytes で検証できる | QZI sidecar は rebuildable で optional、source text より大きくなる場合がある |

デフォルト smoke を実行:

```sh
cargo test --test phase18_competitive_benchmark -- --nocapture
```

SQLite FTS5 と ripgrep に対する外部ツール比較は `bench-compete` feature の裏で実行され、デフォルト quality gate の portability を保ちます。ツールが無い場合は skip され、存在するツールは reference byte scan と同じ hit count を返す必要があります。

```sh
cargo test --features bench-compete --test phase18_competitive_benchmark -- --nocapture
```

## Methodology

- 明示的な seed から決定論的な C1-C6 corpus bytes を生成する。
- 固定 chunk size で QZT に pack する。
- 同一 corpus を whole-file zstd で圧縮する。
- 同一 byte range を `QztFileReader` と whole-file zstd decode で restore する。
- timing を記録する前に byte equality を assert する。
- sidecar/search correctness を timing とは別に記録する。
- `bench-compete` 有効時は同一 corpus に対して ripgrep と SQLite FTS5 を実行し、hit count が reference byte scan と一致しない場合は harness を fail する。

## QZT を使わない場面

QZT は mutable ranking、正規化された language search、高頻度 update indexing が主 workload の full-text database の代替ではありません。QZI sidecar は rebuildable で source text より大きくなる場合があるため、index compactness より verified original-byte evidence が重要な場合に使います。
