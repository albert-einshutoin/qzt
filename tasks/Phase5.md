# Phase5: Reader Open, Info, Export, and Verification Levels

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
- deep verify decompresses every chunk
- line_count and newline_mode are recomputed in deep verify
- corruption tests map to specific errors
```

## Spec refs

```text
- Section 9.1 Reader open procedure
- Section 20.1 Export all
- Section 21 Verification
- Section 25 Reader API
- Section 35.1 Core conformance tests 54-55, 66-69, 74-75
```

## Conformance Tests Covered

```text
- open/info/export for valid no-dictionary containers
- quick verify without decompression
- normal verify compressed checksum detection
- deep verify uncompressed checksum, line_count, newline_mode, and zstd output-size detection
```

## TDD Plan

Write failing tests:

```text
- open valid container
- export equals original bytes
- quick verify succeeds without decompression
- normal verify detects compressed checksum mismatch
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

## Self-Review Checklist

```text
- Does quick verify avoid decompression?
- Are normal and deep verify clearly separated?
- Are resource limits applied before decompression?
- Do corruption tests assert specific errors?
```

## Done Criteria

```text
- reader API works for Phase4 fixtures
- quick/normal/deep tests pass
- no-dictionary Core read/export path is stable
- status.md is updated
```

## Status

Pending.
