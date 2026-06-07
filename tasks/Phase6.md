# Phase6: Sparse Line Index, Range Reads, and CLI Access

## Purpose

Implement partial access, which is the main product value of QZT Core.

## Minimum MVP

```text
- read_range(offset, length)
- read_text_range(offset, length)
- read_line_raw(line_zero_based)
- sparse line lookup through Chunk Table
```

## Goal MVP

```text
- line-spanning chunk reads work
- starts_with_line_continuation flag is honored
- CLI supports range and line commands
- zero-length reads are tested
```

## Spec refs

```text
- Section 12.3 Text range API
- Section 13 Line semantics
- Section 17 Line Index
- Section 20.2 Read byte range
- Section 20.3 Read text range
- Section 20.4 Read line
- Section 24 CLI specification
- Section 35.1 Core conformance tests 56-63
```

## Conformance Tests Covered

```text
- read_range within and across chunks
- read_range overflow and zero-length behavior
- read_text_range UTF-8 boundary rejection
- read_line first, last, out-of-range, and spanning-line behavior
- CLI line numbering and range semantics
```

## TDD Plan

Write failing tests:

```text
- read_range within one chunk
- read_range spanning multiple chunks
- read_range length 0 returns empty bytes
- read_range offset + length overflow is rejected
- read_text_range invalid UTF-8 boundary is rejected
- read_line first and last lines
- read_line for long line spanning chunks
- CLI line default is 1-based
```

## Implementation Tasks

```text
1. implement chunk lookup by logical byte range
2. implement partial chunk decode
3. implement text boundary validation
4. implement sparse line binary search
5. implement continuation-aware line scan
6. implement CLI range
7. implement CLI line
```

## Rust Notes

Avoid decoding more chunks than needed. Keep decoded buffers bounded by configured chunk limits.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- Are byte ranges half-open internally?
- Is CLI line numbering converted exactly once?
- Are CRLF and EOF line endings handled correctly?
- Do spanning-line reads avoid duplicate or missing bytes?
```

## Done Criteria

```text
- range and line API tests pass
- CLI smoke tests pass
- long-line fixtures pass
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.
