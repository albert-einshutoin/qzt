# QZT v0.1 Competitive Benchmark Harness

Date: 2026-06-08

The Phase18 harness is reproducible measurement, not an SLA. It uses the
Phase23 validation corpus generators and compares QZT range restore against
whole-file raw zstd restore on the same bytes.

Run the default smoke:

```sh
cargo test --test phase18_competitive_benchmark -- --nocapture
```

External tool comparisons against SQLite FTS5 and ripgrep run behind the
`bench-compete` feature so the default quality gate remains portable. Missing
tools are skipped; tools that are present must return the same hit count as the
reference byte scan.

```sh
cargo test --features bench-compete --test phase18_competitive_benchmark -- --nocapture
```

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
