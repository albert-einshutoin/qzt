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
| 0 | Project foundation and quality gates | Pending | Rust workspace, CI-local commands, empty test harness | Repeatable quality gates and fixture layout |
| 1 | Deterministic CBOR, primitives, and errors | Pending | Canonical CBOR rejection tests and fixed primitive helpers | Typed errors, checked arithmetic, property tests |
| 2 | Header, footer trailer, and physical ranges | Pending | Encode/decode fixed structures | Range validator and corruption tests |
| 3 | Metadata, footer payload, index root, and chunk table skeleton | Pending | Deterministic CBOR schemas and empty-container skeleton | Structural verifier without zstd chunks |
| 4 | UTF-8 chunker and sparse Chunk Table writer | Pending | deterministic Chunk Plan for UTF-8 input | CRLF-safe, continuation-aware line metadata |
| 5 | No-dictionary zstd writer and finish | Pending | pack/export equality for simple UTF-8 | zstd frames, BLAKE3, footer finish, pack smoke metric |
| 6 | Reader open/info/export and verification levels | Pending | open/info/export on valid files | quick/normal/deep verify corruption coverage |
| 7 | Sparse line index, range reads, and CLI access | Pending | read_range and read_line_raw | CLI range/line, spanning-line support, intermediate benchmarks |
| 8 | Dictionaries, resource limits, and Reader Core completion | Pending | read embedded dictionary fixtures | Reader Core complete with resource hardening |
| 9 | Core conformance hardening and release readiness | Pending | Full Core test pass and fuzz smoke | v0.1 Core release candidate |
| 10 | Dense Line Index, Document Index, memory profile, and maintenance command scoping | Pending | Dense Line Index acceleration with sparse-vs-dense benchmark | Document Index, memory profile fixtures, repack/merge decision |
| 11 | Search granules and raw token index MVP | Pending | Raw token index over search granules | Verified token search with metrics |
| 12 | N-gram index, planner, and benchmark reporting | Pending | Raw n-gram candidate search | Rarest-first planner and performance reports |
| 13 | Search sidecar and high-performance search goal MVP | Pending | `.qzi` sidecar validation | Memory-mappable high-performance search flow |

## Current Focus

No implementation phase has started.

Next action:

```text
Start Phase0 by creating the Rust workspace, test harness, and local quality commands.
```

## Completion Tracks

| Track | Required Phases | Current State | Notes |
|---|---|---|---|
| Writer Core | Phase0-Phase5, Phase9 | Pending | Writer may omit dictionary output, but must write valid sparse line fields and pass Core release gates. |
| Reader Core | Phase0-Phase9 | Pending | Phase8 completes embedded dictionary reading and resource hardening; Phase9 is the release gate. |
| Optional Core-defined indexes | Phase10 | Pending | Dense Line Index and Document Index are not Core release blockers. |
| Search Extension | Phase11-Phase13 | Pending | Must start after Core conformance is stable. |

## Verification Log

| Date | Phase | Commands | Result | Notes |
|---|---:|---|---|---|
| 2026-06-07 | planning | `git diff --check`, code fence count | Pass | Planning docs created after spec review |
| 2026-06-07 | planning | `git diff --check`, phase reference search, code fence count | Pass | External review follow-ups applied to spec and phase plan |

## Open Decisions

| Decision | Current Position | When To Revisit |
|---|---|---|
| Reference implementation language | Rust | Before Phase0 code starts |
| Search Extension timing | After Core conformance | After Phase9 |
| `qzt repack` / `qzt merge` / `qzt compact` | Post-Core maintenance commands, not v0.1 Core blockers | Phase10 |
| Sidecar index priority | Phase13 | After Phase12 benchmark evidence |
