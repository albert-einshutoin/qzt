# QZT v0.1 Memory and Resource Guarantees

Date: 2026-06-08

These are implementation guarantees for the Rust reference implementation.

```text
open(path): fixed header/trailer plus metadata, footer payload, index root,
            chunk table, and optional index blocks. Chunk data is not read.
range(path): index region plus compressed chunks overlapping the byte range.
line(path): index region plus chunks spanning the requested line.
export(path): one decoded chunk at a time.
verify quick: structural metadata and index region only.
verify normal: compressed chunks streamed one at a time for checksum checks.
verify deep: compressed chunks decoded one at a time; original checksum and
             line/newline state are accumulated incrementally.
search: bounded by SearchOptions max_candidate_granules, max_decoded_bytes,
        and max_search_results.
```

CBOR allocation and item budgets are sourced from `ResourceLimits`, including
`max_cbor_allocation` and `max_cbor_items`, before decoded values drive heap
allocation.
