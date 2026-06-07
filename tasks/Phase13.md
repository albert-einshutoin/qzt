# Phase13: Search Sidecar and High-Performance Search Goal MVP

## Purpose

Move large search structures into a rebuildable sidecar and validate that search can be fast without weakening QZT Core evidence guarantees.

## Minimum MVP

```text
- `.qzi` sidecar header and manifest
- source_container_id and checksum matching
- sidecar rejection leaves Core read/export/verify working
```

## Goal MVP

```text
- memory-mappable term dictionary and posting sections
- sidecar rebuild command
- token and n-gram sidecar lookup
- high-performance search claim backed by metrics
```

## Spec refs

```text
- Section 30 Sidecar indexes
- Section 29 Search Index
- Section 35.2 Extension conformance tests 5-6, 20
- Section 36.1 Cut 5d
```

## Conformance Tests Covered

```text
- sidecar source_container_id mismatch rejection
- sidecar source_original_checksum mismatch rejection
- sidecar absence does not break Core read/export/verify
- sidecar lookup matches embedded search index behavior
- high-performance search metrics are reported before claims
```

## TDD Plan

Write failing tests:

```text
- wrong source_container_id sidecar is rejected
- wrong source_original_checksum sidecar is rejected
- missing sidecar does not break Core operations
- sidecar term lookup matches embedded index lookup
- common-term query is capped or requires explicit fallback mode
- rare-term query decodes only candidate-overlapping chunks
```

## Implementation Tasks

```text
1. implement sidecar manifest model
2. implement sidecar source matching
3. write search sidecar builder
4. read sidecar with memory-map-friendly layout
5. add sidecar rebuild CLI
6. compare embedded vs sidecar query metrics
7. document performance envelope
```

## Rust Notes

Use memory mapping only behind a safe abstraction. Validate checksums and bounds before exposing slices to lookup code.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- Is every sidecar byte treated as derived and untrusted?
- Can sidecar rejection never hide valid Core data?
- Are memory-mapped offsets bounds-checked?
- Are performance claims tied to benchmark output?
```

## Done Criteria

```text
- sidecar correctness tests pass
- sidecar benchmark reports exist
- high-performance search goal MVP is demonstrable
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.
