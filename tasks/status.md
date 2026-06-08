# QZT Task Status

Last updated: 2026-06-07

## Current Rule

Implementation must proceed with TDD and the loop:

```text
implement -> self-review -> code review -> architecture review -> fix -> verify -> update status
```

## Phase Summary

| Phase | Name | State | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 0 | Project foundation and quality gates | Complete | Rust workspace, CI-local commands, empty test harness | Repeatable quality gates and fixture layout |
| 1 | Deterministic CBOR, primitives, and errors | Complete | Canonical CBOR rejection tests and fixed primitive helpers | Typed errors, checked arithmetic, property tests |
| 2 | Header, footer trailer, and physical ranges | Complete | Encode/decode fixed structures | Range validator and corruption tests |
| 3 | Metadata, footer payload, index root, and chunk table skeleton | Complete | Deterministic CBOR schemas and empty-container skeleton | Structural verifier without zstd chunks |
| 4 | UTF-8 chunker and sparse Chunk Table writer | Complete | deterministic Chunk Plan for UTF-8 input | CRLF-safe, continuation-aware line metadata |
| 5 | No-dictionary zstd writer and finish | Complete | pack/export equality for simple UTF-8 | zstd frames, BLAKE3, footer finish, pack smoke metric |
| 6 | Reader open/info/export and verification levels | Complete | open/info/export on valid files | quick/normal/deep verify corruption coverage |
| 7 | Sparse line index, range reads, and CLI access | Complete | read_range and read_line_raw | CLI range/line, spanning-line support, intermediate benchmarks |
| 8 | Dictionaries, resource limits, and Reader Core completion | Complete | read embedded dictionary fixtures | Reader Core complete with resource hardening |
| 9 | Core conformance hardening and release readiness | Complete | Full Core test pass and fuzz smoke | v0.1 Core release candidate |
| 10 | Dense Line Index, Document Index, memory profile, and maintenance command scoping | Complete | Dense Line Index acceleration with sparse-vs-dense benchmark | Document Index, memory profile fixtures, repack/merge decision |
| 11 | Search granules and raw token index MVP | Complete | Raw token index over search granules | Verified token search with metrics |
| 12 | N-gram index, planner, and benchmark reporting | Complete | Raw n-gram candidate search | Rarest-first planner and performance reports |
| 13 | Search sidecar and high-performance search goal MVP | Complete | `.qzi` sidecar validation | Memory-mappable high-performance search flow |

## Current Focus

Phase0 through Phase13 are complete. QZT v0.1 Core is release-candidate ready, with optional Dense Line Index, Document Index, memory profile support, raw token search, raw n-gram planner support, and QZI sidecar validation complete.

Next action:

```text
All planned phases are complete and the first release hardening benchmark gate is in place. Next work should be driven by competitive benchmarks or explicit product scope changes.
```

## Completion Tracks

| Track | Required Phases | Current State | Notes |
|---|---|---|---|
| Writer Core | Phase0-Phase5, Phase9 | Complete | v0.1 Writer Core is no-dictionary output; dictionary-emitting writer remains out of Core-ready scope. |
| Reader Core | Phase0-Phase9 | Complete | Embedded dictionary reading, resource limits, partial access, and verify levels are complete for v0.1 Core. |
| Optional Core-defined indexes | Phase10 | Complete | Dense Line Index and Document Index are optional and verified as caches over original bytes. |
| Search Extension | Phase11-Phase13 | Complete | Transient token/ngram correctness paths, planner metrics, and QZI sidecar lookup are complete. |
| Release Hardening | Post-Phase13 | Complete | Deterministic larger-corpus benchmark gate exists in `tests/release_hardening.rs` and `docs/QZT_v0.1_Release_Hardening.md`. |

## Verification Log

