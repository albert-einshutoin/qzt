# Changelog

## 0.1.0 - Unreleased

QZT v0.1 is a technical preview reference implementation of the Cold Evidence
Container format for UTF-8 text.

### Added

- `qzt search <file.qzt> <query> [--sidecar …] --format json` emits a single
  JSON object to stdout:
  `{"hits":[{"logical_offset":…,"byte_length":…,"chunk_start":…,"chunk_end":…,"source":"verified_original_bytes"},…],"metrics":{"query":"…","index_kind":"…","posting_granularity":"…","index_size_bytes":…,"source_size_bytes":…,"index_size_ratio":…,"term_lookups":…,"posting_bytes_read":…,"candidate_granules":…,"candidate_chunks":…,"decoded_bytes":…,"physical_decoded_bytes":…,"verified_matches":…,"query_time_ms":…},"capped":bool,"incomplete_reason":null|"…"}`.
  Field names match the `SearchReport` / `SearchMetrics` / `SearchHit` Rust
  struct fields directly. `incomplete_reason` is JSON `null` when there is no
  reason; a string value (e.g. `"query_shorter_than_ngram_n"`) when set. The
  `source` field and all string values are passed through `cli_json::escape` so
  queries containing `"` or other special characters never corrupt the JSON.
  The stderr incomplete-result warning is still emitted in JSON mode (stdout
  remains clean). Zero-hit searches emit `"hits":[]`. Unknown `--format` values
  exit with code 2. Default text output is unchanged (existing tests pass
  without modification). `SearchReport`, `SearchHit`, and `SearchMetrics` are
  now exported from the `qzt` crate public API.

- `qzt docs <file.qzt>` lists the Document Index entries in tab-separated columns
  (`doc_id`, `offset`, `bytes`, `first_line`, `lines`, `checksum`); the header is
  the first stdout line. `--format json` emits
  `{"documents":[{"doc_id":"…","logical_offset":…,"byte_length":…,"first_line":…,"line_count":…,"checksum":{"algorithm":"blake3","value":"…"}}]}`.
  `first_line` is converted from its 0-based stored value to 1-based in both text
  and JSON output (aligned with `qzt line` semantics). `doc_id` values containing
  literal tabs or newlines are escaped as `\t` / `\n` in text mode; JSON uses the
  existing `cli_json::escape` helper. A container without a Document Index exits 1
  in both modes (distinct from a valid index with zero entries).
- `qzt doc <file.qzt> <doc_id>` extracts one document by ID with BLAKE3
  verification by default (reads the stored checksum from the Document Index and
  calls `read_document_verified`). `--no-verify` skips the checksum check for a
  faster path. `-o <path>` writes to a file instead of stdout. An unknown `doc_id`
  or a missing Document Index exits 1; a verification failure (corrupt bytes) also
  exits 1. JSON escape and hex helpers from `cli_json` are reused for all output.

- `qzt verify` now prints two report lines after the existing compatibility line:
  `Checked chunks: <n>` and `Decoded bytes: <n>` (decoded bytes is 0 for
  `quick`/`normal` levels, and equals the original size for `deep`).
- `qzt verify --format json` outputs a single-line JSON object to stdout:
  `{"ok":true,"level":"deep","checked_chunks":297,"decoded_bytes":2423996}` on
  success, or `{"ok":false,"level":"deep","error":"..."}` (exit code 1) on
  failure. In JSON mode, all output goes to stdout; stderr is silent so JSON
  consumers need only read stdout.
- Exit codes are now documented in `qzt --help` and in `README.md` /
  `README.ja.md`: `0` = success, `1` = command failed, `2` = usage error.
- `qzt info` now appends three identity lines after the existing output:
  `Container ID` (lowercase hex UUID), `Original checksum` (`blake3:<hex>`),
  and `Newline mode`. Existing lines are unchanged.
- `qzt info --format json` outputs a single JSON object to stdout with all
  container fields including `container_id` and `original_checksum`. Unknown
  `--format` values exit with code 2. JSON is hand-assembled without a serde
  dependency via the new `src/cli_json.rs` helper module (also used by
  subsequent Value Phase 1 commands).
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
- `QziFileSidecar`: file-backed sidecar reader with lazy lookup. Opening loads
  only the manifest and term dictionary and stream-verifies section checksums;
  posting lists and candidate granule records are fetched per query, so search
  memory scales with the candidate set instead of the sidecar size.
- `RawTokenIndex::build_from_file` / `RawNgramIndex::build_from_file` /
  `build_search_sidecar_from_file`: index construction over `QztFileReader`
  that decodes one chunk at a time (sidecar output is byte-identical to the
  in-memory builder).
- `RawTokenIndex::search_file` / `RawNgramIndex::search_file`: hit
  verification over a file-backed container that decodes only candidate
  chunks.

### Fixed

- `WriterBuilder::pack` now validates the profile string via `validate_profile`;
  previously any arbitrary string was accepted, producing a container whose
  metadata would be rejected at read time. All pack paths now call
  `validate_profile` unconditionally inside `pack_bytes_internal`.
- The `"memory"` profile requirement that a `DocumentIndex` must be provided is
  now enforced on every pack path, not only on the `WriterBuilder` path.
  Previously `pack_bytes_with_profile` could silently produce a `"memory"`
  container without a Document Index. **Behaviour change**: calls that relied on
  these previously-silent successes now return `QztError::MetadataInvalid`.

### Changed

- `QztError::Display` now emits human-readable messages for all variants instead
  of delegating to `{error:?}` (Debug output). Variant identifiers such as
  `InvalidMagic` no longer appear raw in error text.
- `QztError::NotImplemented` removed; replaced by two purpose-built variants:
  - `QztError::Io(std::io::ErrorKind)` — preserves OS-level I/O error kind
    (file not found, permission denied, write failure, …).
  - `QztError::UnsupportedIndexMode(&'static str)` — signals that the requested
    search index mode is not supported by this implementation.
- `QztFileReader::open_path` and `QziFileSidecar::open_path` now return
  `QztError::Io(kind)` instead of `QztError::ContainerCorrupt` when the
  underlying `File::open` or `File::metadata` call fails.
- `export_to` write failures now return `QztError::Io(kind)` instead of
  `QztError::ContainerCorrupt`.
- `qzt search`, `qzt info`, and `qzt sidecar-rebuild` now run on the
  bounded-memory `QztFileReader` instead of loading the whole container (and
  sidecar) into memory. On a 42 MB / 400K-line corpus with an n-gram sidecar,
  a rare sidecar query dropped from 518 MB to 9.8 MB max RSS (1.33 s to
  0.04 s) and an 80K-hit dense query from 532 MB to 36 MB (1.11 s to 0.17 s).
  Index builds still hold the posting map in memory.
- The search index builder streams chunk-by-chunk and finds chunk spans by
  binary search, removing the O(lines x chunks) granule scan.

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
- **Public API signatures** (pedantic pass, #9): `pack_bytes_with_document_index`
  and `pack_bytes_with_memory_profile` now take `&DocumentIndex` instead of an
  owned value; `run_release_benchmark_with_corpus` now takes `&[u8]` instead of
  `Vec<u8>`.

### Deferred

- crates.io publication and publish dry-run until Phase20 stabilizes the public
  API.
- Production-ready performance claims until Phase18 and Phase23 evidence is
  recorded.
