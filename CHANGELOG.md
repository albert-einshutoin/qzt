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

### Deferred

- crates.io publication and publish dry-run until Phase20 stabilizes the public
  API.
- Production-ready performance claims until Phase18 and Phase23 evidence is
  recorded.