| Date | Phase | Commands | Result | Notes |
|---|---:|---|---|---|
| 2026-06-07 | planning | `git diff --check`, code fence count | Pass | Planning docs created after spec review |
| 2026-06-07 | planning | `git diff --check`, phase reference search, code fence count | Pass | External review follow-ups applied to spec and phase plan |
| 2026-06-07 | 0 | `make check` | Pass | Rust single crate, qzt CLI placeholder, library skeleton, fixture layout, and smoke tests added |
| 2026-06-07 | 1 | `make check` | Pass | Deterministic CBOR encoder/validator, closed-schema helper, primitive helpers, typed errors, and property tests added |
| 2026-06-07 | 2 | `make check` | Pass | Fixed Header, Footer Trailer, version validation, index_hint_offset hint handling, and physical range validation added |
| 2026-06-07 | 3 | `make check` | Pass | Metadata, Footer Payload, Index Root, empty Chunk Table, and empty skeleton structural open/write added |
| 2026-06-07 | 4 | `make check` | Pass | UTF-8/CRLF-safe ChunkPlan, newline mode, sparse line metadata, and continuation flags added |
| 2026-06-07 | 5 | `make check`; `cargo test --test phase5_writer pack_smoke_benchmark_records_nonzero_throughput -- --nocapture` | Pass | No-dictionary zstd writer, BLAKE3 chunk checksums, Header patch, Footer finish, export equality, and pack smoke 26.837 MiB/s for 64KiB fixture |
| 2026-06-07 | 6 | `make check` | Pass | QztReader open/info/export, quick/normal/deep verify, compressed checksum detection, and container_checksum detection added |
| 2026-06-07 | 7 | `make check`; `cargo test --test phase7_access phase7_intermediate_benchmark_records_nonzero_metrics -- --nocapture` | Pass | Range/text range/line Reader APIs, CLI range/line smoke, Chunk Table binary search, and Phase7 benchmark recorded: pack 10.695 MiB/s, export 30.631 MiB/s, range 8.736 MiB/s, line 27.167 us |
| 2026-06-07 | 8 | `cargo test --test phase8_reader_core`; `make check` | Pass | Embedded Dictionary Block parsing, dictionary-assisted zstd decode, missing/duplicate/checksum dictionary rejection, unknown optional/required block handling, and ResourceLimits for chunk/index/dictionary/decode paths added |
| 2026-06-07 | 9 | `cargo test --test phase9_cli_core`; `cargo test --test phase9_hardening`; `cargo test --test phase5_writer pack_smoke_benchmark_records_nonzero_throughput -- --nocapture`; `cargo test --test phase7_access phase7_intermediate_benchmark_records_nonzero_metrics -- --nocapture`; `make check` | Pass | Core CLI pack/info/export/range/line/verify, conformance map 1-77, deterministic malformed open/verify fuzz smoke, readiness note, and benchmark smoke recorded: pack 10.909 MiB/s, export 36.692 MiB/s, range 17.517 MiB/s, line 5.250 us |
| 2026-06-07 | 10 | `cargo test --test phase10_dense_line_index -- --nocapture`; `cargo test --test phase10_document_index`; `cargo test --test phase10_dense_line_index sparse_vs_dense_line_lookup_benchmark_records_threshold_evidence -- --nocapture`; `make check` | Pass | Dense Line Index optional block, reader fast path, deep verify disagreement detection, Document Index CBOR schema/deep verify, memory profile flags, and benchmark threshold recorded: 2048 lines sparse 3370.167 us vs dense 2365.709 us |
| 2026-06-07 | 11 | `cargo test --test phase11_search -- --nocapture`; `make check` | Pass | Line Search Granules, transient raw_utf8 token dictionary, exact key comparison despite key_hash collision, sorted postings, delta-varint posting round-trip, verified original-byte token hits, normalized index rejection, qzt search CLI, and required metrics complete |
| 2026-06-07 | 12 | `cargo test --test phase12_ngram_planner -- --nocapture`; `cargo test --test phase11_search -- --nocapture`; `make check` | Pass | Raw Unicode-scalar n-gram index, adjacent_decode line-granule substring verification, complete/incomplete missing-key behavior, rarest-first planner, high-DF driver avoidance, deterministic skip metadata, qzt search ngram CLI, and benchmark metrics complete |
| 2026-06-07 | 13 | `cargo test --test phase13_sidecar -- --nocapture`; `cargo test --test phase12_ngram_planner -- --nocapture`; `cargo test --test phase11_search -- --nocapture`; `make check` | Pass | QZI sidecar header/manifest/sections, source id/checksum rejection, Core fallback without sidecar, sidecar token/ngram lookup, sidecar rebuild CLI, search --sidecar CLI, common-term capping, rare-term candidate-only decode, and metrics complete |
| 2026-06-07 | release hardening | `cargo test --test release_hardening -- --nocapture`; `make bench-release`; `make check` | Pass | 2.4MB deterministic corpus benchmark recorded: pack 22.732 MiB/s, export 60.833 MiB/s, range 59.361 MiB/s, rare token decodes 97 bytes vs 2423996-byte raw scan, common n-gram caps before decode, token sidecar ratio 1.558508, n-gram sidecar ratio 1.522250 |
| 2026-06-07 | release blocker review fixes | `cargo test --test phase5_writer --test phase9_cli_core --test phase11_search`; `make check` | Pass | Fixed multi-token token-search hit reporting, Metadata writer option serialization, CLI profile/dense wiring, CLI error detail preservation, deep verify integer conversion, O(n log n) physical range overlap validation, info metadata reporting, and hid placeholder streaming writer API |

