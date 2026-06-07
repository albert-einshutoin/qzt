# Phase12: N-Gram Index, Planner, and Benchmark Reporting

## Purpose

Add substring/Japanese-friendly search and the planner features needed to reduce decompression cost.

## Minimum MVP

```text
- n-gram builder
- n-gram candidate search
- exact phrase/substring verification
- raw_utf8 index source only unless normalized mapping metadata is implemented first
```

## Goal MVP

```text
- rarest-first posting intersection
- skip data for long postings
- high document-frequency term handling
- benchmark reporting suitable for performance claims
```

## Spec refs

```text
- Section 29.6 N-gram Index
- Section 29.8 Boundary matches
- Section 29.9 Query planner
- Section 29.10 High document frequency terms
- Section 29.11 Search performance reporting
- Section 35.2 Extension conformance tests 4, 13-20
```

## Conformance Tests Covered

```text
- n-gram unit and normalization declaration
- boundary completeness through line-granule adjacent_decode verification
- rarest-first planner behavior
- high document-frequency handling
- complete vs incomplete missing-key behavior
- benchmark report completeness
- normalized_utf8 indexes remain deferred unless mapping metadata is tested
```

## TDD Plan

Write failing tests:

```text
- n-gram unit and normalization are declared
- raw_utf8 source is the default and normalized_utf8 is feature-gated or rejected
- line granule adjacent_decode preserves declared complete matches across chunk boundaries
- byte_window indexing is deferred to Phase13 sidecar/high-performance work
- missing key in complete=true returns no matches without chunk decode
- missing key in complete=false reports fallback/incomplete state
- rarest required posting list is selected first
- high-DF term does not drive first intersection
- skip data avoids full high-frequency posting decode
```

## Implementation Tasks

```text
1. implement n-gram extraction
2. build n-gram term dictionary and postings
3. implement query parser for required keys
4. implement rarest-first planner
5. implement skip data
6. implement candidate and decoded-byte caps
7. keep normalized_utf8 support out unless mapping metadata tests are implemented first
8. generate benchmark reports
```

## Rust Notes

Represent planner decisions explicitly so tests can inspect why a query used or skipped a posting list.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- Does the planner minimize decoded bytes rather than just candidate count?
- Are complete and incomplete index semantics enforced?
- Is raw-vs-normalized source handling explicit and test-covered?
- Are high-DF limits deterministic and configurable?
- Are benchmark metrics reproducible?
```

## Done Criteria

```text
- n-gram fixtures pass
- planner tests pass
- benchmark reports include required metrics
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Complete.

Completed on: 2026-06-07

Implementation scope:

```text
- Phase12 implements a transient raw_utf8 Unicode-scalar n-gram index over line Search Granules.
- Boundary coverage is declared as adjacent_decode: candidate line granules may span chunks, and exact substring verification reads original byte ranges through QztReader.
- The planner sorts required postings rarest-first, avoids high document-frequency keys as the first driver when alternatives exist, emits incomplete state for complete=false missing keys, and reports candidate/decode metrics.
- Skip metadata is generated for posting lists with at least 1024 entries and is reflected in reported posting_bytes_read.
- byte_window granularity, persisted qzt-search-block-v1, memory-mapped sidecar layout, and true skip-assisted binary posting intersection are deferred to Phase13.
```

Verification:

```text
- cargo test --test phase12_ngram_planner -- --nocapture
- cargo test --test phase11_search -- --nocapture
- make check
```

Review notes:

```text
- Self-review completed: n-gram hits are exact byte substring matches from QztReader output, not raw posting hits.
- Code review completed: normalized_utf8 is rejected, missing-key complete/incomplete behavior is explicit, high-DF keys are not used as the first planner driver when rarer keys exist, and skip metadata is generated deterministically.
- Architecture review completed: Phase12 extends the transient search layer without changing Core container semantics or claiming persisted high-performance search.
- Performance claim check completed: benchmark metrics are emitted, but Phase12 remains an in-memory correctness/planner MVP until Phase13 sidecar persistence and memory-mapped lookup exist.
```
