# Phase9: Core Conformance Hardening and Release Readiness

## Purpose

Turn the implementation into a stable v0.1 Core release candidate.

This phase is the Core release gate. Optional indexes, Document Index, memory profile, and Search Extension work MUST NOT block this phase unless the release target is explicitly expanded.

## Minimum MVP

```text
- all Core conformance tests pass locally
- fuzz harness smoke passes for open and verify
- CLI supports pack, info, export, range, line, verify
- public errors are stable enough for tests
```

## Goal MVP

```text
- v0.1 Core release candidate
- fixture corpus documented
- performance baseline for pack/export/range/line
- malformed binary parser fuzz corpus exists
- no Search Extension code required
- no Document Index or memory profile required
```

## Spec refs

```text
- Section 1.3 Core conformance and profiles
- Section 34.1 Reader Core
- Section 34.2 Writer Core
- Section 35.1 Core conformance tests
- Section 33 Security and resource limits
- Section 36 Reference implementation roadmap
```

## Conformance Tests Covered

```text
- all Core conformance tests 1-77
- CLI integration coverage for pack, info, export, range, line, verify
- fixture coverage for valid and corrupt Core containers
- fuzz smoke coverage for open and verify on malformed files
```

## TDD Plan

Write or finish failing tests for every Core conformance item before release cleanup.

Required fixture groups:

```text
- empty
- ASCII
- LF, CRLF, mixed newline
- Japanese
- emoji
- invalid UTF-8
- long single line
- tiny chunk size
- corrupted fixed structures
- corrupted CBOR blocks
- corrupted chunks
- dictionary reader fixtures
- fuzz seeds derived from conformance fixtures
```

## Implementation Tasks

```text
1. map spec conformance tests to test names
2. fill fixture gaps
3. add CLI integration tests
4. add benchmark smoke tests for pack/export/range/line
5. add cargo-fuzz or equivalent harnesses for open and verify
6. run fuzz smoke with recorded command and duration
7. polish public API docs
8. run self-review against spec sections 1-35
9. produce Core release readiness notes
```

## Rust Notes

Prefer stable public APIs and strict internal types. Do not expose raw parsed structs unless callers need them.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- Does every MUST in Core have a test or a documented reason?
- Are extensions kept out of Core release criteria?
- Are errors documented and stable?
- Can a user recover original bytes from all valid fixtures?
- Can all corrupt fixtures fail without panics?
- Do fuzz targets cover fixed structures, CBOR blocks, Chunk Table records, and compressed chunk boundaries?
```

## Done Criteria

```text
- all Core conformance tests pass
- CLI integration tests pass
- Core benchmark smoke results are recorded
- open and verify fuzz smoke results are recorded
- code review findings are fixed
- architecture review findings are fixed
- release notes draft exists if packaging begins
- status.md marks Core ready
```

## Status

Complete.

Completed on: 2026-06-07

Verification:

```text
- cargo test --test phase9_cli_core
- cargo test --test phase9_hardening
- cargo test --test phase5_writer pack_smoke_benchmark_records_nonzero_throughput -- --nocapture
- cargo test --test phase7_access phase7_intermediate_benchmark_records_nonzero_metrics -- --nocapture
- make check
```

Benchmark smoke:

```text
phase5_pack_smoke bytes=65536 elapsed_ms=6.284 throughput_mib_s=9.945
phase7_bench pack_mib_s=10.909 export_mib_s=36.692 range_mib_s=17.517 line_us=5.250
```

Review notes:

```text
- Self-review completed: Core CLI commands pack/info/export/range/line/verify are present and line ranges use documented 1-based inclusive CLI semantics.
- Code review completed: pack option parsing rejects invalid profile, checksum, dictionary, dense-line-index, and chunk-size settings.
- Architecture review completed: Phase9 adds no Search Extension, Dense Line Index, Document Index, sidecar, repack, merge, or compact code to the Core release gate.
- Fuzz smoke completed through deterministic malformed open/verify seeds in tests/phase9_hardening.rs.
- Core conformance mapping lives in tests/phase9_hardening.rs and Core readiness notes live in docs/QZT_v0.1_Core_Readiness.md.
- No spec ambiguity was found that required QZT_v0.1_Core_Spec.md changes.
```
