# QZT Task Status

Last updated: 2026-06-07

## Current Rule

Implementation must proceed with TDD and the loop:

```text
implement -> self-review -> fix -> verify -> update status
```

## Phase Summary

| Phase | Name | State | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 0 | Project foundation and quality gates | Pending | Rust workspace, CI-local commands, empty test harness | Repeatable quality gates and fixture layout |
| 1 | Deterministic CBOR, primitives, and errors | Pending | Canonical CBOR rejection tests and fixed primitive helpers | Typed errors, checked arithmetic, property tests |
| 2 | Header, footer trailer, and physical ranges | Pending | Encode/decode fixed structures | Range validator and corruption tests |
| 3 | Metadata, footer payload, index root, and chunk table skeleton | Pending | Deterministic CBOR schemas and empty-container skeleton | Structural verifier without zstd chunks |
| 4 | No-dictionary writer and exact export fixtures | Pending | pack/export equality for simple UTF-8 | Chunking, zstd frames, BLAKE3, long-line fixtures |
| 5 | Reader open/info/export and verification levels | Pending | open/info/export on valid files | quick/normal/deep verify corruption coverage |
| 6 | Sparse line index, range reads, and CLI access | Pending | read_range and read_line_raw | CLI range/line, spanning-line support |
| 7 | Dictionaries, resource limits, and Reader Core completion | Pending | read embedded dictionary fixtures | Reader Core complete with resource hardening |
| 8 | Core conformance hardening and release readiness | Pending | Full Core test pass | v0.1 Core release candidate |
| 9 | Dense Line Index, Document Index, and memory profile | Pending | Dense Line Index acceleration | Document Index and memory profile fixtures |
| 10 | Search granules and token index MVP | Pending | Token index over search granules | Verified token search with metrics |
| 11 | N-gram index, planner, and benchmark reporting | Pending | N-gram candidate search | Rarest-first planner and performance reports |
| 12 | Search sidecar and high-performance search goal MVP | Pending | `.qzi` sidecar validation | Memory-mappable high-performance search flow |

## Current Focus

No implementation phase has started.

Next action:

```text
Start Phase0 by creating the Rust workspace, test harness, and local quality commands.
```

## Completion Tracks

| Track | Required Phases | Current State | Notes |
|---|---|---|---|
| Writer Core | Phase0-Phase6, Phase8 | Pending | Writer may omit dictionary output, but must write valid sparse line fields. |
| Reader Core | Phase0-Phase8 | Pending | Phase7 completes embedded dictionary reading and resource hardening. |
| Optional Core-defined indexes | Phase9 | Pending | Dense Line Index and Document Index are not Core release blockers. |
| Search Extension | Phase10-Phase12 | Pending | Must start after Core conformance is stable. |

## Verification Log

| Date | Phase | Commands | Result | Notes |
|---|---:|---|---|---|
| 2026-06-07 | planning | `git diff --check`, code fence count | Pass | Planning docs created after spec review |

## Open Decisions

| Decision | Current Position | When To Revisit |
|---|---|---|
| Reference implementation language | Rust | Before Phase0 code starts |
| Search Extension timing | After Core conformance | After Phase8 |
| Sidecar index priority | Phase12 | After Phase11 benchmark evidence |
