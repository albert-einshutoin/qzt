# Phase8: Dictionaries, Resource Limits, and Reader Core Completion

## Purpose

Complete Reader Core obligations and harden the parser against expensive or malicious files.

## Minimum MVP

```text
- parse embedded Dictionary Block
- validate dictionary checksums
- decode dictionary-compressed fixture
```

## Goal MVP

```text
- Reader Core conformance complete
- resource limits enforced before allocation/decompression
- unknown optional blocks ignored
- unknown required blocks rejected
```

## Spec refs

```text
- Section 15 Dictionary handling
- Section 18 Index Root unknown block behavior
- Section 33 Security and resource limits
- Section 34.1 Reader Core
- Section 35.1 Core conformance tests 18-21, 42-44, 75
```

## Conformance Tests Covered

```text
- embedded dictionary fixture can be read
- missing, duplicate, and checksum-mismatched dictionaries are rejected
- unknown optional blocks are ignored
- unknown required blocks are rejected
- resource limits are enforced before unsafe allocation or decompression
```

## TDD Plan

Write failing tests:

```text
- dictionary-compressed fixture exports exactly
- missing dictionary is rejected
- duplicate dictionary_id is rejected
- dictionary checksum mismatch is rejected
- unknown optional block is ignored
- unknown required block returns UnknownRequiredBlock
- decompression exceeding declared size is rejected
```

## Implementation Tasks

```text
1. implement Dictionary Block schema
2. connect dictionary_id lookup to zstd decode
3. add resource limit configuration
4. enforce dictionary, index, chunk, and preview size limits
5. add unknown block handling
6. add Reader Core conformance checklist
```

## Rust Notes

Dictionary bytes are untrusted input. Validate size and checksum before passing them to zstd.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- Can a missing dictionary never trigger fallback to external state?
- Are size limits checked before allocation?
- Does Reader Core support dictionary files even if Writer Core does not emit them?
- Are all unknown required blocks fatal?
```

## Done Criteria

```text
- Reader Core conformance tests pass
- dictionary fixtures pass
- resource-limit corruption tests pass
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.
