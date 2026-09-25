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
search: per-query limits on input keys, posting work, candidate verification,
        physical decompression, and returned hits; see the table below.
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

Public Writer success is constrained by the same default Reader limits. A
chunk's uncompressed size is at most 64 MiB, compressed bytes at most 72 MiB,
and each stored index block at most 64 MiB. Chunk Table entry count is checked
against its 128-byte record size before the next chunk is encoded. Metadata,
Index Root, Footer, and Document Index CBOR must fit the Reader's 16 MiB
decoded payload/key-copy budget and 1,000,000 decoded-value budget per block;
the Document Index is measured one record at a time before its complete CBOR
tree is built. Generated Dense Line Index vectors must fit the cumulative
256 MiB allocation budget before construction, and their encoded block must
fit 64 MiB before encoding. These checks can reject settings or metadata that
older Writers accepted. They bound the named structures, not total process RSS
or all in-memory `WriterBuilder` work. Generic sink flush does not perform file
sync; CLI output uses a separate atomic replacement and durability procedure.

## Search and index-build budgets

The defaults bound a typical interactive query while preserving the existing
256 MiB logical verification limit and QZI's 128 MiB posting-byte/10-million-ID
guardrails. They limit the named work, **not** peak process RSS: dictionary and
posting maps built for a transient index still scale with corpus vocabulary.
All numeric limits are inclusive. Zero permits no work in the named unit;
an empty query or empty source remains valid where it uses no such unit.

| Limit (default) | Scope and unit; check point | Overrun | API / CLI |
|---|---|---|---|
| Query bytes (4 KiB) | One query's UTF-8 bytes, before copy, key generation, or CLI index build | Error | `SearchOptions.max_query_bytes` / `--max-query-bytes` |
| Distinct keys (256) | One query's unique ASCII-folded token or exact n-gram keys, before adding the next key | Error | `max_query_terms` / `--max-query-terms` |
| Encoded postings (128 MiB) | Sum of selected lists' actual encoded bytes, before file-backed list reads or raw intersection | Error | `max_posting_bytes_per_query` / `--max-posting-bytes` |
| Posting IDs (10,000,000) | Sum of selected lists' decoded ID counts, before file-backed fetch/decode or raw intersection | Error | `max_posting_ids_per_query` / `--max-posting-ids` |
| Intersection work (20,000,000) | One query's first-list ID copies plus ID comparisons and output pushes, charged before each step | Error | `max_posting_work` / `--max-posting-work` |
| Candidate granules (10,000) | Intersected candidates, before candidate decode; file-backed QZI stops before granule fetch | `max_candidate_granules` cap | `max_candidate_granules` / `--max-candidates` |
| Logical bytes (256 MiB) | Cumulative candidate granule bytes and any adjacent token-boundary bytes, checked before each read | `max_decoded_bytes` cap | `max_decoded_bytes` / `--max-decoded-bytes` |
| Physical bytes (256 MiB) | Cumulative full uncompressed chunk sizes on cache misses, before decompression | `max_physical_decoded_bytes` cap | `max_physical_decoded_bytes` / `--max-physical-decoded-bytes` |
| Physical chunks (10,000) | Cumulative decompression calls on cache misses, before decompression | `max_physical_decoded_chunks` cap | `max_physical_decoded_chunks` / `--max-physical-decoded-chunks` |
| Results (10,000) | Verified hit spans retained, before further span generation | `max_search_results` cap | `max_search_results` / `--max-results` |
| Source line (16 MiB) | One line's original bytes including LF and optional CR, before carry growth or key generation | Error | `TokenIndexBuildOptions` / `NgramIndexBuildOptions.max_line_bytes`; `build_search_sidecar_from_file_with_line_limit`; CLI `--max-line-bytes` during raw build or `sidecar-rebuild` |

File-backed QZI also applies the lower of its open-time `SidecarLimits` and
query `SearchOptions` for encoded posting bytes and decoded IDs. Stored QZI
section sizes and Reader per-chunk limits remain separate. `posting_bytes_read`
is a planner estimate for a transient n-gram index; it is **not** the posting
budget meter. A cache hit is free. If a chunk was evicted, decompression is
charged again; `physical_decoded_chunks` therefore differs from the union of
candidate chunk ranges. A cap sets `capped=true` and `stop_reason`, retains only
already verified hits, and exits the CLI successfully. Corruption, I/O, Reader
limits, and query/posting/line overruns remain errors (CLI exit 1). The
independent `incomplete_reason` describes index/query semantic incompleteness.
`index_complete_declared` is the index's declaration;
`index_coverage_verified` is currently false even when the declaration is true.
Adjacent token-boundary reads use the same logical and physical budgets.
