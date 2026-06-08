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

## Product Completeness Track (post-v0.1)

これらの Phase は、Memory Pager や AI memory systems に embed される Cold Evidence Container という
spec の product goal に向けて maturity を上げます。container format bytes は変更しません。この track は
engine sub-track (14-19) と consumer sub-track (20-22) を持ちます。

Engine sub-track:

| Phase | 名前 | 状態 | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 14 | Open-source release hygiene | Pending | LICENSE、CI running make check、package metadata | Contributor docs、MSRV matrix、doc build、packageability check。crates.io publish dry-run は Phase20 後まで defer |
| 15 | File-backed seeking reader | Pending | ReadAt trait と、index region だけ読む QztFileReader open | Bounded-memory range/line/export、CLI file reader 接続 |
| 16 | Streaming verification and export | Pending | full-original Vec なしの streaming verify_deep | Bounded-memory export と file-backed deep verify |
| 17 | Streaming writer | Pending | QztFileWriter push/finish | pack_bytes と byte-identical、bounded memory、streaming pack CLI |
| 18 | Competitive benchmark harness | Pending | large corpus で QZT vs raw-zstd range restore | QZT vs SQLite FTS5 / ripgrep、correctness gate |
| 19 | Resource governance and large-input hardening | Pending | ResourceLimits-driven CBOR budget、max_search_results cap | cargo-fuzz target、large-input round-trip、documented memory bounds |

Consumer sub-track:

| Phase | 名前 | 状態 | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 20 | Public API stabilization | Pending | Internal modules pub(crate)、curated surface、writer builder | missing_docs、semver/stability policy、surface snapshot test、docs.rs |
| 21 | Verified evidence retrieval and Memory Pager integration | Pending | read_range_verified / read_document_verified、evidence_ref example | E2E integration test、doc_id resolution、concurrent verified reads、documented integration pattern |
| 22 | Portable conformance vectors and format stability | Pending | Golden .qzt vectors、manifest、vector runner | Core map + corruption coverage、third-party procedure、frozen v0.1 format-stability statement |

Validation (cross-cutting):

| Phase | 名前 | 状態 | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 23 | Acceptance threshold harness | Pending | Phase23a deterministic C1-C6 corpora、HARD invariants asserted、SOFT targets recorded | Phase23b evidence invariants on C1 after Phase21、shared generators for Phase18/22 |

Dependency order: 14 (independent) -> 15 (foundation)。その後 sub-tracks を parallel に進めます。Engine は 15 -> 16, 17 -> 18 -> 19（18 は Phase23a corpora を再利用）。Consumer は 20 -> 21 -> 22。20 は 14 に依存し、21 は 15 と 20 に依存し、22 は 20、Phase23a、Phase9 conformance map に依存します。Validation は 15 直後に 23a で corpus generators と non-evidence HARD invariants を作り、21 後に 23b で C1 evidence invariants を追加します。Acceptance thresholds は docs/QZT_v0.1_Validation_Corpus.md で定義します。

## Current focus

Phase0 から Phase13 は完了しています。QZT v0.1 Core は release candidate ready です。Dense Line Index、Document Index、memory profile、raw token search、raw n-gram planner、QZI sidecar validation も完了しています。

Product Completeness Track (Phase14-Phase23) も計画済みです。engine sub-track (14-19) は I/O、
hygiene、competitive-validation gaps を閉じます。consumer sub-track (20-22) は QZT を外部 system が
embed できる stable / verifiable dependency にします。Phase23 は shared acceptance corpus と threshold
harness を提供します。

Next action:

```text
Phase14 (open-source release hygiene) から開始する。independent で低コスト、API 安定前に crate を publish せずに public release を unblock する。次に Phase15 (open/range/line/export の file-backed seeking reader) を実施し、「large text」 claim を閉じる。Phase15 後は Phase20 と Phase23a を parallel に進め、Phase20 -> Phase21 で product を定義する verified-evidence-retrieval operation と Memory Pager integration proof を届ける。
```

## Completion tracks

| Track | 状態 | Notes |
|---|---|---|
| Writer Core | Complete | v0.1 Writer Core は no-dictionary output。dictionary-emitting writer は Core-ready scope 外。 |
| Reader Core | Complete | embedded dictionary reading、resource limits、partial access、verify levels が完了。 |
| Optional Core-defined indexes | Complete | Dense Line Index と Document Index は optional cache として検証済み。 |
| Search Extension | Complete | token/ngram correctness path、planner metrics、QZI sidecar lookup が完了。 |
| Release Hardening | Complete | `tests/release_hardening.rs` と release hardening note が存在。 |
| Product Completeness: engine | Pending | Phase14-Phase19。open-source hygiene、file-backed seeking reader、streaming verify/export/writer、competitive benchmarks、large-input resource governance。 |
| Product Completeness: consumer | Pending | Phase20-Phase22。curated public API、Memory Pager integration proof 付き verified evidence retrieval、portable conformance vectors と frozen format-stability statement。 |
| Product Completeness: validation | Pending | Phase23。docs/QZT_v0.1_Validation_Corpus.md の C1-C6 corpora に対する acceptance threshold harness。HARD invariants と provisional SOFT target bands で「期待値を満たす」を測定可能にする。 |

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
- P1 chunker target-size soft-limit
- P1 required block validation
- P2 Metadata decode indexes/integrity
- P0/P2 README limitations
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
