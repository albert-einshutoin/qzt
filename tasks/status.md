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
| 10 | Dense Line Index, Document Index, memory profile, and maintenance command scoping | Pending | Dense Line Index acceleration with sparse-vs-dense benchmark | Document Index, memory profile fixtures, repack/merge decision |
| 11 | Search granules and raw token index MVP | Pending | Raw token index over search granules | Verified token search with metrics |
| 12 | N-gram index, planner, and benchmark reporting | Pending | Raw n-gram candidate search | Rarest-first planner and performance reports |
| 13 | Search sidecar and high-performance search goal MVP | Pending | `.qzi` sidecar validation | Memory-mappable high-performance search flow |

## Current Focus

Phase0 through Phase9 are complete. QZT v0.1 Core is release-candidate ready.

Next action:

```text
Start Phase10 by implementing Dense Line Index acceleration and sparse-vs-dense benchmark evidence.
```

## Completion Tracks

| Track | Required Phases | Current State | Notes |
|---|---|---|---|
| Writer Core | Phase0-Phase5, Phase9 | Complete | v0.1 Writer Core is no-dictionary output; dictionary-emitting writer remains out of Core-ready scope. |
| Reader Core | Phase0-Phase9 | Complete | Embedded dictionary reading, resource limits, partial access, and verify levels are complete for v0.1 Core. |
| Optional Core-defined indexes | Phase10 | Pending | Dense Line Index and Document Index are not Core release blockers. |
| Search Extension | Phase11-Phase13 | Pending | Must start after Core conformance is stable. |

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

## Open Decisions

| Decision | Current Position | When To Revisit |
|---|---|---|
| Reference implementation language | Rust selected | Revisit only with explicit project decision |
| Search Extension timing | After Core conformance | After Phase9 |
| `qzt repack` / `qzt merge` / `qzt compact` | Post-Core maintenance commands, not v0.1 Core blockers | Phase10 |
| Sidecar index priority | Phase13 | After Phase12 benchmark evidence |
