# Phase3: Metadata, Footer Payload, Index Root, and Chunk Table Skeleton

## Purpose

Create the Core container skeleton without compressed text payload complexity.

## Minimum MVP

```text
- Metadata schema encode/decode
- Footer Payload schema encode/decode
- Index Root schema encode/decode
- empty Chunk Table block support
```

## Goal MVP

```text
- empty QZT container can be written and opened structurally
- Header/Footer/Metadata/Index Root consistency checks work
- Chunk Table block invariants are verified
```

## Spec refs

```text
- Section 10 Footer Payload
- Section 11 Metadata Block
- Section 16 Chunk Table
- Section 18 Index Root
- Section 21.1 quick verify
- Section 35.1 Core conformance tests 1, 26-45, 50-51, 70-73
```

## Conformance Tests Covered

```text
- empty container structure
- Footer Payload checksum mismatch
- Metadata and Index Root consistency mismatches
- Chunk Table block size, chunk_count, first_line, line_count, and uncompressed_size invariants
```

## TDD Plan

Write failing tests:

```text
- empty metadata validates
- Footer Payload checksum mismatch is rejected
- Header/Footer container_id mismatch is rejected
- Metadata/Index Root original_size mismatch is rejected
- Chunk Table size != chunk_count * 128 is rejected
- chunk_count mismatch is rejected
```

## Implementation Tasks

```text
1. implement Metadata model
2. implement Footer Payload model
3. implement Index Root model
4. implement Chunk Entry fixed record model
5. implement empty container skeleton writer
6. implement skeleton open procedure through Chunk Table checksum
```

## Rust Notes

Keep logical models separate from serialized models. Use conversions so invalid serialized state cannot leak into trusted runtime structs.

## Self-Review Checklist

```text
- Are duplicate fields rejected before model creation?
- Are source.original_size, original_checksum, and line_count compared consistently?
- Can empty input be represented without fake chunks?
- Are checksums calculated over exact serialized bytes?
```

## Done Criteria

```text
- empty container opens
- quick verify passes on empty container
- structural corruption tests pass
- status.md is updated
```

## Status

Pending.
