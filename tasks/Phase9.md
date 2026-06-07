# Phase9: Dense Line Index, Document Index, and Memory Profile

## Purpose

Add optional Core-defined acceleration and document-range structures after Core release readiness is stable.

This phase MUST NOT change Core source-of-truth semantics. Dense Line Index and Document Index are acceleration or navigation structures over original bytes.

## Minimum MVP

```text
- Dense Line Index writer
- Dense Line Index reader fast path
- deep verify detects Dense Line Index disagreement
```

## Goal MVP

```text
- Document Index schema and verification
- memory profile defaults
- document range reads can be verified against original bytes
```

## Spec refs

```text
- Section 17.1 Dense Line Index extension
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
- Document Index ranges within original_size
- Document Index chunk_start/chunk_end consistency
```

## TDD Plan

Write failing tests:

```text
- Dense Line Index final line without newline
- Dense Line Index line_start_offsets count mismatch
- Dense Line Index disagreement detected by deep verify
- Document Index range outside original_size is rejected
- Document Index chunk_start/chunk_end inconsistency is rejected
- memory profile writes expected metadata flags
```

## Implementation Tasks

```text
1. implement dense line index block codec
2. use dense index as optional fast path
3. keep sparse scan as correctness fallback
4. implement Document Index schema
5. verify document ranges in deep verify
6. implement memory profile defaults
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
- Does this phase leave Phase8 Core release behavior unchanged?
```

## Done Criteria

```text
- Dense Line Index tests pass
- Document Index tests pass
- memory profile fixture passes deep verify
- Core conformance tests still pass
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.
