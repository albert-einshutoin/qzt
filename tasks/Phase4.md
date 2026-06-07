# Phase4: No-Dictionary Writer and Exact Export Fixtures

## Purpose

Write real QZT files with independent zstd chunks and prove exact restoration for no-dictionary containers.

## Minimum MVP

```text
- pack UTF-8 input into independent zstd frames
- no dictionary output
- no Dense Line Index
- Chunk Table includes first_line and line_count
- starts_with_line_continuation is written correctly
- export(pack(input)) == input for simple fixtures
```

## Goal MVP

```text
- line-preferred chunking works
- CRLF boundaries are preserved
- UTF-8 boundaries are preserved
- compressed and uncompressed BLAKE3 chunk checksums are written
- sum(line_count) equals Metadata and Index Root line_count
- first_line continuity is valid for every adjacent chunk
- long-line fixtures pass
```

## Spec refs

```text
- Section 12.1 Chunk boundary
- Section 13 Line semantics
- Section 14 Chunks
- Section 16 Chunk Table
- Section 34.2 Writer Core
- Section 35.1 Core conformance tests 1-17, 46-53, 66, 70-75
```

## Conformance Tests Covered

```text
- UTF-8 writer rejection
- CRLF-safe chunking
- UTF-8-safe chunking
- no zero-length chunks
- Chunk Table chunk_id, logical offset, first_line, line_count, and size invariants
- starts_with_line_continuation writer behavior
- export equality fixtures for no-dictionary containers
```

## TDD Plan

Write failing tests:

```text
- empty file pack/export equality
- ASCII file pack/export equality
- Japanese and emoji pack/export equality
- CRLF file is not split between CR and LF
- invalid UTF-8 is rejected by writer
- long line exceeding max_chunk_size is split safely
- Chunk Table sum(line_count) matches container line_count
- adjacent first_line continuity is valid
- continuation chunks set starts_with_line_continuation
```

## Implementation Tasks

```text
1. implement writer options validation
2. implement UTF-8 validation
3. implement line-preferred chunker
4. compute chunk first_line and line_count while chunking
5. compute starts_with_line_continuation flags
6. implement zstd single-frame encoder wrapper
7. calculate chunk checksums
8. write chunks, Metadata, Chunk Table, Index Root, Footer Payload, Footer Trailer
9. patch Header at finish
```

## Rust Notes

Prefer streaming-friendly writer internals, but do not prematurely optimize. Exact byte restoration and clear invariants matter first.

## Self-Review Checklist

```text
- Does each chunk contain exactly one complete zstd frame?
- Does every chunk have non-zero compressed and uncompressed sizes?
- Is max_chunk_size enforced as a hard writer limit?
- Does sparse line metadata match decoded original bytes?
- Are continuation flags derivable from original byte boundaries?
- Are checksums over exact bytes?
```

## Done Criteria

```text
- no-dictionary pack/export fixtures pass
- writer rejects invalid inputs
- generated containers include valid sparse line index fields
- generated containers become Phase5 reader/verify fixtures
- status.md is updated
```

## Status

Pending.
