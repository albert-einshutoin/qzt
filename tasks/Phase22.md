# Phase22: Portable Conformance Vectors and Format Stability

## Purpose

Make QZT a real, independently verifiable format, not just one Rust crate. Cold
evidence implies long-term readability: a third party, or a future reader in
another language, must be able to validate a `.qzt` file years later. Today the
conformance map lives inside the Rust test suite and is not packaged as
portable golden vectors, and there is no frozen format-stability statement.

This phase commits portable golden vectors, a vector runner, a third-party
verification procedure, and a frozen v0.1 format-stability statement. It MUST
NOT change the container format bytes; it freezes and documents them.

## Minimum MVP

```text
- golden .qzt vectors covering valid containers and representative corruption cases, committed under tests/vectors/
- a manifest describing each vector's expected open/verify/export result
- a vector-runner test that validates the reference implementation against the manifest
```

## Goal MVP

```text
- the vector set covers the Core conformance map (open/verify/export/range/line) and the corruption taxonomy
- a documented procedure lets a third-party or other-language reader run the vectors
- a frozen QZT v0.1 format-stability statement defines which bytes/structures are stable, the forward/backward-compatibility policy, and how format_version is negotiated
- vectors regenerate deterministically and the regeneration command is documented
```

## Spec refs

```text
- Section 1.3 Core conformance and profiles
- Section 22 Immutability
- Section 34 conformance levels
- Section 35 test suite and conformance tests
- format_version handling across the spec
```

## Conformance Tests Covered

```text
- the reference implementation passes every committed golden vector
- a corruption vector is rejected with the manifest's specified error variant
- vectors regenerate byte-identically from a deterministic command
- a format_version newer than supported is handled per the stability statement
```

## TDD Plan

Write failing tests:

```text
- the vector runner asserts each golden vector's open/verify/export/range/line result equals the manifest
- a corruption vector is rejected with the manifest's specified error variant
- vector regeneration is deterministic: regenerating yields byte-identical files
- a container declaring an unsupported newer format_version is rejected or negotiated per the stability statement
```

## Implementation Tasks

```text
1. build a deterministic vector generator producing valid and corrupt .qzt files
2. write a manifest of expected results for each vector
3. add a vector-runner test validating the implementation against the manifest
4. cover the Core conformance map and the corruption taxonomy with vectors
5. document the third-party / other-language verification procedure
6. write the v0.1 format-stability and version-negotiation statement
7. document the deterministic regeneration command
```

## Rust Notes

Keep vectors tiny and deterministic so they live in git without bloat. The
generator reuses the writer; the runner uses only the Phase20 public reader
API, so it doubles as a public-API smoke test. The stability statement must
commit to not changing v0.1 container bytes: any change to the byte layout is a
new `format_version`, never an in-place edit of v0.1.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec
and this phase plan before continuing.

## Self-Review Checklist

```text
- Do the vectors cover the Core conformance map and the corruption taxonomy?
- Can a third party validate a .qzt without reading the Rust source?
- Does the runner use only the public reader API?
- Is the v0.1 format-stability policy explicit and frozen?
- Do vectors regenerate byte-identically?
- Did this phase avoid any container format byte change?
```

## Done Criteria

```text
- golden vectors and a manifest are committed under tests/vectors/
- the vector-runner test passes against the manifest
- corruption vectors are rejected with the specified errors
- the third-party verification procedure is documented
- the v0.1 format-stability and version-negotiation statement is published
- deterministic regeneration is documented and verified
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.

Depends on: Phase20 (the runner uses the stable public reader API), Phase23a
(the vector set reuses the shared validation corpora), and the Phase9 Core
conformance map (complete). Establishes the format as independently verifiable
and frozen for long-term evidence readability.
