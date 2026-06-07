# Phase10: Dense Line Index, Document Index, Memory Profile, and Maintenance Command Scoping

## Purpose

Add optional Core-defined acceleration and document-range structures after Core release readiness is stable. Also decide the post-Core maintenance command scope for immutable-container rewrites.

This phase MUST NOT change Core source-of-truth semantics. Dense Line Index and Document Index are acceleration or navigation structures over original bytes.

## Minimum MVP

```text
- Dense Line Index writer
- Dense Line Index reader fast path
- deep verify detects Dense Line Index disagreement
- sparse-vs-dense line lookup benchmark establishes when Dense Line Index is worth enabling
- `qzt repack`, `qzt merge`, and `qzt compact` are explicitly scoped as implement-now, defer, or reject
```

## Goal MVP

```text
- Document Index schema and verification
- memory profile defaults
- memory profile Dense Line Index defaults are backed by benchmark evidence
- document range reads can be verified against original bytes
- any implemented maintenance command rewrites into a fresh valid container, never mutates in place
```

## Spec refs

```text
- Section 17.1 Dense Line Index extension
- Section 22 Immutability
- Section 27.5 memory profile
- Section 28 Document Index
- Section 35.1 Core conformance tests 64-65, if Dense Line Index is present
- Section 35.2 Extension conformance tests 1-2
```

## Conformance Tests Covered

```text
- Dense Line Index final line without newline
- Dense Line Index line_start_offsets count mismatch
- Dense Line Index disagreement detected by deep verify
- sparse-vs-dense lookup threshold documented for representative line counts and chunk sizes
- Document Index ranges within original_size
- Document Index chunk_start/chunk_end consistency
- maintenance commands are non-blocking for v0.1 Core conformance
```

## TDD Plan

Write failing tests:

```text
- Dense Line Index final line without newline
- Dense Line Index line_start_offsets count mismatch
- Dense Line Index disagreement detected by deep verify
- sparse Chunk Table lookup is faster or equivalent below the documented threshold
- Dense Line Index lookup beats sparse lookup above the documented threshold before enabling it by default
- Document Index range outside original_size is rejected
- Document Index chunk_start/chunk_end inconsistency is rejected
- memory profile writes expected metadata flags
- maintenance command scope decision is recorded
```

## Implementation Tasks

```text
1. implement dense line index block codec
2. use dense index as optional fast path
3. keep sparse scan as correctness fallback
4. benchmark sparse vs dense lookup across representative line counts and chunk sizes
5. record the threshold for enabling Dense Line Index in memory profile defaults
6. implement Document Index schema
7. verify document ranges in deep verify
8. implement memory profile defaults
9. decide whether `qzt repack`, `qzt merge`, and `qzt compact` are implemented in this phase or deferred
```

## Rust Notes

Dense Line Index must be a cache, not authority. Keep verification code able to recompute from decoded bytes.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- Does disabling Dense Line Index preserve behavior?
- Are document byte and line ranges half-open internally?
- Does deep verify catch stale optional indexes?
- Is memory profile still Core-compatible when extensions are optional?
- Does this phase leave Phase9 Core release behavior unchanged?
- Is Dense Line Index enabled only where benchmark evidence supports it?
- Are maintenance commands clearly non-blocking for v0.1 Core conformance?
```

## Done Criteria

```text
- Dense Line Index tests pass
- sparse-vs-dense benchmark results and threshold decision are recorded
- Document Index tests pass
- memory profile fixture passes deep verify
- repack/merge/compact scope decision is recorded in status.md
- Core conformance tests still pass
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Complete.

Completed on: 2026-06-07

Verification:

```text
- cargo test --test phase10_dense_line_index -- --nocapture
- cargo test --test phase10_document_index
- cargo test --test phase10_dense_line_index sparse_vs_dense_line_lookup_benchmark_records_threshold_evidence -- --nocapture
- make check
```

Benchmark smoke:

```text
phase10_dense_bench lines=2048 sparse_us=3370.167 dense_us=2365.709 threshold_decision=enable_dense_for_memory_profile_at_or_above_2048_lines
```

Maintenance command scope:

```text
- qzt repack: defer. It is a fresh-container rewrite command and is not required for v0.1 Core or Phase10 optional index correctness.
- qzt merge: defer. It needs document/range policy decisions beyond Dense Line Index and Document Index validation.
- qzt compact: defer. It should be implemented with repack once rewrite policy is defined.
```

Review notes:

```text
- Self-review completed: Dense Line Index can be disabled without changing Core behavior, and sparse scan remains the source-of-truth fallback.
- Code review completed: Dense Line Index count mismatch is rejected at open, stale offsets are rejected by deep verify, Document Index byte/checksum/chunk ranges are verified by deep verify.
- Architecture review completed: Dense Line Index and Document Index are optional blocks, memory profile remains Core-compatible, and Phase9 Core release behavior is unchanged.
- Spec updated: qzt-line-delta-varint-v1 physical payload and Document Index CBOR payload/chunk range semantics are now explicit.
- Dense Line Index default decision: memory profile writes Dense Line Index; benchmark evidence currently supports enabling it for representative inputs at or above 2048 lines.
```
