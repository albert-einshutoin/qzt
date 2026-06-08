# Phase15: File-Backed Seeking Reader

## Purpose

Align the implementation with the product's core value proposition: access
large containers without loading them fully into memory. Today `QztReader`
holds the entire container in a `Vec<u8>`, which contradicts the
"handle large text without full expansion" claim.

This phase introduces a `ReadAt` abstraction and a
`QztFileReader<R: Read + Seek>` that opens by reading only the fixed trailer,
footer payload, header, metadata, index root, and chunk table — never the
chunk-data region — and serves range and line reads by seeking to and
decoding only the chunks that overlap the request.

This phase MUST NOT change container format bytes or verification semantics.
The file reader and the in-memory reader MUST produce identical results for
every fixture.

## Minimum MVP

```text
- ReadAt trait: read_exact_at(offset, buf) with a slice impl and a File impl
- QztFileReader::open reads only the bounded prefix/suffix and index region, not the chunk-data region
- read_range seeks to the start chunk and decodes only overlapping chunks
- in-memory QztReader is preserved; shared decode/verify core is extracted so logic is not duplicated
```

## Goal MVP

```text
- read_line_raw via the file reader uses the Chunk Table and optional Dense Line Index, seeking only needed chunks
- export_to streams chunk-by-chunk from the file with bounded buffers
- peak resident memory for range/line reads is bounded by max chunk size plus the index region, not by file size
- CLI range / line / export use the file reader for file-path inputs
```

## Spec refs

```text
- Section 9.1 reader open procedure
- Section 12 range reads and UTF-8 boundary handling
- Section 13 line semantics
- tasks/README.md Rust Style: traits for ReadAt / WriteAt behavior
```

## Conformance Tests Covered

```text
- file reader open reads a bounded byte count and never touches the chunk-data region
- file reader and in-memory reader agree on info/export/range/line for all fixtures
- file reader range read decodes only chunks overlapping the requested range
- corrupt/out-of-bounds physical offsets are rejected without panic on the seeking path
- resource limits are enforced on the file path before any decode
```

## TDD Plan

Write failing tests:

```text
- a counting ReadAt records that open() reads only the trailer, footer, header, metadata, index root, and chunk table bytes
- open() does not read any byte inside the chunk-data region
- read_range on the file reader equals read_range on the in-memory reader for every fixture (differential test)
- read_range that spans N chunks reads exactly those N compressed frames, no more
- read_line_raw on the file reader equals the in-memory reader, including spanning lines
- export_all on the file reader equals the original input for every fixture
- a request with a corrupt chunk physical_offset returns a specific error, never panics
- ResourceLimits chunk-size and index-size caps are enforced on the file path
```

## Implementation Tasks

```text
1. define a ReadAt trait and impls for &[u8] and std::fs::File (positioned read; seek+read fallback)
2. extract the shared decode-and-verify chunk core so both readers call it
3. implement QztFileReader::open reading only the bounded prefix/suffix and the index region
4. implement file-backed read_range using Chunk Table binary search plus per-chunk seek/decode
5. implement file-backed read_line_raw using the Chunk Table and optional Dense Line Index fast path
6. implement file-backed export_to streaming chunk-by-chunk
7. add a differential test harness that runs every fixture through both readers
8. wire CLI file-path inputs to the file reader for range/line/export
9. document the peak-memory bound per operation
```

## Rust Notes

Keep `ReadAt` minimal and object-safe where it helps testing. Prefer positioned
reads where they are available, but do not make Send + Sync concurrency part of
this phase. Phase21 owns the concurrent verified-read guarantee after the
public evidence API exists. This phase should establish the single-reader
seeking path first.

The decode-and-verify core must be shared between readers so checksum and
boundary logic is never duplicated or allowed to drift. Use checked arithmetic
for every file offset derived from container data.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec
and this phase plan before continuing.

## Self-Review Checklist

```text
- Does the file reader produce byte-identical results to the in-memory reader for all fixtures?
- Does open() provably avoid reading the chunk-data region?
- Is peak memory bounded by chunk size plus index, not file size?
- Are all file offsets derived from container data validated with checked arithmetic?
- Is the decode/verify core shared rather than duplicated?
- Did this phase leave the in-memory reader public API unchanged?
```

## Done Criteria

```text
- ReadAt trait and File/slice impls exist
- QztFileReader open/info/export/range/line implemented
- differential tests pass across all fixtures
- open-reads-bounded-prefix test passes
- peak-memory bound test passes
- CLI uses the file reader for file-path inputs
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.

Depends on: Phase14 (CI exists to run the larger differential test matrix).
This is the most important product-completeness phase: it closes the gap
between the product claim and the implementation. Marked as "M-4 file-path
seeking reader" in the status follow-up table.
