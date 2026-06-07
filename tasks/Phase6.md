# Phase6: Reader Open, Info, Export, and Verification Levels

## Purpose

Make no-dictionary QZT containers readable and verifiable through the public reader API.

## Minimum MVP

```text
- QztReader::open
- info
- export_to
- quick verify
```

## Goal MVP

```text
- normal verify checks all compressed chunk checksums
- normal verify checks container_checksum when present
- deep verify decompresses every chunk
- line_count and newline_mode are recomputed in deep verify
- starts_with_line_continuation flags are verified in deep verify
- corruption tests map to specific errors
```

## Spec refs

```text
- Section 9.1 Reader open procedure
- Section 20.1 Export all
- Section 21 Verification
- Section 25 Reader API
- Section 35.1 Core conformance tests 53-55, 66-69, 74-75, 77
```

## Conformance Tests Covered

```text
- open/info/export for valid no-dictionary containers
- quick verify without decompression
- normal verify container_checksum detection when present
- normal verify compressed checksum detection
- deep verify starts_with_line_continuation mismatch detection
- deep verify uncompressed checksum, line_count, newline_mode, and zstd output-size detection
```

## TDD Plan

Write failing tests:

```text
- open valid container
- export equals original bytes
- quick verify succeeds without decompression
- normal verify detects container_checksum mismatch when present
- normal verify detects compressed checksum mismatch
- deep verify detects starts_with_line_continuation mismatch
- deep verify detects uncompressed checksum mismatch
- deep verify detects line_count mismatch
- deep verify detects newline_mode mismatch
```

## Implementation Tasks

```text
1. implement read-at abstraction for files
2. implement open procedure
3. load and validate Chunk Table
4. implement info summary
5. implement export_to
6. implement VerifyLevel enum
7. implement quick, normal, and deep verify reports
```

## Rust Notes

Separate open-time validation from deep verification. The quick path must not accidentally decompress all chunks.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- Does quick verify avoid decompression?
- Are normal and deep verify clearly separated?
- Are resource limits applied before decompression?
- Does continuation-flag verification avoid repeated adjacent-chunk decompression?
- Do corruption tests assert specific errors?
```

## Done Criteria

```text
- reader API works for Phase5 fixtures
- normal verify validates container_checksum when present without full decompression
- quick/normal/deep tests pass
- no-dictionary Core read/export path is stable
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
- code review completed; quick, normal, and deep verify paths are separated by decompression cost
- architecture review completed; Reader owns read/export/verify while writer delegates export compatibility to Reader
```
