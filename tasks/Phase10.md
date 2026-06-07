# Phase10: Search Granules and Token Index MVP

## Purpose

Build the first correct Search Extension path without claiming high-performance search prematurely.

## Minimum MVP

```text
- Search Granule Table for one granularity
- token dictionary builder
- sorted posting lists
- exact verification against original bytes
```

## Goal MVP

```text
- token search works over line or byte_window granularity
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
- Section 35.2 Extension conformance tests 3, 8-12, 19-20
```

## Conformance Tests Covered

```text
- Search Granule range and chunk coverage
- sorted Term Dictionary entries
- exact key comparison despite key_hash acceleration
- sorted posting lists
- token search candidates verified against original bytes
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
```

## Implementation Tasks

```text
1. implement Search Granule ID model
2. implement one granularity first, preferably line for logs or byte_window for general text
3. implement simple tokenizer
4. build term dictionary
5. build delta-varint posting lists
6. implement qzt search for token queries
7. emit performance metrics
```

## Rust Notes

Keep index building separate from querying. Query code should consume immutable search index views.

## Self-Review Checklist

```text
- Are original bytes always verified before returning hits?
- Is the index explicitly marked as candidate or complete?
- Are search metrics returned even on capped queries?
- Is this phase avoiding n-gram complexity?
```

## Done Criteria

```text
- token search fixtures pass
- benchmark report fields are present
- no claim of high-performance search unless metrics support it
- status.md is updated
```

## Status

Pending.
