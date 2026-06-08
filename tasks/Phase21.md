# Phase21: Verified Evidence Retrieval and Memory Pager Integration

## Purpose

Deliver the product's defining operation — verified evidence retrieval by
pointer — and prove the Memory Pager integration story end to end. Spec
Section 3 defines an `evidence_ref` carrying `container_id`, `doc_id`,
`byte_range`, `line_range`, and `checksum`; Memory Pager restores those ranges
and verifies them against the stored checksum.

Today range reads exist, but there is no single "restore and verify against an
expected checksum" operation. File-backed `doc_id` resolution is introduced in
this phase with the public evidence API rather than in Phase15, and there is no
example or integration test proving the Section 3 workflow. This phase closes
that gap so QZT is demonstrably usable as the Cold Evidence Container for an AI
memory system.

This phase MUST NOT change the container format bytes. Verified retrieval is a
read-side capability over existing structures.

## Minimum MVP

```text
- read_range_verified: restore a byte range and verify it against an expected BLAKE3 checksum, returning a specific error on mismatch
- read_document_verified: resolve a doc_id range and verify it against an expected BLAKE3 checksum
- an examples/ program demonstrating the Section 3 evidence_ref workflow against a real container
```

## Goal MVP

```text
- an end-to-end integration test: build a container, emit an evidence_ref-like pointer, reopen by path plus container_id, then restore and verify both the byte range and the document
- the evidence-retrieval API and the Memory Pager integration pattern are documented in the README or a usage guide
- concurrent verified reads from one open container are exercised by a test (building on the Phase15 file-backed reader path)
- a tampered container byte makes the corresponding verified read fail with a specific error, never returning wrong bytes silently
```

## Spec refs

```text
- Section 2 product boundary
- Section 3 relationship to Memory Pager and the evidence_ref shape
- Section 12 range reads
- Section 13 line semantics
- Section 28 Document Index
```

## Conformance Tests Covered

```text
- verified retrieval returns bytes when the checksum matches
- verified retrieval rejects a tampered range with a specific error
- doc_id resolution matches the Document Index and the in-memory reader
- the evidence_ref end-to-end workflow restores and verifies the expected bytes
- concurrent verified reads return the same results as serial reads
```

## TDD Plan

Write failing tests:

```text
- read_range_verified returns the bytes when the expected checksum matches and a specific error when it does not
- read_document_verified resolves doc_id via the Document Index and verifies the per-document checksum
- file-backed doc_id verified read equals the in-memory reader
- the evidence_ref E2E test passes: pack, build pointer, reopen by path plus container_id, verified restore of range and document
- N parallel verified range reads from one reader equal N serial reads
- a corrupted container byte makes the matching verified read fail rather than silently returning wrong bytes
```

## Implementation Tasks

```text
1. add the base read_document(doc_id) on the reader (resolve doc_id to a byte range via the Document Index, then read_range); none exists today (only private verify helpers)
2. add read_range_verified(range, expected_checksum) over the shared decode/verify core
3. add read_document_verified(doc_id, expected_checksum) layering verification on read_document
4. ensure file-backed doc_id resolution and the in-memory path share the same Document Index lookup
5. write an examples/ program mirroring the Section 3 evidence_ref JSON workflow
6. add an end-to-end integration test reopening by path plus container_id
7. add a concurrency test issuing parallel verified reads from one reader
8. document the evidence-retrieval API and the Memory Pager integration pattern
```

## Rust Notes

Verified reads compute BLAKE3 over the restored bytes and compare against the
expected value, reusing the shared decode/verify core so checksum logic is
never duplicated. The Document Index is a navigation structure over original
bytes, never the source of truth; deep verify must still recompute it.
Concurrency relies on the Phase15 positioned-read design so `&self` reads stay
sound without shared mutable seek state. Keep the example small and runnable
with `cargo run --example`, and have it consume only the Phase20 public API.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec
and this phase plan before continuing.

## Self-Review Checklist

```text
- Does verified retrieval fail closed (specific error) on any checksum mismatch?
- Does doc_id resolution use the Document Index as navigation, not as authority?
- Does the example prove the real Section 3 evidence_ref workflow end to end?
- Are concurrent verified reads sound and equal to serial reads?
- Does a single tampered byte reliably surface as a verified-read error?
- Did this phase avoid any container format byte change?
```

## Done Criteria

```text
- read_range_verified and read_document_verified implemented and tested
- file-backed doc_id verified read matches the in-memory reader
- examples/ evidence_ref workflow program exists and runs
- end-to-end integration test passes
- concurrency test passes
- evidence-retrieval API and Memory Pager integration pattern are documented
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.

Depends on: Phase15 (file-backed reader), Phase20 (stable public surface for
the example), and Phase10 Document Index (complete). This phase delivers the
product's defining operation and proves the headline Memory Pager use case.
