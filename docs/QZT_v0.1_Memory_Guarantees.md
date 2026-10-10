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
| Source line (16 MiB) | One line's original bytes including LF and optional CR, before carry growth or key generation | Error | `TokenIndexBuildOptions` / `NgramIndexBuildOptions.max_line_bytes`; `SidecarBuildOptions.max_line_bytes`; CLI `--max-line-bytes` during raw build or `sidecar-rebuild` |

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

## Index construction admission (Unreleased)

Added after public pre.6. These source APIs/options are absent from the public
pre.6 binary. One `IndexBuildLimits` applies to token and Unicode-scalar n-gram
construction through `build_from_container`, `build_from_file`, transient CLI
search, QZI default memory/file builders and the custom sidecar builder.
No raw-scan or alternate-builder fallback occurs on refusal.

| Field | Default | Unit and check point |
|---|---:|---|
| `max_granules` | 1,000,000 | Retained line records; before push. |
| `max_distinct_keys` | 262,144 | Dictionary keys; before new-key copy/insert. |
| `max_posting_ids` | 8,000,000 | Distinct key/line pairs across all lists; before push. Repetitions within one line are free, a new line costs another pair. |
| `max_key_bytes` | 16 MiB | Sum of retained distinct key lengths; before new-key copy/insert. An individual scratch key must also fit this limit. |
| `max_encoded_bytes` | 128 MiB | Transient delta postings plus 24-byte skip records, or QZI granule+dictionary+posting data sections; each phase is checked independently before encoding. |

All limits are inclusive; zero allows no elements/bytes in that unit. Empty
raw indexes use zero retained/encoded posting bytes. An empty QZI still needs
16 data-section header bytes, plus the excluded envelope. Overflow and overrun
return `ResourceLimitExceeded` (CLI exit 1), never a capped or partial-success
index. Invalid CLI values are usage errors (exit 2) before file I/O. Token
`from_parts` takes explicit limits; n-gram `from_parts` uses its options' limits
and validates already-materialized counts before encoding.

The count limits bound logical lengths, **not requested/allocated capacities**.
Vector spare capacity, BTree nodes, allocator bookkeeping, the decoded Reader
chunk, line carry, one normalized-token scratch key and QZI header/manifest are
excluded. The existing Reader limits and separate source-line limit still
apply. Data sections, the final QZI output and raw structures may coexist;
the encoded limit is not their summed live memory. Caller-supplied materialized
arrays and public post-construction mutation are not admission-controlled
allocations. No strict RSS or OOM-prevention guarantee is made.

The initial defaults keep distinct-key bytes at 16 MiB, logical u64 posting
payload at 64,000,000 bytes, granules at one million and each encoded phase at
128 MiB. Small C2/C4 profiles observed growth in line keys, posting maps and
later encoding; the defaults add explicit admission before the larger
100 MiB runs that previously hit external monitoring. They are conservative
operation budgets, not source-size-to-RSS estimates. Acceptance is intentionally
narrower: inputs formerly accepted may now require raised explicit limits or
may be refused. Safe refusal is not evidence of faster processing or large-input
support. See the [plan and profile rationale](benchmarks/2026-10-index-build-budget-plan.md).

QZI opening/reconstruction continues under the existing `SidecarLimits`.
Its materialized reconstruction passes those established Reader counts to the
same constructor; source-build defaults do not tighten Reader acceptance.
Query, Reader limits, source binding, posting order/deduplication and QZI v1/v2
bytes are unchanged for admitted builds. CLI builds complete bytes before
the existing atomic-output path, so refusal preserves input and existing output
and does not create a new completed QZI.
