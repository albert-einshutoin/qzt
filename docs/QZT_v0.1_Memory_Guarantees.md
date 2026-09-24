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
allocation. Chunk Table entries are also rejected at open when compressed or
uncompressed size exceeds the configured per-chunk limit. Normal verification
hashes compressed bytes through a 64 KiB buffer; deep verification holds at
most one bounded compressed chunk and its bounded decoded output at a time.

At open, the shared Chunk Table validator rejects line counts larger than the
corresponding uncompressed byte counts, including when no Dense Line Index
(DLI) exists. DLI decoding checks declared entry and offset counts against the
Chunk Table and the remaining encoded bytes before reserving vectors. The
`ResourceLimits::max_dense_line_index_allocation` budget counts the requested
capacity in bytes for the outer `DenseLineEntry` vector plus every chunk's
`u64` offset vector cumulatively. Its default is 256 MiB: a 64 MiB encoded
index block can expand substantially when varints become `u64` offsets, while
the separate 64 MiB `max_index_block_size` still bounds stored block bytes.
Containers whose DLI would require more than 256 MiB of vector capacity now
need an explicit higher limit. This adds a field to the public `ResourceLimits`
struct; source users constructing it without `..ResourceLimits::default()`
must set the new field. The CBOR-only budgets are unchanged.
