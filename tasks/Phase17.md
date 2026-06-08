# Phase17: Streaming Writer

## Purpose

Implement the real `QztFileWriter<W: Write + Seek>` so producers can build
containers larger than RAM. Today the writer is a one-shot `pack_bytes` over a
fully buffered input, and `QztWriter` is a `#[doc(hidden)]` placeholder.

The streaming writer accepts input incrementally, splits it at chunk
boundaries with the existing chunker, compresses and writes each frame as it
goes, accumulates only the in-memory Chunk Table (128 bytes per chunk), and on
`finish()` writes the metadata, optional blocks, Chunk Table, Index Root,
Footer Payload, and Trailer, then seeks back to patch the Header.

This phase MUST produce byte-identical output to `pack_bytes` for the same
input and options, so it cannot change the format or break
`export(pack(input)) == input`.

## Minimum MVP

```text
- QztFileWriter::new(writer, options)
- push(&[u8]) buffers across calls, splits at chunk boundaries via the existing chunker, compresses, writes each frame, records a ChunkEntry
- finish() writes metadata, Chunk Table, Index Root, Footer Payload, Trailer, and patches the Header via seek-back
```

## Goal MVP

```text
- streaming writer output is byte-identical to pack_bytes for the same input and options (differential golden test)
- writer peak memory is bounded by max chunk size plus the Chunk Table, not by input size
- CLI pack streams a file or stdin through the writer without reading the whole input
- finish() is single-shot: after an error or after finish, the writer is poisoned and cannot emit a container claimed valid
```

## Spec refs

```text
- Section 22 Immutability (finish seals the container)
- Section on Writer API and finish semantics
- Section on the Footer Payload fixed-point final_file_size convergence
```

## Conformance Tests Covered

```text
- streaming writer output equals pack_bytes output byte-for-byte across fixtures and options
- streaming writer round-trips: export(stream_pack(input)) == input
- partial input across many push() calls produces the same container as one push()
- finish() cannot be called twice; a poisoned writer never yields a valid-looking container
- writer enforces UTF-8 validity at chunk boundaries exactly as the one-shot writer does
```

## TDD Plan

Write failing tests:

```text
- stream_pack(input) byte-equals pack_bytes(input) for empty, single-line, multi-line, CRLF, mixed, and UTF-8 fixtures
- pushing input in 1-byte, prime-sized, and chunk-sized increments yields identical containers
- export of a streamed container equals the original input
- writer peak allocation is bounded by max chunk size plus Chunk Table, measured with an allocation probe
- invalid UTF-8 pushed mid-stream is rejected with the same error variant as the one-shot writer
- calling finish() twice returns a specific error and does not emit a second container
```

## Implementation Tasks

```text
1. define QztFileWriter<W: Write + Seek> and a WriteAt/seek-back helper for the Header patch
2. carry a boundary buffer across push() calls and reuse the chunker boundary logic
3. compress each finalized chunk, write the frame, and append a ChunkEntry with both BLAKE3 checksums
4. on finish(), write metadata and optional blocks, then the Chunk Table, Index Root, Footer Payload, and Trailer
5. reuse the existing fixed-point footer convergence for final_file_size
6. seek back and patch the Header metadata offset/size
7. poison the writer after finish() or after any fatal error
8. add a differential golden test against pack_bytes
9. stream stdin/file through the writer in the pack CLI
```

## Rust Notes

The Chunk Table grows in memory at 128 bytes per chunk; document this bound and
note that spilling the Chunk Table for extreme chunk counts is explicitly
deferred. Reuse the existing chunker and the existing fixed-point footer
routine rather than reimplementing either. The Header patch needs `Seek`; keep
the `Write + Seek` bound and fail clearly if the sink is not seekable. Treat
`finish()` as the immutability boundary from Section 22 — once sealed, no
further writes.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec
and this phase plan before continuing.

## Self-Review Checklist

```text
- Is streaming output byte-identical to pack_bytes for every fixture and option set?
- Is peak memory bounded by chunk size plus Chunk Table, independent of input size?
- Does push-fragmentation never change the resulting container?
- Is finish() a hard immutability boundary that cannot run twice?
- Are UTF-8 and CRLF boundary rules identical to the one-shot writer?
- Did this phase avoid any format byte change?
```

## Done Criteria

```text
- QztFileWriter push/finish implemented
- byte-identical differential golden tests pass
- fragmentation-invariance tests pass
- peak-memory bound test passes
- double-finish poisoning test passes
- pack CLI streams without full-input buffering
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.

Depends on: Phase15 (shared WriteAt/ReadAt direction and decode core for the
round-trip differential tests). Completes the large-data I/O model started by
the file-backed reader. Replaces the H-5 placeholder writer.
