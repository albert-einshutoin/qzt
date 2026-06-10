# QZT v0.1 Release Hardening

Date: 2026-06-07

## Purpose

This note records the post-Phase13 release hardening gate.

The goal is not to promise absolute performance numbers. The goal is to make release evidence reproducible:

```text
- larger synthetic corpus
- pack/export/range smoke metrics
- token sidecar rare-query evidence
- token sidecar missing-query evidence
- n-gram sidecar common-query cap evidence
- sidecar size ratios
- query-case timing quantiles for evidence-only profiling
```

## Command

```bash
cargo test --test release_hardening -- --nocapture
```

The same test is also included in:

```bash
make check
```

Profile-only run (non-gating, ignored test):

```bash
make bench-profile
```

Matrix profiling across corpus sizes and character sets (1/10/100MB, ASCII/UTF-8-mixed/Japanese):

```bash
make bench-profile-matrix
```

You can override the profile repetition counts with environment variables:

```bash
QZT_RELEASE_BENCH_QUERY_REPETITIONS=500 \
QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=20 \
make bench-profile
```

You can run the matrix profile with the same variables:

```bash
QZT_RELEASE_BENCH_QUERY_REPETITIONS=500 \
QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=20 \
make bench-profile-matrix
```

For routine evidence checks, `QZT_RELEASE_BENCH_QUERY_REPETITIONS=5` and
`QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=2` are practical defaults; use the
default `500/20` only for formal release profiling runs.

## Corpus

The release hardening test uses a deterministic synthetic text corpus:

```text
lines: 24000
bytes: 2423996
chunk size: 8192
rare token: rare-token-unique
common n-gram: aaa
```

The corpus is intentionally repetitive so that it exercises both compression and high document-frequency search behavior.

## Latest Local Output

`cargo test --test release_hardening -- --nocapture` prints a single `release_bench` line with both legacy counters and the new query-case telemetry.

These values are local smoke evidence only. They are not a release SLA.

## Latest Local Profile Output (3 runs, release build)

`make bench-profile` was run with `QZT_RELEASE_BENCH_QUERY_REPETITIONS=500` and
`QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=20` on a local machine.

```text
Case            Run1 p50/p95/p99 (µs)   Run2 p50/p95/p99 (µs)   Run3 p50/p95/p99 (µs)
rare-token      46 / 50 / 58            45 / 48 / 58            45 / 49 / 59
missing-token   36 / 38 / 43            36 / 39 / 46            36 / 39 / 56
common-ngram    194 / 271 / 571         191 / 205 / 277         192 / 204 / 278
```

Run-to-run counters were identical across all runs and 4500 query calls in this configuration
(`3 cases × 500 repetitions × 3 runs`).

## Release Gate Assertions

The automated gate asserts:

```text
- corpus is at least 1,000,000 bytes
- export exactly restores original bytes
- rare token query verifies exactly one hit
- rare token sidecar search decodes less than a raw scan
- common n-gram query is capped before candidate decode
- token/missing/n-gram query case telemetry is reported
- token/missing/n-gram query case timing has warmup + repeat quantiles (p50/p95/p99), treated as evidence only
- token and n-gram sidecar sizes are reported
- pack/export/range throughput metrics are non-zero

- For future release runs, keep metric gates deterministic:
  - candidate/cap/decode counters are compared with semantic checks; timing remains evidence-only
  - index size comparison is path-aware:
    - in-memory estimate and file-sidecar manifest size are intentionally not semantically equivalent
    - high-skip workloads can intentionally reverse index size ordering
```

## Self-Review

```text
- The benchmark is deterministic and does not depend on external files or network state.
- Timing is reported (p50/p95/p99 after warmup), but correctness assertions do not require machine-specific speed thresholds.
- Search evidence remains original-byte verified through QztReader.
- Common-query cap behavior is explicit, so high-frequency terms do not silently trigger large decompression work.
```

## Remaining Product Evidence Gap

This gate does not compare QZT against SQLite FTS, Tantivy, Lucene, seekable zstd, or split-frame object storage.

That competitive benchmark remains the next product-level release question.
