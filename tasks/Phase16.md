# Phase16: Streaming Verification and Export

## Purpose

Remove the O(file size) memory cost from deep verification and export. Today
`verify_deep` decodes every chunk and accumulates the entire original byte
stream in a `Vec<u8>` before recomputing the container checksum and line
information, which doubles peak memory for large files.

This phase makes deep verify and export streaming passes that update a BLAKE3
hasher and a line analyzer incrementally, carrying only the small cross-chunk
continuation state (the previous chunk tail relevant to newline and UTF-8
boundary checks).

This phase MUST NOT weaken any verification guarantee. Every byte that was
previously checked MUST still be checked; only the memory profile changes.

## Minimum MVP

```text
- verify_deep streams: decode chunk -> update container BLAKE3 hasher -> update line/newline analysis -> drop the decoded buffer
- cross-chunk line continuation is verified using only the retained previous-chunk tail
- verify_deep no longer accumulates the full original bytes in a Vec
```

## Goal MVP

```text
- export_to streams to the writer chunk-by-chunk with bounded buffers
- verify_deep on the file-backed reader uses bounded memory
- Document Index deep verification is range-scoped (or a separate deep-document mode) instead of materializing all documents
- a test asserts deep verify peak memory is bounded by max chunk size, not file size
```

## Spec refs

```text
- Section on verification levels (quick / normal / deep)
- Section 13 line semantics and continuation flags
- Section 28 Document Index verification
```

## Conformance Tests Covered

```text
- deep verify still detects compressed-chunk checksum mismatch
- deep verify still detects uncompressed-chunk checksum mismatch
- deep verify still detects container checksum mismatch
- deep verify still detects a stale Dense Line Index
- deep verify still detects a stale Document Index range
- streaming deep verify result equals the previous full-buffer result for every fixture
```

## TDD Plan

Write failing tests:

```text
- streaming verify_deep returns the same Ok/Err result as the previous implementation for every fixture
- verify_deep peak allocation is bounded by max chunk size plus index, measured with an allocation probe
- a corrupted final chunk is still detected when continuation state is carried, not buffered
- export_to streams without allocating the full original size
- Document Index deep verify rejects a stale range without materializing all documents
```

## Implementation Tasks

```text
1. replace the full-output Vec in verify_deep with an incremental BLAKE3 hasher
2. replace full-buffer line analysis with an incremental line/newline analyzer carrying only the previous-chunk tail
3. verify the STARTS_WITH_LINE_CONTINUATION flag using the retained tail instead of the whole buffer
4. make export_to stream chunk-by-chunk with a bounded reusable buffer
5. scope Document Index deep verify to per-document ranges
6. add an allocation-probe test to bound deep verify peak memory
7. confirm equivalence against the previous behavior on all fixtures
```

## Rust Notes

Use `blake3::Hasher::update` incrementally rather than hashing a final buffer.
Keep a single reusable decode buffer sized to the maximum uncompressed chunk
size to avoid per-chunk reallocation. The continuation analyzer needs only the
last byte (and, for UTF-8/CRLF safety, the minimal trailing bytes) of the
previous chunk; do not retain more. Streaming must not change error variants
returned for corruption cases.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec
and this phase plan before continuing.

## Self-Review Checklist

```text
- Does streaming deep verify check exactly the same bytes as the buffered version?
- Are all corruption cases still detected with identical error variants?
- Is retained cross-chunk state minimal (tail only)?
- Is peak memory bounded by chunk size, not file size?
- Does export stream without full-original allocation?
- Did this phase leave quick and normal verify behavior unchanged?
```

## Done Criteria

```text
- streaming verify_deep equivalence tests pass on all fixtures
- deep verify peak-memory bound test passes
- export streaming test passes
- Document Index range-scoped deep verify test passes
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Complete.

Completed on: 2026-06-08

Implementation scope:

```text
- Reworked deep verify to stream chunk decode and accumulate BLAKE3/text analysis incrementally.
- Added export_to chunk streaming and range-scoped Document Index verification for file-backed readers.
```

Verification:

```text
- cargo test --test phase16_streaming_verify
- make check
```

Review notes:

```text
- Self-review pass 1 completed: verify_deep no longer materializes the full original byte vector.
- Self-review pass 2 completed: stale Document Index checks use range-scoped reads instead of whole-file export.
- Code review completed: deep verification still checks original checksum, line count, newline mode, and indexed document ranges.
- Architecture review completed: streaming verification composes with Phase15 ReadAt and keeps Core format bytes unchanged.
```

Depends on: Phase15 (the file-backed reader supplies the ReadAt path that
makes bounded-memory deep verify meaningful for large files). The hasher and
analyzer refactor can begin on the in-memory reader and then apply to both.
