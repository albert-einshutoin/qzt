# Changelog

## 0.1.0 - Unreleased

QZT v0.1 is a technical preview reference implementation of the Cold Evidence
Container format for UTF-8 text.

### Added

- Deterministic QZT Core writer and reader.
- Chunk Table, sparse line index, optional Dense Line Index, and optional
  Document Index support.
- Raw token and n-gram search MVP plus QZI sidecar technical preview.
- Product Completeness Track plan for release hygiene, file-backed I/O,
  stable public API, evidence retrieval, conformance vectors, and acceptance
  thresholds.

### Deferred

- crates.io publication and publish dry-run until Phase20 stabilizes the public
  API.
- Production-ready performance claims until Phase18 and Phase23 evidence is
  recorded.
