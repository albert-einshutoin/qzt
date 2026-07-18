# QZT v0.1 Competitive Benchmark Harness

Date: 2026-06-08

The Phase18 harness is reproducible measurement, not an SLA. It uses the
Phase23 validation corpus generators and compares QZT range restore against
whole-file raw zstd restore on the same bytes.

The latest measured report and complete raw runs are published in
[docs/benchmarks/2026-07-v0.1.md](benchmarks/2026-07-v0.1.md).

## When To Use QZT

QZT v0.1 is a technical preview. Use it when verified original-byte evidence
matters more than database-style indexing or whole-file decompression.

| Workload | Use QZT when | Caveat |
| --- | --- | --- |
| Evidence containers | You need a cold, immutable container that preserves source bytes and integrity checks | Technical preview; no production SLA or performance guarantee |
| Large immutable logs | Append-once text must stay seekable without recompressing or decoding the whole archive | Benchmark timing is reproducible evidence, not a performance promise |
| Byte-exact range restore | Callers need a verified slice from compressed storage, not a full-file decode | Restore decodes only the chunks overlapping the requested range |
| Verified document retrieval | Stable IDs should resolve to original-byte ranges and round-trip through partial decode | Document Index is an optional extension, not a replacement for original text |
| Rebuildable sidecar search | Search can live in a derived QZI index while hits are verified against container bytes | QZI sidecars are rebuildable, optional, and may be larger than source text |

Run the default smoke:

```sh
cargo test --test phase18_competitive_benchmark -- --nocapture
```

External tool comparisons against SQLite FTS5 and ripgrep run behind the
`bench-compete` feature so the default quality gate remains portable. Missing
tools are skipped; tools that are present must return the same hit count as the
reference byte scan.

```sh
cargo test --release --all-features --test phase18_competitive_benchmark -- --nocapture
```

`--all-features` enables `bench-compete` together with the internal test
surface imported by this integration harness.

## Methodology

- Generate deterministic C1-C6 corpus bytes from explicit seeds.
- Pack with QZT using fixed chunk size.
- Compress the same corpus with whole-file zstd.
- Restore the same byte range through `QztFileReader` and through whole-file
  zstd decode.
- Assert byte equality before recording timing.
- Record sidecar/search correctness separately from timing.
- With `bench-compete`, run ripgrep and SQLite FTS5 over the same corpus and
  fail the harness if their hit counts disagree with the reference byte scan.

## When Not To Use QZT

QZT is not a replacement for a full-text database when mutable ranking,
normalized language search, or high-update indexing is the primary workload.
QZI sidecars are rebuildable and can be larger than source text, so they should
be used when verified original-byte evidence matters more than index compactness.