## Review Follow-ups

| Item | State | Notes |
|---|---|---|
| C-1 `verified_spans` multi-token hits | Fixed | Added regression coverage for `alpha beta` returning both token hit ranges. |
| C-2 Metadata writer options | Fixed | Metadata now records `zstd_level`, `target_chunk_size`, and `max_chunk_size` from `WriterOptions`. |
| C-3 CLI `--profile` / `--dense-line-index` | Fixed | `qzt pack --profile memory` defaults to Dense Line Index and explicit `--dense-line-index` overrides it. |
| C-4 CLI error detail | Fixed | CLI command failures now preserve `QztError` / I/O details instead of printing only `failed`. |
| H-1 deep verify integer conversion | Fixed | Removed `unwrap_or(u64::MAX)` fallback. |
| H-2 physical range overlap complexity | Fixed | Range overlap detection now sorts by physical offset and checks adjacent pairs. |
| H-3 varuint duplication | Deferred | Refactor only; keep for a focused primitives cleanup phase to avoid mixing error-code semantics with release-blocker fixes. |
| H-4 `TextAnalysis` / `LineInfo` duplication | Deferred | Refactor only; should be handled with private-unit coverage around newline classification. |
| H-5 `QztWriter` placeholder | Fixed | Public placeholder is `#[doc(hidden)]` until a streaming writer API is implemented. |
| H-6 `qzt info` hardcoded metadata | Fixed | `qzt info` reads Metadata via skeleton details and prints profile, zstd level, chunk sizes, line index, and document index presence. |
| M-1 CBOR limits wiring | Deferred | Needs a separate decode-with-limits API pass for CBOR validation. |
| M-4 file-path/seeking reader | Deferred | Current reader remains in-memory; file-backed seeking reader remains a post-v0.1 scalability phase. |
| P1 chunker target-size soft-limit | Fixed | `choose_chunk_end` now uses `target_chunk_size` as the pack-all threshold instead of `max_chunk_size`, and clamps `max_end` to `input.len()`. Regression test added to phase4_chunker. |
| P1 required block validation | Fixed | `decode_block_descriptor` now rejects any `required=true` block whose type is not `chunk_table`. `is_known_block_type` removed. Regression tests for `token_index` required block and duplicate chunk_table added to phase8_reader_core. |
| P2 Metadata decode indexes/integrity | Fixed | `Metadata::decode` now validates fixed boolean values for all indexes fields and verifies all integrity algorithm fields equal `"blake3"`. |
| P0/P2 README limitations | Fixed | English README now contains a "v0.1 Technical Preview — Limitations" section covering in-memory reader, transient search, token co-occurrence semantics, normalized search, and benchmark gaps. |

## Open Decisions

| Decision | Current Position | When To Revisit |
|---|---|---|
| Reference implementation language | Rust selected | Revisit only with explicit project decision |
| Search Extension timing | After Core conformance | After Phase9 |
| `qzt repack` / `qzt merge` / `qzt compact` | Deferred. They remain post-Core fresh-container rewrite commands and are not implemented in Phase10. | Post-Phase13 or a dedicated maintenance phase |
| Search index persistence | QZI sidecar persistence implemented. Embedded qzt-search-block-v1 remains optional future work. | Release hardening |
| Sidecar index priority | Implemented for token/ngram search sidecars | Larger-corpus benchmark pass |
| Competitive benchmark | Not implemented. QZT has reproducible internal benchmarks but no comparison against SQLite FTS, Tantivy, Lucene, seekable zstd, or split zstd frames. | Product validation |
