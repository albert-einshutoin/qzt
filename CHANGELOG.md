# Changelog

## 0.1.0 - Unreleased

QZT v0.1 is a technical preview reference implementation of the Cold Evidence
Container format for UTF-8 text.

### Added

- Deterministic QZT Core writer and reader.
- Chunk Table, sparse line index, optional Dense Line Index, and optional
  Document Index support.
- Raw token and n-gram search MVP plus QZI sidecar technical preview.
- Product Completeness Track: file-backed `QztFileReader`, streaming writer,
  streaming verify/export, competitive benchmarks, resource governance, curated
  public API, verified evidence retrieval, portable conformance vectors, and
  acceptance threshold harness.
- `DocumentEntry::new()` constructor that derives `doc_id_hash` automatically;
  callers no longer need to depend on blake3 directly.
- `#[non_exhaustive]` on `QztError`, `VerifyLevel`, `SearchIndexSource`,
  `SidecarIndexKind`, and `CorpusKind` to allow future variant additions without
  breaking downstream match exhaustiveness.
- `QztError::DocumentNotFound` to distinguish "no document index present" from
  "id not found in index".
- `SearchMetrics::physical_decoded_bytes` reporting the uncompressed bytes
  actually decompressed during hit verification (chunk-level work), alongside
  the logical `decoded_bytes`.
- `SearchReport::incomplete_reason` is now set for queries the index cannot
  answer: `query_shorter_than_ngram_n` (n-gram query with fewer scalars than
  the index `n`) and `query_has_no_indexable_tokens` (token query with no
  ASCII-alphanumeric tokens). The CLI prints the reason and a stderr warning
  instead of returning a confident empty result.

### Changed

- Search hit verification reuses a single-chunk decode cache, so hit-dense
  queries decode each candidate chunk once instead of once per candidate
  granule (measured 16.4 s -> sub-second on a 45 MB corpus with 4,124 hits).
- `qzt export` streams chunks directly to the output file or stdout instead of
  materializing the whole original in memory, matching the documented
  bounded-memory guarantee.
- `make bench-release` now actually builds with `--release`; earlier recorded
  throughput numbers in `tasks/status.md` were debug-build values.
- The quality gate (`make check` and CI) also compiles the default-features
  surface via `cargo check --lib --bins`; internal-testing-only items are
  explicitly `allow(dead_code)` in the curated build.

### Deferred

- crates.io publication and publish dry-run until Phase20 stabilizes the public
  API.
- Production-ready performance claims until Phase18 and Phase23 evidence is
  recorded.
