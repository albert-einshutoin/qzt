# Phase4: UTF-8 Chunker and Sparse Chunk Table Writer

## Purpose

Implement the deterministic chunk planning layer before writing compressed frames.

This phase reduces writer risk by proving UTF-8-safe chunk boundaries, CRLF preservation, sparse line metadata, and continuation flags independently from zstd and final file assembly.

## Minimum MVP

```text
- UTF-8 input validation
- line-preferred chunk planner
- CRLF boundary protection
- Chunk Plan entries include logical_offset, uncompressed_size, first_line, line_count, and flags
```

## Goal MVP

```text
- max_chunk_size is enforced as a hard limit
- long lines split only at valid UTF-8 boundaries
- starts_with_line_continuation is computed correctly
- sum(line_count) equals container line_count
- first_line continuity is valid for every adjacent planned chunk
```

## Spec refs

```text
- Section 12.1 Chunk boundary
- Section 13 Line semantics
- Section 14 Chunks
- Section 16 Chunk Table
- Section 34.2 Writer Core
- Section 35.1 Core conformance tests 1-16, 46-52, 70-73
```

## Conformance Tests Covered

```text
- UTF-8 writer rejection
- CRLF-safe chunking
- UTF-8-safe chunking
- no zero-length planned chunks
- planned Chunk Table chunk_id, logical offset, first_line, line_count, and size invariants
- starts_with_line_continuation writer behavior
```

## TDD Plan

Write failing tests:

```text
- invalid UTF-8 is rejected before chunk planning
- empty input produces zero planned chunks and line_count 0
- ASCII input plans contiguous logical offsets
- Japanese and emoji boundaries are never split inside code points
- CRLF input is not split between CR and LF
- long line exceeding max_chunk_size is split safely
- Chunk Plan sum(line_count) matches container line_count
- adjacent first_line continuity is valid
- continuation chunks set starts_with_line_continuation
```

## Implementation Tasks

```text
1. implement writer options validation needed by chunk planning
2. implement UTF-8 validation
3. implement newline_mode and line_count calculation
4. implement line-preferred chunk planner
5. compute planned chunk first_line and line_count while chunking
6. compute starts_with_line_continuation flags
7. expose Chunk Plan as internal writer input for Phase5
```

## Rust Notes

Keep the chunk planner independent from file I/O and zstd. It should be easy to property-test with byte slices.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- Does chunk planning avoid all invalid UTF-8 boundaries?
- Does CRLF preservation hold independently of target chunk size?
- Does sparse line metadata match decoded original bytes?
- Are continuation flags derivable from original byte boundaries?
- Can Phase5 consume the Chunk Plan without recomputing line semantics?
```

## Done Criteria

```text
- chunk planner tests pass
- UTF-8 and CRLF boundary tests pass
- sparse line metadata tests pass
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Complete.

Completed on 2026-06-07.

Verification:

```text
make check
```

Review notes:

```text
- self-review completed
- code review completed; UTF-8 and CRLF split rules are tested before writer I/O exists
- architecture review completed; chunk planning is independent from zstd and exposes ChunkPlan for Phase5
```
