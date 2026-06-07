# Phase11: N-Gram Index, Planner, and Benchmark Reporting

## Purpose

Add substring/Japanese-friendly search and the planner features needed to reduce decompression cost.

## Minimum MVP

```text
- n-gram builder
- n-gram candidate search
- exact phrase/substring verification
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
- boundary overlap completeness
- rarest-first planner behavior
- high document-frequency handling
- complete vs incomplete missing-key behavior
- benchmark report completeness
```

## TDD Plan

Write failing tests:

```text
- n-gram unit and normalization are declared
- byte_window overlap preserves declared complete matches
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
7. generate benchmark reports
```

## Rust Notes

Represent planner decisions explicitly so tests can inspect why a query used or skipped a posting list.

## Self-Review Checklist

```text
- Does the planner minimize decoded bytes rather than just candidate count?
- Are complete and incomplete index semantics enforced?
- Are high-DF limits deterministic and configurable?
- Are benchmark metrics reproducible?
```

## Done Criteria

```text
- n-gram fixtures pass
- planner tests pass
- benchmark reports include required metrics
- status.md is updated
```

## Status

Pending.
