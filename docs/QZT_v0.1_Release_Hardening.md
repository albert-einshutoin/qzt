# QZT v0.1 Release Hardening

Date: 2026-06-07

## Purpose

This note records the post-Phase13 release hardening gate.

The goal is not to promise absolute performance numbers. The goal is to make release evidence reproducible:

```text
- larger synthetic corpus
- pack/export/range smoke metrics
- token sidecar rare-query evidence
- n-gram sidecar common-query cap evidence
- sidecar size ratios
```

## Command

```bash
cargo test --test release_hardening -- --nocapture
```

The same test is also included in:

```bash
make check
```

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

```text
release_bench corpus_bytes=2423996 lines=24000 packed_bytes=132320 compression_ratio=0.054588 qzi_token_bytes=3777818 qzi_token_ratio=1.558508 qzi_ngram_bytes=3689927 qzi_ngram_ratio=1.522250 pack_mib_s=22.732 export_mib_s=60.833 range_mib_s=59.361 rare_token_candidate_granules=1 rare_token_candidate_chunks=1 rare_token_decoded_bytes=97 rare_token_verified_matches=1 common_ngram_candidate_granules=24000 common_ngram_decoded_bytes=0 common_ngram_capped=true raw_scan_decoded_bytes=2423996
```

These values are local smoke evidence only. They are not a release SLA.

## Release Gate Assertions

The automated gate asserts:

```text
- corpus is at least 1,000,000 bytes
- export exactly restores original bytes
- rare token query verifies exactly one hit
- rare token sidecar search decodes less than a raw scan
- common n-gram query is capped before candidate decode
- token and n-gram sidecar sizes are reported
- pack/export/range throughput metrics are non-zero
```

## Self-Review

```text
- The benchmark is deterministic and does not depend on external files or network state.
- Timing is reported, but correctness assertions do not require machine-specific speed thresholds.
- Search evidence remains original-byte verified through QztReader.
- Common-query cap behavior is explicit, so high-frequency terms do not silently trigger large decompression work.
```

## Remaining Product Evidence Gap

This gate does not compare QZT against SQLite FTS, Tantivy, Lucene, seekable zstd, or split-frame object storage.

That competitive benchmark remains the next product-level release question.
