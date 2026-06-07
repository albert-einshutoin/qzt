# Phase1: Deterministic CBOR, Primitives, and Errors

## Purpose

Implement the low-level safety layer used by every parser, writer, and verifier.

## Minimum MVP

```text
- QztError enum exists
- primitive little-endian helpers exist
- checked u64 offset arithmetic exists
- deterministic CBOR encoder/decoder rejects obvious invalid encodings
```

## Goal MVP

```text
- Core CBOR closed-schema behavior is enforced
- duplicate keys are rejected
- non-canonical integer/string/map encodings are rejected
- property tests cover checked arithmetic and primitive round-trips
```

## Spec refs

```text
- Section 7 Byte order and primitive types
- Section 7.1 Deterministic CBOR profile
- Section 23 Error codes
- Section 33 Security and resource limits
- Section 35.1 Core conformance tests 31-34, 58
```

## Conformance Tests Covered

```text
- non-canonical CBOR rejection
- duplicate CBOR key rejection
- checked offset arithmetic overflow rejection
- primitive byte-order round-trips for later fixed structures
```

## TDD Plan

Write failing tests first:

```text
- decode u16/u32/u64 little-endian fixtures
- offset + size overflow returns LogicalRangeOutOfBounds or PhysicalRangeOutOfBounds
- duplicate CBOR map key returns DuplicateCborKey
- non-shortest integer encoding returns NonCanonicalCbor
- unknown closed-schema field is rejected
```

## Implementation Tasks

```text
1. define QztError variants from the spec
2. implement fixed primitive encode/decode helpers
3. implement checked range helpers
4. wrap a CBOR library or implement constrained deterministic checks
5. add schema validation helpers for required/optional fields
```

## Rust Notes

Use `TryFrom<&[u8]>` for fixed records where practical. Keep parsing fallible and allocation-bounded.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- No parser path panics on short or corrupt input
- Every overflow uses checked arithmetic
- CBOR validation happens before trusting decoded fields
- Error variants are specific enough for conformance tests
```

## Done Criteria

```text
- primitive tests pass
- CBOR rejection tests pass
- property tests pass for range arithmetic
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
- code review completed; CBOR negative integer encoding was changed to avoid overflow/panic
- architecture review completed; primitive, range, error, and CBOR modules are isolated for later fixed-structure parsing
```
