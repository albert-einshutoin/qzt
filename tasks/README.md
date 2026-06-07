# QZT Implementation Tasks

This directory is the execution plan for the QZT reference implementation.

The reference implementation SHOULD be written in Rust unless a project decision changes this file first. Rust fits QZT because the format needs precise binary layout, checked arithmetic, explicit errors, streaming I/O, bounded decompression, and strong testable invariants.

## Operating Rules

Every phase uses TDD:

```text
1. write or update failing tests
2. implement the smallest behavior that passes
3. run targeted tests
4. run broader verification for touched areas
5. self-review the diff
6. perform a code review
7. perform an architecture review
8. fix review findings
9. update tasks/status.md
```

Do not mark a phase complete until tests, self-review, code review, architecture review, review fixes, and status updates are done.

## Implementation Flow

Use this loop for every meaningful change:

```text
implement -> self-review -> code review -> architecture review -> fix -> verify -> update status
```

Self-review MUST check:

```text
- Does the code implement the spec invariant directly?
- Are overflow and resource limits handled before trusting file data?
- Are errors specific enough for conformance tests?
- Are tests proving both success and corruption cases?
- Did the change preserve exact export semantics?
- Did the change avoid adding extension behavior into Core?
```



## Review Gates

Every phase MUST include both review gates before completion.

Code review checks:

```text
- parser and writer code has no hidden panics on untrusted input
- errors are specific and testable
- tests cover success, corruption, and boundary cases
- public APIs remain small and coherent
- implementation follows Rust ownership and type-safety idioms
```

Architecture review checks:

```text
- module boundaries still match the spec sections
- Core behavior is not coupled to optional extensions
- data flow preserves exact export and source-of-truth semantics
- resource limits and checked arithmetic are enforced at trust boundaries
- future phases can build on the current design without rewriting completed phases
```

If a review finds a spec ambiguity or library constraint, update both `docs/QZT_v0.1_Core_Spec.md` and the relevant `tasks/PhaseN.md` before continuing.

## Rust Style Expectations

Use language features that make the binary format safer:

```text
- newtypes for offsets, sizes, chunk IDs, line IDs, and granule IDs where useful
- Result<T, QztError> for fallible operations
- checked_add / checked_mul for all offset and size arithmetic
- TryFrom for parsed fixed binary structures
- traits for ReadAt / WriteAt behavior where it keeps tests simple
- borrowed slices for fixed-layout parsing when safe
- explicit Vec allocation limits before decompression or CBOR decode
- property tests for round-trip and checked arithmetic
- golden fixtures for conformance files
```

Avoid hidden panics in parser, verifier, and reader paths. A corrupt file is an expected input, not an exceptional programming state.

## Phase File Contract

Each `PhaseN.md` contains:

```text
- Purpose
- Minimum MVP
- Goal MVP
- TDD plan
- Implementation tasks
- Self-review checklist
- Done criteria
- Status
```

Minimum MVP is the smallest useful increment that should land first.
Goal MVP is the phase's intended stopping point before the next phase starts.

## Status Tracking

`tasks/status.md` is the single progress summary.

When work starts or finishes:

```text
- update the phase state
- record the current commit if relevant
- record tests run
- record open blockers
- keep the next action concrete
```

## Phase Order

```text
Phase0  Project foundation and quality gates
Phase1  Deterministic CBOR, primitives, and errors
Phase2  Header, footer trailer, and physical ranges
Phase3  Metadata, footer payload, index root, and chunk table skeleton
Phase4  No-dictionary writer and exact export fixtures
Phase5  Reader open/info/export and verification levels
Phase6  Sparse line index, range reads, and CLI access
Phase7  Dictionaries, resource limits, and Reader Core completion
Phase8  Core conformance hardening and release readiness
Phase9  Dense Line Index, Document Index, and memory profile
Phase10 Search granules and token index MVP
Phase11 N-gram index, planner, and benchmark reporting
Phase12 Search sidecar and high-performance search goal MVP
```

Do not start Search Extension implementation before Core conformance is stable, except for design-only work.

Optional indexes and extension profiles MUST NOT block Core release readiness unless a phase explicitly says the release target includes them.
