# QZT タスク進捗

[English](status.md)

最終更新: 2026-06-07

## 現在のルール

実装は TDD と以下の loop で進めます。

```text
implement -> self-review -> code review -> architecture review -> fix -> verify -> update status
```

## Phase summary

| Phase | 名前 | 状態 | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 0 | Project foundation and quality gates | Complete | Rust crate、local quality command、test harness | 再現可能な quality gate と fixture layout |
| 1 | Deterministic CBOR, primitives, and errors | Complete | canonical CBOR rejection と fixed primitive | typed errors、checked arithmetic、property tests |
| 2 | Header, footer trailer, and physical ranges | Complete | fixed structure encode/decode | range validator と corruption tests |
| 3 | Metadata, footer payload, index root, and chunk table skeleton | Complete | deterministic CBOR schema と empty skeleton | zstd chunk なしの structural verifier |
| 4 | UTF-8 chunker and sparse Chunk Table writer | Complete | deterministic Chunk Plan | CRLF-safe、continuation-aware line metadata |
| 5 | No-dictionary zstd writer and finish | Complete | simple UTF-8 の pack/export equality | zstd frames、BLAKE3、Footer finish、pack metric |
| 6 | Reader open/info/export and verification levels | Complete | valid file の open/info/export | quick/normal/deep verify corruption coverage |
| 7 | Sparse line index, range reads, and CLI access | Complete | read_range と read_line_raw | CLI range/line、spanning-line、benchmark |
| 8 | Dictionaries, resource limits, and Reader Core completion | Complete | embedded dictionary fixture を読む | Reader Core と resource hardening |
| 9 | Core conformance hardening and release readiness | Complete | Core test pass と fuzz smoke | v0.1 Core release candidate |
| 10 | Dense Line Index, Document Index, memory profile, maintenance scope | Complete | Dense Line Index acceleration | Document Index、memory profile、maintenance decision |
| 11 | Search granules and raw token index MVP | Complete | Raw token index | verified token search with metrics |
| 12 | N-gram index, planner, benchmark reporting | Complete | Raw n-gram candidate search | rarest-first planner と performance reports |
| 13 | Search sidecar and high-performance search goal MVP | Complete | `.qzi` sidecar validation | memory-mappable high-performance search flow |

## Current focus

Phase0 から Phase13 は完了しています。QZT v0.1 Core は release candidate ready です。Dense Line Index、Document Index、memory profile、raw token search、raw n-gram planner、QZI sidecar validation も完了しています。

次の作業は、競合 benchmark または明示的な product scope change によって決めるべきです。

## Completion tracks

| Track | 状態 | Notes |
|---|---|---|
| Writer Core | Complete | v0.1 Writer Core は no-dictionary output。dictionary-emitting writer は Core-ready scope 外。 |
| Reader Core | Complete | embedded dictionary reading、resource limits、partial access、verify levels が完了。 |
| Optional Core-defined indexes | Complete | Dense Line Index と Document Index は optional cache として検証済み。 |
| Search Extension | Complete | token/ngram correctness path、planner metrics、QZI sidecar lookup が完了。 |
| Release Hardening | Complete | `tests/release_hardening.rs` と release hardening note が存在。 |

## Verification summary

すべての Phase は `make check` または targeted tests で検証されています。直近の release blocker review fix では以下を修正し、`make check` が通っています。

```text
- multi-token token-search hit reporting
- Metadata writer option serialization
- CLI profile/dense wiring
- CLI error detail preservation
- deep verify integer conversion
- O(n log n) physical range overlap validation
- info metadata reporting
- placeholder streaming writer API の doc hidden 化
```

## Review follow-ups

Fixed:

```text
- C-1 verified_spans multi-token hits
- C-2 Metadata writer options
- C-3 CLI --profile / --dense-line-index
- C-4 CLI error detail
- H-1 deep verify integer conversion
- H-2 physical range overlap complexity
- H-5 QztWriter placeholder
- H-6 qzt info hardcoded metadata
```

Deferred:

```text
- varuint duplication cleanup
- TextAnalysis / LineInfo duplication cleanup
- CBOR limits wiring
- file-backed seeking reader
```

## Open decisions

```text
- repack / merge / compact は post-Core maintenance phase
- embedded qzt-search-block-v1 は optional future work
- competitive benchmark は未実装
- SQLite FTS / Tantivy / Lucene / seekable zstd / split zstd frames との比較が product validation の次課題
```
