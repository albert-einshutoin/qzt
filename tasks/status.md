# QZT Task Status

Last updated: 2026-06-13

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

## Product Completeness Track (post-v0.1)

These phases raise maturity toward the spec's product goal: the Cold Evidence Container embedded by Memory Pager and AI memory systems. None change container format bytes. The track has an engine sub-track (14-19) and a consumer sub-track (20-22).

Engine sub-track:

| Phase | Name | State | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 14 | Open-source release hygiene | Complete | LICENSE, CI running make check, package metadata | Contributor docs, MSRV matrix, doc build, packageability check; crates.io publish dry-run deferred until after Phase20 |
| 15 | File-backed seeking reader | Complete | ReadAt trait and QztFileReader open reading only the index region | Bounded-memory range/line/export, CLI wired to file reader |
| 16 | Streaming verification and export | Complete | Streaming verify_deep with no full-original Vec | Bounded-memory export and file-backed deep verify |
| 17 | Streaming writer | Complete | QztFileWriter push/finish | Byte-identical to pack_bytes, bounded memory, streaming pack CLI |
| 18 | Competitive benchmark harness | Complete | QZT vs raw-zstd range restore on a large corpus | QZT vs SQLite FTS5 and ripgrep with correctness gate behind `bench-compete` |
| 19 | Resource governance and large-input hardening | Complete | ResourceLimits-driven CBOR budget, max_search_results cap | cargo-fuzz open+verify target, large-input acceptance coverage, documented memory bounds |

Consumer sub-track:

| Phase | Name | State | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 20 | Public API stabilization | Complete | Internal modules hidden by default, curated crate-root surface, writer builder | missing_docs lint, semver/stability policy, root API smoke test, docs.rs |
| 21 | Verified evidence retrieval and Memory Pager integration | Complete | read_range_verified / read_document_verified, evidence_ref example | End-to-end integration test, doc_id resolution, concurrent verified reads, documented integration pattern |
| 22 | Portable conformance vectors and format stability | Complete | Golden .qzt vectors, manifest, vector runner | Core map + corruption coverage, third-party procedure, frozen v0.1 format-stability statement |

Validation (cross-cutting):

| Phase | Name | State | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 23 | Acceptance threshold harness | Complete | Phase23a deterministic C1-C6 corpora, HARD invariants asserted, SOFT targets recorded | Phase23b evidence invariants on C1 after Phase21; shared generators for Phase18/22 |

Dependency order: 14 (independent) -> 15 (foundation). Then sub-tracks in parallel. Engine: 15 -> 16, 17 -> 18 -> 19 (18 reuses Phase23a corpora). Consumer: 20 -> 21 -> 22, where 20 depends on 14, 21 depends on 15 and 20, and 22 depends on 20, Phase23a, and the Phase9 conformance map. Validation: 23a right after 15 for corpus generators and non-evidence HARD invariants; 23b after 21 for C1 evidence invariants. Acceptance thresholds are defined in docs/QZT_v0.1_Validation_Corpus.md.

## Post-Phase23 Execution Track (post-v0.1)

Phase0-Phase23 are complete; execution continues on two GitHub-issue roadmaps.
Their cross-track ordering, wave plan, milestones, and release gates are fixed
in [PostPhase23.md](PostPhase23.md). Per-issue progress lives on the GitHub
issue checklists.

