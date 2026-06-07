# Phase11: Search Granules and Raw Token Index MVP

## Purpose

Build the first correct Search Extension path without claiming high-performance search prematurely.

The minimum MVP is raw UTF-8 only. Normalized indexes are deferred until their mapping metadata can prove every hit against original bytes.

## Minimum MVP

```text
- Search Granule Table for one granularity
- raw_utf8 token dictionary builder
- sorted posting lists
- exact verification against original bytes
- no normalized token index in the minimum MVP
```

## Goal MVP

```text
- raw token search works over line or byte_window granularity
- search reports candidate_granules, candidate_chunks, decoded_bytes, and query_time_ms
- false positives are verified away
```

## Spec refs

```text
- Section 29.1 Candidate search rule
- Section 29.2 Search Granules
- Section 29.3 Search Index physical model
- Section 29.4 Term Dictionary and postings
- Section 29.5 Token Index
- Section 29.7 Raw vs normalized indexes
- Section 35.2 Extension conformance tests 3, 8-12, 19-20
```

## Conformance Tests Covered

```text
- Search Granule range and chunk coverage
- sorted Term Dictionary entries
- exact key comparison despite key_hash acceleration
- sorted posting lists
- token search candidates verified against original bytes
- normalized index behavior is explicitly out of scope
- required search metrics are reported
```

## TDD Plan

Write failing tests:

```text
- granule table ranges are inside original_size
- granule chunk_start/chunk_end covers range
- term dictionary entries are sorted
- exact key comparison beats key_hash collision
- posting lists are sorted
- token search result is verified against original bytes
- normalized_utf8 token index creation is rejected or feature-gated out of the MVP
```

## Implementation Tasks

```text
1. implement Search Granule ID model
2. implement one granularity first, preferably line for logs or byte_window for general text
3. implement simple raw UTF-8 tokenizer
4. build term dictionary
5. build delta-varint posting lists
6. implement qzt search for token queries
7. reject or hide normalized token index options in the MVP
8. emit performance metrics
```

## Rust Notes

Keep index building separate from querying. Query code should consume immutable search index views.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- Are original bytes always verified before returning hits?
- Is the index explicitly marked as candidate or complete?
- Is normalized search clearly deferred and unable to silently change original text?
- Are search metrics returned even on capped queries?
- Is this phase avoiding n-gram complexity?
```

## Done Criteria

```text
- token search fixtures pass
- benchmark report fields are present
- no claim of high-performance search unless metrics support it
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.
