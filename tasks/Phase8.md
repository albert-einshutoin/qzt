# Phase8: Core Conformance Hardening and Release Readiness

## Purpose

Turn the implementation into a stable v0.1 Core release candidate.

This phase is the Core release gate. Optional indexes, Document Index, memory profile, and Search Extension work MUST NOT block this phase unless the release target is explicitly expanded.

## Minimum MVP

```text
- all Core conformance tests pass locally
- CLI supports pack, info, export, range, line, verify
- public errors are stable enough for tests
```

## Goal MVP

```text
- v0.1 Core release candidate
- fixture corpus documented
- performance baseline for pack/export/range/line
- no Search Extension code required
- no Document Index or memory profile required
```

## Spec refs

```text
- Section 1.3 Core conformance and profiles
- Section 34.1 Reader Core
- Section 34.2 Writer Core
- Section 35.1 Core conformance tests
- Section 36 Reference implementation roadmap
```

## Conformance Tests Covered

```text
- all Core conformance tests 1-75
- CLI integration coverage for pack, info, export, range, line, verify
- fixture coverage for valid and corrupt Core containers
```

## TDD Plan

Write or finish failing tests for every Core conformance item before release cleanup.

Required fixture groups:

```text
- empty
- ASCII
- LF, CRLF, mixed newline
- Japanese
- emoji
- invalid UTF-8
- long single line
- tiny chunk size
- corrupted fixed structures
- corrupted CBOR blocks
- corrupted chunks
- dictionary reader fixtures
```

## Implementation Tasks

```text
1. map spec conformance tests to test names
2. fill fixture gaps
3. add CLI integration tests
4. add benchmark smoke tests for pack/export/range/line
5. polish public API docs
6. run self-review against spec sections 1-35
7. produce Core release readiness notes
```

## Rust Notes

Prefer stable public APIs and strict internal types. Do not expose raw parsed structs unless callers need them.

## Self-Review Checklist

```text
- Does every MUST in Core have a test or a documented reason?
- Are extensions kept out of Core release criteria?
- Are errors documented and stable?
- Can a user recover original bytes from all valid fixtures?
- Can all corrupt fixtures fail without panics?
```

## Done Criteria

```text
- all Core conformance tests pass
- CLI integration tests pass
- Core benchmark smoke results are recorded
- release notes draft exists if packaging begins
- status.md marks Core ready
```

## Status

Pending.