| Track | Scope | State | Source |
|---|---|---|---|
| Refactoring (5 phases, 24 issues #2-#30) | error type and helpers, duplicate removal, trait unification, structural consolidation, perf/CI polish; 1 real bug fix (#8) | In progress (Phase 1 complete: #2-#9 merged; Phase 2 next) | issue #31, [PostPhase23.md](PostPhase23.md) |
| Product value (4 phases, 14 issues #33-#46) | CLI evidence loop with JSON output, attest and conformance kit, crates.io and binary distribution, benchmarks and tutorials | Planned | issue #47, [PostPhase23.md](PostPhase23.md) |

## Current Focus

Phase0 through Phase13 are complete. QZT v0.1 Core is release-candidate ready, with optional Dense Line Index, Document Index, memory profile support, raw token search, raw n-gram planner support, and QZI sidecar validation complete.

The Product Completeness Track (Phase14-Phase23) is complete. The engine sub-track (14-19) closes the I/O, hygiene, and competitive-validation gaps. The consumer sub-track (20-22) makes QZT a stable, verifiable dependency an external system can embed: a curated public API, verified evidence retrieval with a proven Memory Pager integration, and portable conformance vectors with a frozen format-stability statement. Phase23 supplies the shared acceptance corpus and threshold harness.

Post-Phase23 execution is planned: the refactoring roadmap (issue #31) and the
product value roadmap (issue #47) are sequenced in
[PostPhase23.md](PostPhase23.md) toward a v0.1.0 technical-preview release.

Next action:

```text
Execute tasks/PostPhase23.md Wave 1 value lane / Wave 2: Value Phase 1
#33 (qzt info + JSON foundation) -> #34/#35/#36 -> #37 (#38 waits for #22),
then refactor Phase 2 (#10-#16). Do not start #25 while #33-#37 are in
flight. Release gates (v0.1.0 tag, crates.io publish) remain owner-approved
decisions.
```

## Completion Tracks

| Track | Required Phases | Current State | Notes |
|---|---|---|---|
| Writer Core | Phase0-Phase5, Phase9 | Complete | v0.1 Writer Core is no-dictionary output; dictionary-emitting writer remains out of Core-ready scope. |
| Reader Core | Phase0-Phase9 | Complete | Embedded dictionary reading, resource limits, partial access, and verify levels are complete for v0.1 Core. |
| Optional Core-defined indexes | Phase10 | Complete | Dense Line Index and Document Index are optional and verified as caches over original bytes. |
| Search Extension | Phase11-Phase13 | Complete | Transient token/ngram correctness paths, planner metrics, and QZI sidecar lookup are complete. |
| Release Hardening | Post-Phase13 | Complete | Deterministic larger-corpus benchmark gate exists in `tests/release_hardening.rs` and `docs/QZT_v0.1_Release_Hardening.md`. |
| Product Completeness: engine | Phase14-Phase19 | Complete | Open-source hygiene, file-backed seeking reader, streaming verify/export/writer, competitive benchmarks, and large-input resource governance. Closes the in-memory and competitive-validation gaps. |
| Product Completeness: consumer | Phase20-Phase22 | Complete | Curated public API, verified evidence retrieval with Memory Pager integration proof, and portable conformance vectors with a frozen format-stability statement. Closes the embedded-dependency gaps so an external system can adopt QZT. |
| Product Completeness: validation | Phase23 | Complete | Acceptance threshold harness over the C1-C6 corpora defined in docs/QZT_v0.1_Validation_Corpus.md. Makes "meets expectations" measurable via HARD invariants and SOFT target bands. |

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
| 2026-06-08 | 14-23 | `cargo test --test phase17_streaming_writer`; `cargo test --test phase18_competitive_benchmark`; `cargo test --features bench-compete --test phase18_competitive_benchmark`; `cargo test --all-targets --all-features --test phase20_public_api`; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` | Pass | Self-review pass 1 fixed streaming-writer bounded-memory checksum hashing. Self-review pass 2 added feature-gated ripgrep/SQLite FTS5 correctness hooks, `cargo-fuzz` open+verify target, and curated API default visibility. |
| 2026-06-08 | 14-23 | `make check`; `cargo package --offline --allow-dirty` | Pass | Full all-target/all-feature quality gate and offline packageability passed. Online `cargo package --allow-dirty` could not reach crates.io in this sandbox. |
| 2026-06-08 | design review follow-ups | `cargo fmt --all -- --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-targets --all-features` | Pass | Applied design-review recommendations (DR-1..DR-6). 151 tests pass (+12). |
| 2026-06-10 | quality review follow-ups | `make check`; `make bench-release`; 45 MB-corpus CLI measurements | Pass | Search hit verification now reuses a chunk decode cache (4,124-hit query: 16,376 ms -> 49 ms; new `physical_decoded_bytes` metric exposes chunk-level decode work), short/unindexable queries report `incomplete_reason` plus a CLI warning instead of silent empty results, `qzt export` streams with bounded memory (9.6 MB max RSS on a 45 MB corpus), the gate adds a default-features `cargo check --lib --bins`, and `bench-release` was repaired (it had been failing to compile since the Phase20 API curation) to run `--release --all-features`: pack 137.745 MiB/s, export 473.350 MiB/s, range 532.576 MiB/s on the 2.4 MB deterministic corpus — the 2026-06-07 row recorded debug-build values. 155 tests pass (+4). |
| 2026-06-10 | bounded-memory search wiring (DR-7) | `make check`; 42 MB / 400K-line corpus before/after CLI measurements (old binary built from the previous commit) | Pass | `qzt search`, `qzt info`, and `qzt sidecar-rebuild` now run on `QztFileReader`; new `QziFileSidecar` opens with manifest + term dictionary only (sections stream-verified) and fetches posting lists / candidate granule records lazily per query. Rare sidecar query: 518 MB -> 9.8 MB max RSS, 1.33 s -> 0.04 s. Dense 80K-hit sidecar query: 532 MB -> 36 MB, 1.11 s -> 0.17 s. `qzt info`: 9.6 MB -> 2.0 MB. Index builders stream chunk-by-chunk with binary-search chunk spans (removing the O(lines x chunks) scan); rebuilt sidecar bytes are identical to the previous builder. Transient (no-sidecar) search and sidecar-rebuild remain index-build-bound (~0.6-1.3 GB on this corpus) - tracked as the sidecar size/build follow-up. 160 tests pass (+5). |
| 2026-06-12 | post-Phase23 planning | `make check`; `git diff --check` | Pass | Post-Phase23 execution plan added (`tasks/PostPhase23.md`): wave ordering, parallelism rules, milestones M1-M6, and release gates for the GitHub roadmaps #31 (refactoring, issues #2-#30) and #47 (product value, issues #33-#46). 161 tests pass (+1). |
| 2026-06-13 | refactor phase 1 | `make check`; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` | Pass | Refactor Phase 1 (#3-#9 on top of merged #2): real bug #8 fixed (profile validation moved into `pack_bytes_internal` so every pack path validates; `"memory"` requires a DocumentIndex on all paths), `QztError` Display humanized with new `Io(ErrorKind)` / `UnsupportedIndexMode` variants replacing `NotImplemented` (#3), `usize_to_u64`/`u64_to_usize` helpers replace ~155 boilerplate conversion sites (#4), `Checksum::from_hasher`/`from_raw_bytes` remove inline constructions (#5), lib.rs module declarations collapsed into `internal_module!` macro (#6), dead `QztWriter`/`format::VERSION` removed (#7), `[lints]` clippy pedantic baseline established with documented allows (#9; public `pack_bytes_with_document_index`/`pack_bytes_with_memory_profile` now take `&DocumentIndex`, `run_release_benchmark_with_corpus` takes `&[u8]`). 169 tests pass (+8). |

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
| M-4 file-path/seeking reader | Fixed | Phase15 adds `ReadAt` and `QztFileReader`; CLI export/range/line/verify now use file-backed reads for core paths. |
| P1 chunker target-size soft-limit | Fixed | `choose_chunk_end` now uses `target_chunk_size` as the pack-all threshold instead of `max_chunk_size`, and clamps `max_end` to `input.len()`. Regression test added to phase4_chunker. |
| P1 required block validation | Fixed | `decode_block_descriptor` now rejects any `required=true` block whose type is not `chunk_table`. `is_known_block_type` removed. Regression tests for `token_index` required block and duplicate chunk_table added to phase8_reader_core. |
| P2 Metadata decode indexes/integrity | Fixed | `Metadata::decode` now validates fixed boolean values for all indexes fields and verifies all integrity algorithm fields equal `"blake3"`. |
| P0/P2 README limitations | Fixed | English README now contains a "v0.1 Technical Preview — Limitations" section covering in-memory reader, transient search, token co-occurrence semantics, normalized search, and benchmark gaps. |

## Design Review Follow-ups (2026-06-08)

Applied from the combined design + product review. No container format bytes change; all are read-path / verify-path / docs / test improvements behind the unchanged public API.

| Item | State | Notes |
|---|---|---|
| DR-1 README phase plan stale | Fixed | "Phase Plan" rewritten to cover Phase0-13 + Product Completeness 14-23; stale "QztFileReader planned" limitation corrected to the real remaining gap (search loads the whole container). |
| DR-2 `QztReader` read-path duplication | Fixed | `QztReader::read_range` / `read_line_raw` now delegate to the shared `read_range_from_entries` / `read_line_from_entries` free functions, removing ~75 lines of parallel logic that mirrored `QztFileReader`. |
| DR-3 deep-verify document re-decode + O(documents x chunks) | Fixed | Single-pass `DocumentHasher` hashes document ranges from the chunks already decoded by deep verify (no second decode pass). `document_chunk_range` now uses two binary searches instead of an O(chunks) scan per document. Handles arbitrary caller-supplied document order/overlap; empty documents verified without decoded bytes. Unit + integration coverage added. |
| DR-4 `read_document` O(documents) scan + unused `doc_id_hash` | Fixed | `SkeletonDetails` builds a `doc_id -> index` lookup once at open; `find_document` is now O(1) and keeps first-occurrence semantics on duplicate ids. |
| DR-5 `find_document` error conflation | Fixed | Added `QztError::DocumentNotFound`; "no document index" (`MissingRequiredBlock`) and "id not present" (`DocumentNotFound`) are now distinct. |
| DR-6 thin property coverage + dead param | Fixed | Added `tests/property_roundtrip.rs` (`export(pack(x)) == x`, `read_range == slice`). Removed the unused `StreamingTextAnalysis::new` parameter. |
| DR-7 search uses in-memory reader (P-2) | Fixed | 2026-06-10: `search_file` / `build_from_file` / `build_search_sidecar_from_file` / `QziFileSidecar` wire search and sidecar lookup to `QztFileReader` with lazy posting/granule fetch; the CLI search/info/sidecar-rebuild paths use them. Existing `&[u8]` / `&QztReader` entry points remain and delegate to the file-backed implementations. |

## Open Decisions

| Decision | Current Position | When To Revisit |
|---|---|---|
| Reference implementation language | Rust selected | Revisit only with explicit project decision |
| Search Extension timing | After Core conformance | After Phase9 |
| `qzt repack` / `qzt merge` / `qzt compact` | Deferred. They remain post-Core fresh-container rewrite commands and are not implemented in Phase10. | Post-Phase13 or a dedicated maintenance phase |
| Search index persistence | QZI sidecar persistence implemented. Embedded qzt-search-block-v1 remains optional future work. | Release hardening |
| Sidecar index priority | Implemented for token/ngram search sidecars | Larger-corpus benchmark pass |
| Competitive benchmark | Implemented for QZT vs whole-file raw zstd by default, with feature-gated ripgrep and SQLite FTS5 correctness hooks. Tantivy, Lucene, seekable zstd, and split zstd frames remain future comparison targets. | Post-v0.1 product validation |
