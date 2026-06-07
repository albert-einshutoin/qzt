# Phase5: No-Dictionary Zstd Writer and Finish

## Purpose

Turn the Phase4 Chunk Plan into real no-dictionary QZT files with independent zstd frames, checksums, indexes, footer payload, footer trailer, and patched header.

## Minimum MVP

```text
- pack planned chunks into independent zstd frames
- no dictionary output
- no Dense Line Index
- write valid Header, Metadata, Chunk Table, Index Root, Footer Payload, and Footer Trailer
- export(pack(input)) == input for simple fixtures
```

## Goal MVP

```text
- compressed and uncompressed BLAKE3 chunk checksums are written
- one complete zstd frame is written per chunk
- Header is patched at finish
- generated containers include valid sparse line index fields from Phase4
- long-line and mixed-newline fixtures produce valid containers
- pack throughput smoke metric is recorded
```

## Spec refs

```text
- Section 8 Fixed Header
- Section 10 Footer Payload
- Section 11 Metadata Block
- Section 14.1 Zstd frame requirements
- Section 16 Chunk Table
- Section 18 Index Root
- Section 24.1 pack
- Section 34.2 Writer Core
- Section 35.1 Core conformance tests 1-17, 45-52, 70-73
- generated fixtures for Section 35.1 Core conformance tests 53-55, 66, 74-75
```

## Conformance Tests Covered

```text
- no-dictionary pack/export fixtures
- independent zstd frame output
- compressed and uncompressed checksum generation
- Header patch and final file structure
- generated container fixtures for Phase6 reader/verify tests
```

## TDD Plan

Write failing tests:

```text
- empty file pack/export equality
- ASCII file pack/export equality
- Japanese and emoji pack/export equality
- CRLF and mixed-newline pack/export equality
- long-line pack/export equality
- each non-empty chunk has one complete zstd frame
- compressed and uncompressed checksums match exact bytes
- Header metadata pointers are patched after finish
- pack smoke benchmark records bytes/sec
```

## Implementation Tasks

```text
1. consume Phase4 Chunk Plan
2. implement zstd single-frame encoder wrapper
3. calculate compressed and uncompressed chunk checksums
4. write compressed chunks
5. write Metadata
6. write fixed Chunk Table records
7. write Index Root
8. write Footer Payload and Footer Trailer
9. patch Header at finish
10. record initial pack throughput smoke metric
```

## Rust Notes

Prefer streaming-friendly writer internals, but do not prematurely optimize. Exact byte restoration and clear invariants matter first.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- Does each chunk contain exactly one complete zstd frame?
- Does every non-empty chunk have non-zero compressed and uncompressed sizes?
- Are checksums over exact bytes?
- Are Phase4 line metadata fields preserved without recomputation drift?
- Are generated containers suitable as Phase6 fixtures?
```

## Done Criteria

```text
- no-dictionary pack/export fixtures pass
- writer rejects invalid inputs
- generated containers include valid sparse line index fields
- pack throughput smoke metric is recorded
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.
